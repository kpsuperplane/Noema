#[tokio::test]
async fn oauth_callback_completion_drains_the_bound_runtime_request() {
    use noema_store::{McpAuthenticationRequestState, NewMcpAuthenticationRequest};
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
        .create_mcp_authentication_request(
            NewMcpAuthenticationRequest {
                owner_human_id: "human:local".to_string(),
                conversation_id: Some(conversation.conversation_id),
                turn_id: Some(turn.turn_id),
                task_id: None,
                run_id: None,
                task_generation: None,
                requesting_agent_id: "agent:primary".to_string(),
                mcp_server_id: "mcp:docs".to_string(),
                capability_name: "mcp.docs.search".to_string(),
                operation_token: "exact-token".to_string(),
                input_schema: json!({ "type": "object" }),
                arguments: json!({ "query": "private" }),
                output_index: 0,
                call_id: Some("call:auth".to_string()),
                provider_call_id: None,
                provider_name: None,
                governed_action: None,
            },
            None,
        )
        .await
        .expect("authentication request");
    store
        .begin_mcp_authentication(&request.request_id, request.revision, "human:local", "a")
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
        .with_runtime(runtime);

    super::super::mcp::complete_mcp_server_oauth_setup(&state, "a", "http://localhost/?code=x")
    .await
    .expect("OAuth callback");

    let stored = store
        .get_mcp_authentication_request(&request.request_id, request.revision)
        .await
        .expect("read authentication request")
        .expect("authentication request");
    assert_eq!(stored.state, McpAuthenticationRequestState::Superseded);
}
