#[tokio::test]
async fn oauth_callback_completion_drains_the_bound_runtime_request() {
    use noema_store::{CapabilityAuthenticationRequestState, NewCapabilityAuthenticationRequest};
    use noema_capabilities::{
        CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
        CapabilityAuthenticationChallengeKind,
    };
    use serde_json::json;

    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    noema_store::test_support::insert_mcp_server(&store, "mcp:docs")
        .await
        .expect("MCP server");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(noema_conversations::NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    let request = store
        .create_capability_authentication_request(
            NewCapabilityAuthenticationRequest {
                owner_human_id: "human:local".to_string(),
                conversation_id: Some(conversation.conversation_id),
                turn_id: Some(turn.turn_id),
                task_id: None,
                run_id: None,
                task_generation: None,
                requesting_agent_id: "agent:primary".to_string(),
                challenge: CapabilityAuthenticationChallenge::new(
                    CapabilityAuthenticationChallengeKind::Reauthenticate,
                    CapabilityAuthenticationAuthorityKind::McpServer,
                    "mcp:docs",
                    "generation:created",
                )
                .expect("challenge"),
                capability_name: "mcp.docs.search".to_string(),
                operation_token: "exact-token".to_string(),
                input_schema: json!({ "type": "object" }),
                protected_arguments_ref: "a".repeat(32),
                arguments_sha256: "b".repeat(64),
                provider_selection_digest: "c".repeat(64),
                output_index: 0,
                call_id: Some("call:auth".to_string()),
                provider_call_id: None,
                provider_name: None,
                governed_action: None,
                result_context: json!({"route":"synthetic"}),
            },
            None,
        )
        .await
        .expect("authentication request");
    store
        .begin_capability_authentication(&request.request_id, request.revision, "human:local", "a")
        .await
        .expect("begin authentication");
    let environment = crate::test_support::test_environment();
    let artifacts = crate::test_support::artifact_operations_for_environment(&store, &environment)
        .expect("artifact operations");
    let runtime = noema_runtime::contract_test_support::spawn_runtime_with_provider(
        noema_runtime::contract_test_support::fixed_response_provider("continued"),
        store.clone(),
        artifacts,
    )
    .await
    .expect("runtime");
    let state = GraphqlState::for_tests_with_store(store.clone())
        .with_mcp_operations(Arc::new(McpBoundaryOperations::without_runtime()))
        .with_runtime(runtime)
        .with_mcp_oauth_callback_url("http://localhost/mcp/oauth/callback");

    assert!(
        super::super::mcp::require_exact_oauth_callback(
            &state,
            "http://127.0.0.1:4444/mcp/oauth/callback"
        )
        .is_err()
    );
    super::super::mcp::require_exact_oauth_callback(
        &state,
        "http://localhost/mcp/oauth/callback",
    )
    .expect("exact callback target");

    super::super::mcp::complete_mcp_server_oauth_setup(
        &state,
        "a",
        "http://localhost/mcp/oauth/callback?attemptId=a&code=x",
    )
    .await
    .expect("OAuth callback");

    let stored = store
        .get_capability_authentication_request(&request.request_id, request.revision)
        .await
        .expect("read authentication request")
        .expect("authentication request");
    assert_eq!(stored.state, CapabilityAuthenticationRequestState::Superseded);
}
