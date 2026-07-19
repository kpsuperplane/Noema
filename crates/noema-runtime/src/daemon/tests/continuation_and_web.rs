#[tokio::test]
async fn rejected_response_chain_falls_back_to_complete_local_replay() {
    let provider = Arc::new(
        FakeCodexProvider::new(FakeCodexScenario::NativeSearchMemoryContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                tool_choice: true,
                allowed_tools: false,
                schema_dialect: noema_providers::ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                prompt_cache_retention: true,
                prompt_cache_key: false,
                prompt_cache_options: false,
                prompt_cache_breakpoints: false,
                encrypted_reasoning: false,
            })
            .with_response_continuation(ProviderResponseContinuation::PreviousResponseId {
                store_response: false,
            })
            .rejecting_first_chained_request(),
    );
    let (handle, _store, _server) = spawn_runtime_with_memory_provider(
        provider.clone(),
        Arc::new(RecordingMemoryOperations::default()),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn succeeds after replay fallback");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "native tool result received"
    )));
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1].options.previous_response_id.as_deref(),
        Some("resp_1")
    );
    assert!(matches!(requests[1].input, GenerateInput::Items(_)));
    assert_eq!(input_tool_results(&requests[1].input).len(), 1);
    assert!(requests[2].options.previous_response_id.is_none());
    assert!(matches!(requests[2].input, GenerateInput::Items(_)));
    assert_eq!(input_tool_results(&requests[2].input).len(), 1);
}

#[tokio::test]
async fn native_provider_can_call_web_fetch_and_continue() {
    let provider = Arc::new(native_fake_provider(
        FakeCodexScenario::NativeWebFetchContinuation,
    ));
    let store = crate::test_support::test_store().await;
    store
        .record_observed_urls(
            noema_store::ObservedUrlSource::SearchResult,
            "tool_call:test_search",
            &["https://example.com/page".to_string()],
        )
        .await
        .expect("record observed URL");
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Fetch the example page.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I read the fetched page."
    )));
    assert!(provider.requests().iter().any(|request| {
        input_tool_results(&request.input).iter().any(|result| {
            result.name == "web.fetch"
                && result.success
                && result.payload["content"] == "Test page content"
        })
    }));
}

#[tokio::test]
async fn native_provider_can_create_local_artifact_with_two_versions_and_continue() {
    let provider = Arc::new(native_fake_provider(
        FakeCodexScenario::NativeArtifactCreateLocalFileContinuation,
    ));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Create a small artifact and revise it once.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "I created the two-version artifact."
    )));
    let artifacts = store
        .list_artifacts_for_owner(
            noema_artifacts::ArtifactOwnerRef::conversation(&conversation_id),
            10,
        )
        .await
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].versions.len(), 2);
    assert_eq!(artifacts[0].current_version.version_index, 2);
    assert!(provider.requests().iter().any(|request| {
        input_tool_results(&request.input).iter().any(|result| {
            result.name == "artifact.create_local_file"
                && result.success
                && result.payload["current_version_index"] == 2
        })
    }));
}

#[tokio::test]
async fn hard_ceiling_and_audit_failure_get_one_no_tools_finalization_attempt() {
    for scenario in [
        FakeCodexScenario::LongContinuationThenFinalization,
        FakeCodexScenario::ProgressAuditFailsThenFinalization,
    ] {
        assert_one_no_tools_finalization(scenario).await;
    }
}

async fn assert_one_no_tools_finalization(scenario: FakeCodexScenario) {
    let provider = Arc::new(FakeCodexProvider::new(scenario));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    if matches!(
        scenario,
        FakeCodexScenario::LongContinuationThenFinalization
    ) {
        authenticate_provider_account(&store, &codex.provider_account_id).await;
        upsert_ready_agent_runtime_preference(
            &store,
            noema_store::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: codex.provider_account_id,
                model_profile: "gpt-5.5".to_string(),
                reasoning_effort: Some(noema_providers::ReasoningEffort::High),
            },
        )
        .await;
    } else {
        upsert_ready_auxiliary_model_preference(
            &store,
            noema_store::NewAuxiliaryModelPreference {
                task_id: noema_store::TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: codex.provider_account_id,
                model_profile: "gpt-5.4-mini".to_string(),
                reasoning_effort: None,
            },
        )
        .await;
    }
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id,
        "Research healthy restaurants and keep going.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let requests = provider.requests();
    let finalization_requests = requests
        .iter()
        .filter(|request| {
            request.tools.is_empty()
                && !request.parallel_tool_calls
                && request
                    .instructions
                    .as_deref()
                    .is_some_and(|instructions| instructions.contains("must stop now"))
        })
        .collect::<Vec<_>>();
    assert_eq!(finalization_requests.len(), 1);
    if matches!(
        scenario,
        FakeCodexScenario::LongContinuationThenFinalization
    ) {
        assert_eq!(
            finalization_requests[0].options.reasoning_effort,
            Some(noema_providers::ReasoningEffort::High)
        );
    }
}
