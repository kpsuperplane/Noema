#[tokio::test]
async fn injected_capability_invoker_receives_the_opaque_advertised_target() {
    let mut actor = test_actor().await;
    let invoker = Arc::new(RecordingCapabilityInvoker::default());
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            invoker.clone(),
        )]);
    let mut turn = test_turn();
    turn.initial_model_tools =
        test_injected_capability_model_tools(Arc::new(noema_capabilities::OmitPayloadSanitizer));
    let call = test_tool_call(TEST_CAPABILITY_NAME, json!({"document_id": "document:1"}));

    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &call,
        )
        .await;

    assert!(result.success);
    assert_eq!(result.payload, json!({"document": "contents"}));
    let invocations = invoker.invocations.lock().expect("invocation lock");
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].operation.as_str(), TEST_CAPABILITY_NAME);
    assert_eq!(
        invocations[0].operation_token.as_str(),
        "opaque-child-authority"
    );
    assert_eq!(invocations[0].arguments, call.payload);
}

#[tokio::test]
async fn unconfigured_reviewer_blocks_external_write_before_invocation() {
    let mut actor = test_actor().await;
    let invoker = Arc::new(RecordingCapabilityInvoker::default());
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            invoker.clone(),
        )]);
    let conversation = actor
        .store
        .get_or_create_primary_conversation(
            "human:local",
            Some("gpt-test".to_string()),
            None,
        )
        .await
        .expect("primary conversation");
    let mut turn = test_turn();
    turn.conversation_id = conversation.conversation_id;
    turn.initial_model_tools = test_governed_capability_model_tools();

    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(TEST_CAPABILITY_NAME, json!({"body":"exact"})),
        )
        .await;

    assert!(!result.success);
    assert_eq!(result.payload["status"], "awaiting_approval");
    let action_id = result.blocked_action_id.expect("blocked action id");
    let action = actor
        .store
        .get_governed_action(&action_id, 1)
        .await
        .expect("read action")
        .expect("action");
    assert_eq!(
        action.state,
        noema_store::GovernedActionState::AwaitingApproval
    );
    assert_eq!(action.arguments, json!({"body":"exact"}));
    assert!(invoker.invocations.lock().expect("invocation lock").is_empty());
}

#[tokio::test]
async fn unavailable_capability_remains_unadvertised_and_denied() {
    let mut actor = test_actor().await;
    let invoker = Arc::new(RecordingCapabilityInvoker::default());
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            invoker.clone(),
        )]);
    let turn = test_turn();
    assert!(
        turn.initial_model_tools
            .provider_tools()
            .iter()
            .all(|tool| tool.name.as_str() != TEST_CAPABILITY_NAME)
    );

    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(TEST_CAPABILITY_NAME, json!({"secret": "do not persist"})),
        )
        .await;

    assert!(!result.success);
    assert_eq!(
        result.payload,
        json!({"error": "capability operation is unavailable"})
    );
    assert_eq!(result.persisted.arguments, None);
    assert_eq!(result.persisted.output, None);
    assert!(result.requires_provider_continuation);
    assert!(
        invoker
            .invocations
            .lock()
            .expect("invocation lock")
            .is_empty()
    );

    let forged = test_actor()
        .await
        .execute_local_tool(
            &test_turn(),
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call("forged.operation", json!({"secret": "do not persist"})),
        )
        .await;
    assert!(!forged.success, "unadvertised forged call must fail");
    assert_eq!(
        forged.payload["error"], "capability operation is unavailable",
        "unadvertised forged call must expose only the sanitized failure"
    );
    assert_eq!(forged.persisted.arguments, None);
    assert_eq!(forged.persisted.output, None);
    assert!(forged.requires_provider_continuation);
}

#[tokio::test]
async fn advertised_capability_failure_is_sanitized_and_continues_to_provider() {
    let mut actor = test_actor().await;
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            Arc::new(RecordingCapabilityInvoker::returning(Err(
                CapabilityError::Failed,
            ))),
        )]);
    let mut turn = test_turn();
    turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
        noema_capabilities::RedactingPayloadSanitizer,
    ));

    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                TEST_CAPABILITY_NAME,
                json!({"document_id": "document:1", "api_key": "private"}),
            ),
        )
        .await;

    assert!(!result.success);
    assert_eq!(
        result.payload,
        json!({"error": "capability invocation failed"})
    );
    assert_eq!(
        result.persisted.arguments,
        Some(json!({"document_id": "document:1", "api_key": "[REDACTED]"}))
    );
    assert_eq!(result.persisted.output, Some(json!({"error": "failed"})));
    assert!(result.requires_provider_continuation);
}

#[tokio::test]
async fn tool_declared_failed_capability_output_is_persisted_as_failed() {
    let mut actor = test_actor().await;
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            Arc::new(RecordingCapabilityInvoker::returning(Ok(
                CapabilityOutput::failed(json!({
                    "isError": true,
                    "error": "document rejected",
                    "password": "private"
                })),
            ))),
        )]);
    let mut turn = test_turn();
    turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
        noema_capabilities::RedactingPayloadSanitizer,
    ));

    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(TEST_CAPABILITY_NAME, json!({"document_id": "document:1"})),
        )
        .await;

    assert!(!result.success);
    assert_eq!(result.payload["isError"], true);
    assert_eq!(
        result.persisted.output,
        Some(json!({
            "isError": true,
            "error": "document rejected",
            "password": "[REDACTED]"
        }))
    );
    assert!(result.requires_provider_continuation);
}

#[tokio::test]
async fn task_delegate_uses_the_initialized_reviewer_route() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("default account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate default account");
    let provider_account_id = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account")
        .provider_account_id;
    store
        .update_provider_account_status(
            &provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation account");
    let provider_registry = crate::test_support::ready_test_provider_registry();
    let _pool = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", provider_registry.as_ref())
        .await
        .expect("task model settings");
    let actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([("codex".to_string(), local_tool_test_provider())]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");
    let turn = test_turn_with_selection(noema_providers::ProviderSelectionSnapshot::explicit(
        "foundation_local",
        provider_account_id.clone(),
        "default",
        None,
        Some("agent:primary".to_string()),
    ));

    let result = actor
        .execute_bound_runtime_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                crate::daemon::task_tool::TASK_DELEGATE_TOOL,
                json!({
                    "title": "Preserve the source account",
                    "description": "Verify exact task delegation provenance.",
                    "execution_intent": {
                        "request_markdown": "Verify exact task delegation provenance.",
                        "complexity": "simple",
                        "criteria": [{
                            "description": "The reviewer retains the source provider account."
                        }],
                        "execution_plan_markdown": "Inspect the immutable reviewer selection."
                    }
                }),
            ),
        )
        .await
        .expect("runtime task delegate");

    assert!(result.success, "task delegation failed: {}", result.payload);
    let task_id = result.payload["task"]["task_id"].as_str().expect("task id");
    let task = store
        .get_work_task(&noema_tasks::TaskId::new(task_id).expect("task id"))
        .await
        .expect("read task")
        .expect("created task");
    let contract = task.current_contract.expect("created contract");
    assert_eq!(contract.reviewer_model.provider_kind, "codex");
    assert_eq!(
        contract.reviewer_model.provider_account_id,
        "provider_account:codex:default"
    );
    assert_eq!(
        contract.reviewer_model.model_profile.as_deref(),
        Some("gpt-5.6-luna")
    );
    assert_eq!(contract.reviewer_model.reasoning_effort, None);
    assert_eq!(
        contract.reviewer_model.selection_source.as_deref(),
        Some("agent:task-reviewer")
    );
}

#[tokio::test]
async fn forged_background_call_is_unknown_and_omits_persistence() {
    let actor = test_actor().await;
    let policy = crate::agent_execution::ToolPolicy::for_role(
        crate::agent_execution::ExecutionRole::TaskReviewer,
    );
    let result = actor
        .execute_local_tool_with_policy(
            &test_turn(),
            &AgentPromptIdentity {
                agent_id: "agent:reviewer".to_string(),
                display_name: None,
            },
            &test_tool_call("forged.operation", json!({"secret": "do not persist"})),
            &policy,
        )
        .await;

    assert!(!result.success);
    assert_eq!(
        result.payload["error"],
        "capability operation is unavailable"
    );
    assert_eq!(result.persisted.arguments, None);
    assert_eq!(result.persisted.output, None);
}

#[tokio::test]
async fn known_background_call_denied_by_role_policy_uses_binding_persistence() {
    let actor = test_actor().await;
    let policy = crate::agent_execution::ToolPolicy::for_role(
        crate::agent_execution::ExecutionRole::TaskReviewer,
    );
    let result = actor
        .execute_local_tool_with_policy(
            &test_turn(),
            &AgentPromptIdentity {
                agent_id: "agent:reviewer".to_string(),
                display_name: None,
            },
            &test_tool_call(
                noema_capabilities::web::search::WEB_SEARCH_TOOL,
                json!({"query": "safe", "api_key": "do not persist"}),
            ),
            &policy,
        )
        .await;

    assert!(!result.success);
    assert_eq!(result.payload["error"], "capability invocation was denied");
    assert_eq!(
        result.persisted.arguments,
        Some(json!({"query": "safe", "api_key": "[REDACTED]"}))
    );
    assert_eq!(result.persisted.output, Some(json!({"error": "denied"})));
}

async fn ensure_provider_account_without_web_capabilities(
    store: &noema_store::NoemaStore,
) -> String {
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default Codex provider account");
    account.provider_account_id
}

async fn create_exa_provider_account(
    store: &noema_store::NoemaStore,
    status: noema_providers::ProviderAccountStatus,
) -> String {
    crate::test_support::create_exa_provider_account_for_tests(
        store,
        "Exa runtime test",
        status,
        json!({}),
    )
    .await
    .provider_account_id
}

async fn insert_provider_capability_binding(
    store: &noema_store::NoemaStore,
    tool_name: &str,
    provider_account_id: impl Into<String>,
) {
    crate::test_support::save_provider_capability_assignment_for_tests(
        store,
        tool_name,
        tool_name,
        ProviderCapabilityAccountReference::persisted(provider_account_id),
    )
    .await;
}
