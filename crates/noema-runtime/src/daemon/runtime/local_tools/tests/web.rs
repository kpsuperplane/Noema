#[tokio::test]
async fn web_fetch_runtime_context_uses_saved_summarizer_preference() {
    let store = crate::test_support::test_store().await;
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation");
    upsert_ready_auxiliary_model_preference(
        &store,
        NewAuxiliaryModelPreference {
            task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        },
    )
    .await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([
            (
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            ),
            (
                "foundation_local".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("foundation-tool-default")))
                    as noema_providers::ProviderHandle,
            ),
        ]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let context = actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect("web fetch context");

    assert_eq!(
        context.summarizer_route.selection().provider_kind,
        "foundation_local",
    );
    assert_eq!(context.summarizer_model, "default");
    assert_eq!(
        context.generation_priority,
        noema_providers::GenerationPriority::Foreground
    );
}

#[tokio::test]
async fn web_fetch_runtime_context_uses_saved_summarizer_reasoning_effort() {
    let store = crate::test_support::test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate codex");
    upsert_ready_auxiliary_model_preference(
        &store,
        NewAuxiliaryModelPreference {
            task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
        },
    )
    .await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let context = actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Background)
        .await
        .expect("web fetch context");

    assert_eq!(
        context.summarizer_reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
    assert_eq!(
        context.generation_priority,
        noema_providers::GenerationPriority::Background
    );
}

#[tokio::test]
async fn web_fetch_runtime_context_rejects_saved_provider_missing_from_runtime() {
    let store = crate::test_support::test_store().await;
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation");
    upsert_ready_auxiliary_model_preference(
        &store,
        NewAuxiliaryModelPreference {
            task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        },
    )
    .await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let message = actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect_err("missing provider should fail");

    assert_eq!(
        message,
        "web.fetch summarizer provider is not available in this daemon"
    );
}

#[tokio::test]
async fn web_fetch_runtime_context_uses_initialized_auxiliary_selection() {
    let store = crate::test_support::test_store().await;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::with_requests(
                Some("configured-tool-override"),
                Arc::clone(&requests),
            )) as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let context = actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect("web fetch context");

    assert_eq!(context.summarizer_model, "gpt-5.6-luna");
    let requests = requests.lock().expect("requests");
    assert!(requests.is_empty());
}

#[tokio::test]
async fn bound_exa_web_search_without_secret_falls_back_to_duckduckgo() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = create_exa_provider_account(
        &store,
        noema_providers::ProviderAccountStatus::Authenticated,
    )
    .await;
    insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let (provider, fallback_from, fallback_reason, auth_failure_account_id) = actor
        .web_search_runtime_provider_resolution()
        .await
        .expect("provider");

    assert_eq!(
        provider.backend_id(),
        crate::search::types::DUCKDUCKGO_PUBLIC_PROVIDER_ID,
    );
    assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
    assert_eq!(
        fallback_reason.as_deref(),
        Some("provider account unauthenticated")
    );
    assert!(auth_failure_account_id.is_none());
    let account = store
        .get_provider_account(&provider_account_id)
        .await
        .expect("provider account")
        .expect("Exa account");
    assert_eq!(
        account.status,
        noema_providers::ProviderAccountStatus::Unauthenticated
    );
    assert_eq!(account.last_error_code.as_deref(), Some("auth_failed"));
}

#[tokio::test]
async fn bound_exa_web_fetch_without_secret_falls_back_to_direct_http() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = create_exa_provider_account(
        &store,
        noema_providers::ProviderAccountStatus::Authenticated,
    )
    .await;
    insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let (provider, context, fallback_from, fallback_reason, auth_failure_account_id) = actor
        .web_fetch_runtime_execution_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect("context");

    assert_eq!(
        provider.backend_id(),
        crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID,
    );
    assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
    assert_eq!(
        fallback_reason.as_deref(),
        Some("provider account unauthenticated")
    );
    assert!(auth_failure_account_id.is_none());
    assert_eq!(context.summarizer_model, "gpt-5.6-luna");
    assert_eq!(
        context.generation_priority,
        noema_providers::GenerationPriority::Foreground
    );
}

#[tokio::test]
async fn web_search_local_tool_result_payload_preserves_fallback_metadata() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = ensure_provider_account_without_web_capabilities(&store).await;
    insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let result = actor
        .execute_local_tool(
            &test_turn(),
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                "web.search",
                json!({
                    "query": "   ",
                }),
            ),
        )
        .await;

    let GenerateActionItem::ToolResult { payload, .. } =
        super::local_tool_result_action_item(&result)
    else {
        panic!("expected tool result action item");
    };

    assert_eq!(payload["fallback_from"], provider_account_id);
    assert_eq!(
        payload["fallback_reason"],
        "bound provider account does not declare web.search"
    );
    assert_eq!(payload["error"], "query is required");
}

#[tokio::test]
async fn web_fetch_local_tool_result_payload_preserves_fallback_metadata() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = ensure_provider_account_without_web_capabilities(&store).await;
    insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let result = actor
        .execute_local_tool(
            &test_turn(),
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                "web.fetch",
                json!({
                    "url": "",
                }),
            ),
        )
        .await;

    let GenerateActionItem::ToolResult { payload, .. } =
        super::local_tool_result_action_item(&result)
    else {
        panic!("expected tool result action item");
    };

    assert_eq!(payload["fallback_from"], provider_account_id);
    assert_eq!(
        payload["fallback_reason"],
        "bound provider account does not declare web.fetch"
    );
    assert_eq!(payload["error"], "url is required");
}
