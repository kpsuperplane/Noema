#[test]
fn persisted_native_memory_search_keeps_references_but_omits_snippets() {
    let result = super::LocalToolResult {
        call_id: Some("call-1".into()),
        provider_call_id: None,
        provider_name: None,
        name: noema_memory::NATIVE_SEARCH_MEMORY_TOOL_NAME.into(),
        arguments: json!({"query": "Alice"}),
        persisted: noema_capabilities::PersistedCapabilityPayload {
            arguments: Some(json!({"query": "Alice"})),
            output: Some(json!({"pages": [{
                "id": "memory:human:people.md",
                "path": "people.md",
                "hash": "abc"
            }]})),
        },
        success: true,
        payload: json!({
            "pages": [{
                "id": "memory:human:people.md",
                "path": "people.md",
                "title": "People",
                "hash": "abc",
                "snippet": "private search excerpt"
            }]
        }),
        requires_provider_continuation: true,
        blocked_action_id: None,
        blocked_authentication_id: None,
        blocked_outcome_uncertain: false,
        kind: super::LocalToolKind::Memory,
    };
    let GenerateActionItem::ToolResult { payload, .. } =
        super::local_tool_result_action_item(&result)
    else {
        panic!("expected tool result");
    };
    assert_eq!(
        payload,
        json!({"pages": [{
            "id": "memory:human:people.md",
            "path": "people.md",
            "hash": "abc"
        }]})
    );
}

#[test]
fn uncertain_gateway_failure_stops_provider_continuation() {
    let result = super::gateway_failure_result(
        &test_tool_call(TEST_CAPABILITY_NAME, json!({"body": "ambiguous"})),
        noema_capabilities::CapabilityDispatchFailure {
            error: CapabilityError::OutcomeUncertain,
            persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
        },
    );
    assert!(!result.success);
    assert!(!result.requires_provider_continuation);
    assert!(result.is_blocked());
    assert_eq!(result.payload, json!({"error": "capability outcome is uncertain"}));
}

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
    let mut turn = test_turn();
    let current_route = actor
        .resolve_primary_provider()
        .await
        .expect("primary route");
    turn.provider_kind = current_route.selection().provider_kind.clone();
    turn.model = current_route.selection().model_profile.clone();
    turn.provider_route = Arc::new(current_route);
    let (conversation_id, turn_id, item_id) =
        crate::contract_test_support::seed_authorization_source(
            &actor.store,
            "Write the exact body.",
        )
        .await;
    turn.conversation_id = conversation_id;
    turn.turn_id = turn_id;
    turn.user_item_id = item_id;
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
    assert_eq!(
        action.authorization_context["context"]["messages"][0]["text"],
        "Write the exact body."
    );
    assert_eq!(
        action.authorization_context["destination"],
        json!({
            "service_id": "mcp",
            "connection_id": "mcp:docs",
            "revision": "generation:created",
        })
    );
    assert_eq!(
        action.authorization_context["provider_selection_digest"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert!(
        invoker
            .invocations
            .lock()
            .expect("invocation lock")
            .is_empty()
    );

    noema_store::test_support::insert_mcp_server(&actor.store, "mcp:docs")
        .await
        .expect("server");
    actor.capability_bindings = static_capability_binding_source(turn.initial_model_tools.bindings.clone());
    actor.capability_invokers = Arc::from([
        noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            Arc::new(RecordingCapabilityInvoker::returning(Err(
                CapabilityError::AuthenticationRequired {
                    challenge: noema_capabilities::CapabilityAuthenticationChallenge::new(
                        noema_capabilities::CapabilityAuthenticationChallengeKind::Reauthenticate,
                        noema_capabilities::CapabilityAuthenticationAuthorityKind::McpServer,
                        "mcp:docs",
                        "generation:created",
                    )
                    .expect("challenge"),
                },
            ))),
        ),
    ]);
    let resolved = actor
        .resolve_governed_action(
            &action_id,
            action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("approve action into authentication pause");
    assert_eq!(
        resolved.state,
        noema_store::GovernedActionState::AwaitingAuthentication
    );
    assert_eq!(
        actor
            .store
            .list_pending_capability_authentication_requests(
                "human:local",
                Some(&turn.conversation_id),
                None,
                10,
            )
            .await
            .expect("authentication request")
            .len(),
        1
    );

    let observed_url = "https://example.com/public".to_string();
    actor
        .store
        .record_observed_urls(
            noema_store::ObservedUrlSource::SearchResult,
            "tool_call:search",
            std::slice::from_ref(&observed_url),
        )
        .await
        .expect("record observed URL");
    turn.initial_model_tools = test_governed_web_fetch_model_tools();
    let fetched = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                noema_capabilities::web::fetch::WEB_FETCH_TOOL,
                json!({"url": observed_url}),
            ),
        )
        .await;
    assert!(
        fetched.success,
        "bare exact observed URL should bypass review"
    );

    let augmented = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                noema_capabilities::web::fetch::WEB_FETCH_TOOL,
                json!({
                    "url": "https://example.com/public",
                    "reason": "Read the public source.",
                    "max_chars": 12_000,
                }),
            ),
        )
        .await;
    assert!(
        augmented.success,
        "local-only fetch controls should not force review of an observed URL"
    );
}

#[tokio::test]
async fn approved_foreground_action_resumes_with_its_stored_result() {
    let mut actor = test_actor().await;
    let conversation = actor
        .store
        .get_or_create_primary_conversation("human:local", Some("gpt-test".to_string()), None)
        .await
        .expect("primary conversation");
    let durable_turn = actor
        .store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({"turn_index": 1}),
        })
        .await
        .expect("conversation turn");
    let user_item = actor
        .store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(durable_turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local").expect("human actor"),
            content_text: Some("Fetch this page and summarize it.".to_string()),
            payload_json: json!({}),
            metadata: json!({"turn_index": 1}),
        })
        .await
        .expect("user item");
    let mut turn = test_turn();
    let current_route = actor
        .resolve_primary_provider()
        .await
        .expect("primary route");
    turn.provider_kind = current_route.selection().provider_kind.clone();
    turn.model = current_route.selection().model_profile.clone();
    turn.provider_route = Arc::new(current_route);
    turn.conversation_id = conversation.conversation_id.clone();
    turn.turn_id = durable_turn.turn_id.clone();
    turn.user_item_id = user_item.item_id.clone();
    turn.initial_model_tools = test_governed_web_fetch_model_tools();
    let call = test_tool_call(
        noema_capabilities::web::fetch::WEB_FETCH_TOOL,
        json!({"url": "https://example.com/requires-approval"}),
    );
    let action_turn = crate::daemon::runtime::turn::ProviderActionTurn {
        conversation_id: turn.conversation_id.clone(),
        turn_id: turn.turn_id.clone(),
        turn_index: 1,
        user_item_id: turn.user_item_id.clone(),
        provider: "codex".to_string(),
        model: "gpt-test".to_string(),
        response_phase: "initial",
        usage: None,
        stream_id: Some("stream:test".to_string()),
    };
    let (item_tx, _item_rx) = tokio::sync::mpsc::unbounded_channel();
    actor
        .persist_provider_action_item(
            &action_turn,
            0,
            crate::daemon::runtime::tool_lifecycle::tool_call_action_item(
                &call,
                turn.initial_model_tools
                    .bindings
                    .resolve(&call.name)
                    .and_then(|binding| binding.persist_arguments(&call.payload)),
            ),
            &item_tx,
        )
        .await
        .expect("tool call item");
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
    let action_id = result.blocked_action_id.clone().expect("blocked action id");
    actor
        .persist_provider_action_item(
            &action_turn,
            1,
            super::local_tool_result_action_item(&result),
            &item_tx,
        )
        .await
        .expect("approval request item");
    actor
        .store
        .complete_conversation_turn(&turn.turn_id)
        .await
        .expect("complete original turn");

    let pending_items = actor
        .store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("pending transcript");
    assert!(pending_items.iter().any(|item| {
        item.kind == ConversationItemKind::ApprovalRequest
            && item.payload_json.pointer("/metadata/action/id") == Some(&json!(action_id))
    }));
    assert!(
        pending_items
            .iter()
            .all(|item| { item.kind != ConversationItemKind::ToolResult })
    );

    let resolved = actor
        .resolve_governed_action(
            &action_id,
            1,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("approve action");
    assert_eq!(resolved.state, noema_store::GovernedActionState::Succeeded);

    let resumed_items = actor
        .store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("resumed transcript");
    let tool_result = resumed_items
        .iter()
        .find(|item| item.kind == ConversationItemKind::ToolResult)
        .expect("terminal tool result");
    assert_eq!(
        tool_result
            .payload_json
            .pointer("/metadata/action/provider_call_id"),
        Some(&json!("provider_call:test"))
    );
    assert!(
        tool_result
            .payload_json
            .pointer("/metadata/action/payload/result")
            .is_some_and(serde_json::Value::is_object)
    );
    assert_eq!(
        resumed_items
            .iter()
            .filter(|item| item.kind == ConversationItemKind::AssistantText)
            .filter_map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec!["summarized page"]
    );
    actor
        .resolve_governed_action(
            &action_id,
            1,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("idempotent terminal resolution");
    let replayed_items = actor
        .store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("replayed transcript");
    let count = |kind| {
        replayed_items
            .iter()
            .filter(|item| item.kind == kind)
            .count()
    };
    assert_eq!(
        (
            count(ConversationItemKind::ToolResult),
            count(ConversationItemKind::AssistantText),
            count(ConversationItemKind::UserText),
        ),
        (1, 1, 1)
    );
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
async fn authentication_challenge_creates_one_durable_interruption() {
    let mut actor = test_actor().await;
    noema_store::test_support::insert_mcp_server(&actor.store, "mcp:docs")
        .await
        .expect("MCP server");
    actor.capability_invokers =
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("external:test"),
            Arc::new(RecordingCapabilityInvoker::returning(Err(
                CapabilityError::AuthenticationRequired {
                    challenge: noema_capabilities::CapabilityAuthenticationChallenge::new(
                        noema_capabilities::CapabilityAuthenticationChallengeKind::Reauthenticate,
                        noema_capabilities::CapabilityAuthenticationAuthorityKind::McpServer,
                        "mcp:docs",
                        "generation:created",
                    )
                    .expect("challenge"),
                },
            ))),
        )]);
    let mut turn = test_turn();
    let conversation = actor
        .store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let durable_turn = actor
        .store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    turn.conversation_id = conversation.conversation_id;
    turn.turn_id = durable_turn.turn_id;
    turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
        noema_capabilities::RedactingPayloadSanitizer,
    ));
    let call = test_tool_call(
        TEST_CAPABILITY_NAME,
        json!({"document_id": "document:1", "api_key": "private"}),
    );

    let first = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &call,
        )
        .await;
    let duplicate = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &call,
        )
        .await;

    assert!(!first.success);
    assert!(!first.requires_provider_continuation);
    assert_eq!(
        first.blocked_authentication_id,
        duplicate.blocked_authentication_id
    );
    let requests = actor
        .store
        .list_pending_capability_authentication_requests(
            "human:local",
            Some(&turn.conversation_id),
            None,
            10,
        )
        .await
        .expect("pending authentication requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].protected_arguments_ref.len(), 32);
    assert!(!requests[0].protected_arguments_ref.contains("private"));
    assert_eq!(
        actor
            .capability_auth_arguments
            .load(
                &requests[0].protected_arguments_ref,
                &requests[0].arguments_sha256,
            )
            .expect("protected arguments"),
        call.payload
    );
    assert_eq!(
        first.persisted.arguments,
        Some(json!({"document_id": "document:1", "api_key": "[REDACTED]"}))
    );
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
    let mut turn = test_turn_with_selection(noema_providers::ProviderSelectionSnapshot::explicit(
        "foundation_local",
        provider_account_id.clone(),
        "default",
        None,
        Some("agent:primary".to_string()),
    ));
    let (conversation_id, turn_id, item_id) =
        crate::contract_test_support::seed_authorization_source(&store, "Delegate this task.")
            .await;
    turn.conversation_id = conversation_id;
    turn.turn_id = turn_id;
    turn.user_item_id = item_id;

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
                    "project": {"kind": "none"},
                    "complexity_hint": "difficult",
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
    assert_eq!(contract.complexity, noema_tasks::TaskComplexity::Simple);
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
