use super::*;
use noema_capabilities::PersistedCapabilityPayload;
use noema_providers::ProviderSelectionSnapshot;
use std::collections::HashMap;

#[tokio::test]
async fn uncertain_foreground_action_fails_with_a_durable_non_retry_notice() {
    let store = crate::test_support::test_store().await;
    let provider = crate::contract_test_support::fixed_response_provider("unexpected retry");
    let mut actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([("codex".to_string(), provider.clone())]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");
    let (conversation_id, turn_id, user_item_id) =
        crate::contract_test_support::seed_authorization_source(&store, "Do the external action")
            .await;
    let selection = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-test",
        None,
        Some("test".to_string()),
    );
    let model_tools = ModelTools::empty(crate::agent_execution::ExecutionRole::PrimaryConversation);
    let turn = SuccessfulProviderTurn {
        conversation_id: conversation_id.clone(),
        turn_id,
        turn_index: 1,
        user_item_id,
        user_input: "Do the external action".to_string(),
        task_id: None,
        task_run_id: None,
        task_run_fence: None,
        task_terminal_contract: None,
        cwd: None,
        provider_kind: "codex".to_string(),
        model: Some("gpt-test".to_string()),
        reasoning_effort: None,
        provider_route: crate::test_support::provider_route(selection, provider),
        initial_stream_id: "stream:test".to_string(),
        response: GenerateResponse::final_text("", "codex", "gpt-test"),
        agent_identity: AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        },
        runtime_environment: current_runtime_environment(None),
        tool_capabilities: ProviderToolCapabilities::default(),
        initial_model_tools: model_tools.clone(),
        continuation_model_tools: model_tools,
        initial_provider_input: GenerateInput::Text("test".to_string()),
    };
    let uncertain = LocalToolResult {
        call_id: Some("call:test".to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "external.test".to_string(),
        arguments: json!({}),
        persisted: PersistedCapabilityPayload::omitted(),
        persisted_output_source: None,
        success: false,
        side_effect: false,
        payload: json!({"error": "capability outcome is uncertain"}),
        requires_provider_continuation: false,
        blocked_action_request: None,
        blocked_authentication_id: None,
        blocked_outcome_uncertain: true,
        pending_interaction_id: None,
        kind: LocalToolKind::Gateway,
    };
    let state = ForegroundContinuationState {
        next_output_index: 1,
        task_handoff: false,
        all_local_tool_results: vec![uncertain],
        progress_tracker: ContinuationProgressTracker::new(&turn.user_input),
        continuation_context: ContinuationContext::from_provider_input(
            turn.initial_provider_input.clone(),
        ),
        continuation_tool_results: Vec::new(),
        waiting_for_interaction: false,
        citation_sources: Default::default(),
    };
    let (item_tx, _item_rx) = mpsc::unbounded_channel();

    assert!(
        actor
            .run_foreground_continuations(
                &turn,
                state,
                &item_tx,
                &TurnTiming::new(&turn.conversation_id, &turn.turn_id, 1, None),
            )
            .await
            .expect("terminal handling")
    );
    let runtime = store
        .conversation_runtime_status(&conversation_id)
        .await
        .expect("runtime status")
        .expect("status");
    assert_eq!(runtime.turn_status, ConversationTurnStatus::Failed);
    assert_eq!(runtime.agent_status, PersistedAgentStatus::Error);
    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("items");
    let notices = items
        .iter()
        .filter(|item| item.kind == ConversationItemKind::ErrorNotice)
        .collect::<Vec<_>>();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].payload_json["recoverable"], false);
    assert!(
        notices[0]
            .content_text
            .as_deref()
            .is_some_and(|text| text.contains("avoid a duplicate"))
    );
    assert!(
        !items
            .iter()
            .any(|item| item.kind == ConversationItemKind::AssistantText)
    );
}
