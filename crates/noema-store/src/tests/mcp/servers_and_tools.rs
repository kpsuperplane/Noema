use super::{support::*, *};

#[tokio::test]
async fn creates_and_lists_mcp_server_with_discovered_tool() {
    let store = test_store().await;

    let server = store
        .create_mcp_server(McpServerSeed {
            mcp_server_id: "mcp_server:local-test".to_string(),
            display_name: "Local Test".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({"command": "test-mcp"}),
        })
        .await
        .expect("create server");

    assert_eq!(server.mcp_server_id, "mcp_server:local-test");
    assert_eq!(server.display_name, "Local Test");
    assert_eq!(server.transport_kind, McpTransportKind::Stdio);
    assert_eq!(server.safe_config, json!({"command": "test-mcp"}));
    assert!(!server.enabled);
    assert_eq!(server.health_status, McpServerHealthStatus::Unknown);
    assert_eq!(server.auth_status, McpServerAuthStatus::None);
    assert_eq!(server.tool_count, 0);

    let tool = store
        .upsert_discovered_mcp_tool(McpToolSeed {
            mcp_tool_id: "mcp_tool:local-test:read".to_string(),
            mcp_server_id: "mcp_server:local-test".to_string(),
            name: "read".to_string(),
            description: Some("Read metadata".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: Some(json!({"type": "object"})),
            annotations: json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint:local-test:read:v1".to_string(),
        })
        .await
        .expect("upsert tool");

    assert_eq!(tool.mcp_tool_id, "mcp_tool:local-test:read");
    assert_eq!(tool.mcp_server_id, "mcp_server:local-test");
    assert_eq!(tool.name, "read");
    assert_eq!(tool.description.as_deref(), Some("Read metadata"));
    assert_eq!(tool.input_schema, json!({"type": "object"}));
    assert_eq!(tool.output_schema, Some(json!({"type": "object"})));
    assert_eq!(tool.annotations, json!({"readOnlyHint": true}));
    assert_eq!(tool.metadata_fingerprint, "fingerprint:local-test:read:v1");
    assert!(!tool.discovered_at.is_empty());

    let servers = store.list_mcp_servers().await.expect("list servers");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].mcp_server_id, "mcp_server:local-test");
    assert_eq!(servers[0].tool_count, 1);

    let tools = store
        .list_mcp_tools_for_server("mcp_server:local-test")
        .await
        .expect("list tools");
    assert_eq!(tools, vec![tool]);
}

#[tokio::test]
async fn mcp_server_status_can_be_updated_after_setup() {
    let store = test_store().await;
    store
        .create_mcp_server(McpServerSeed {
            mcp_server_id: "mcp:setup".to_string(),
            display_name: "Setup".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({ "command": "test-mcp" }),
        })
        .await
        .expect("server");

    let updated = store
        .update_mcp_server_setup_status(
            "mcp:setup",
            McpServerHealthStatus::Healthy,
            McpServerAuthStatus::NeedsAuth,
        )
        .await
        .expect("updated server");

    assert_eq!(updated.health_status, McpServerHealthStatus::Healthy);
    assert_eq!(updated.auth_status, McpServerAuthStatus::NeedsAuth);
}

#[tokio::test]
async fn delete_mcp_server_removes_server_tools_and_calibrations() {
    let store = test_store_with_mcp_tool().await;
    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save calibration");

    let deleted = store
        .delete_mcp_server("mcp_server:google")
        .await
        .expect("delete server");

    assert!(deleted);
    assert!(
        store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("get server")
            .is_none()
    );
    assert!(
        store
            .list_mcp_tools_for_server("mcp_server:google")
            .await
            .expect("list tools")
            .is_empty()
    );
    assert!(
        store
            .get_tool_calibration("mcp_tool:google:read_doc")
            .await
            .expect("get calibration")
            .is_none()
    );
    assert!(
        !store
            .delete_mcp_server("mcp_server:google")
            .await
            .expect("delete missing")
    );
}

#[tokio::test]
async fn rediscovered_tool_metadata_invalidates_reviewed_calibration() {
    let store = test_store_with_mcp_tool().await;
    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save calibration");

    store
        .upsert_discovered_mcp_tool(McpToolSeed {
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}}),
            ..google_tool("read_doc", json!({"readOnlyHint": true}), "fingerprint_2")
        })
        .await
        .expect("refresh tool metadata");

    let calibration = store
        .get_tool_calibration("mcp_tool:google:read_doc")
        .await
        .expect("get calibration")
        .expect("calibration exists");
    assert_eq!(calibration.status, McpCalibrationStatus::NeedsReview);
    assert_eq!(calibration.reviewed_by, None);
    assert_eq!(calibration.reviewed_metadata_fingerprint, None);
    assert!(
        !store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("get server")
            .expect("server")
            .enabled
    );
}

#[tokio::test]
async fn same_fingerprint_tool_move_recomputes_old_and_new_server_enabled_state() {
    let store = test_store_with_mcp_tool().await;
    store
        .create_mcp_server(McpServerSeed {
            mcp_server_id: "mcp_server:drive".to_string(),
            display_name: "Drive".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({}),
        })
        .await
        .expect("create second server");
    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save calibration");

    assert!(
        store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("get old server")
            .expect("old server")
            .enabled
    );
    assert!(
        !store
            .get_mcp_server("mcp_server:drive")
            .await
            .expect("get new server")
            .expect("new server")
            .enabled
    );

    store
        .upsert_discovered_mcp_tool(McpToolSeed {
            mcp_server_id: "mcp_server:drive".to_string(),
            ..google_tool("read_doc", json!({"readOnlyHint": true}), "fingerprint_1")
        })
        .await
        .expect("move tool to second server");

    assert!(
        !store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("get old server")
            .expect("old server")
            .enabled
    );
    assert!(
        store
            .get_mcp_server("mcp_server:drive")
            .await
            .expect("get new server")
            .expect("new server")
            .enabled
    );
    let calibration = store
        .get_tool_calibration("mcp_tool:google:read_doc")
        .await
        .expect("get calibration")
        .expect("calibration");
    assert_eq!(calibration.status, McpCalibrationStatus::Ready);
    assert_eq!(
        calibration.reviewed_metadata_fingerprint.as_deref(),
        Some("fingerprint_1")
    );
}

#[tokio::test]
async fn mcp_tool_upsert_replaces_discovered_metadata() {
    let store = test_store().await;
    store
        .create_mcp_server(google_server())
        .await
        .expect("create server");

    store
        .upsert_discovered_mcp_tool(McpToolSeed {
            description: Some("Old description".to_string()),
            ..google_tool("read", json!({}), "fingerprint:v1")
        })
        .await
        .expect("upsert old tool");

    let updated = store
        .upsert_discovered_mcp_tool(McpToolSeed {
            description: Some("New description".to_string()),
            annotations: json!({"destructiveHint": false}),
            ..google_tool("read", json!({}), "fingerprint:v2")
        })
        .await
        .expect("upsert replacement tool");

    assert_eq!(updated.description.as_deref(), Some("New description"));
    assert_eq!(updated.metadata_fingerprint, "fingerprint:v2");
    assert_eq!(updated.annotations, json!({"destructiveHint": false}));
    let tools = store
        .list_mcp_tools_for_server("mcp_server:google")
        .await
        .expect("list tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0], updated);
}
