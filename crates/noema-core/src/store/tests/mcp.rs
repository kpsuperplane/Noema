use serde_json::json;

use super::test_store;
use crate::{
    McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind,
    McpTrustClassification, NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    mcp::{McpToolIneligibility, mcp_tool_ineligibility},
};

#[tokio::test]
async fn mcp_control_plane_tables_bootstrap() {
    let store = test_store().await;

    assert_eq!(store.schema_version().await.expect("schema version"), 1);
    store
        .with_connection(|conn| {
            conn.execute_batch(
                r#"
            INSERT INTO mcp_servers (
              mcp_server_id, display_name, transport_kind, safe_config_json,
              auth_status, health_status, enabled
            )
            VALUES (
              'mcp_server:local-test', 'Local Test', 'stdio', '{}',
              'none', 'unknown', 0
            );

            INSERT INTO mcp_tools (
              mcp_tool_id, mcp_server_id, name, description, input_schema_json,
              output_schema_json, annotations_json, metadata_fingerprint, discovered_at
            )
            VALUES (
              'mcp_tool:local-test:read', 'mcp_server:local-test', 'read',
              'Read metadata', '{}', NULL, '{}', 'fingerprint:local-test:read',
              '2026-06-30T00:00:00Z'
            );

            INSERT INTO tool_calibrations (
              calibration_id, mcp_tool_id, read_classification,
              write_classification, export_classification, status
            )
            VALUES (
              'tool_calibration:local-test:read', 'mcp_tool:local-test:read',
              'trusted', 'none', 'none', 'needs_review'
            );

            INSERT INTO approval_requests (
              approval_id, action_summary, tool_invocation_id, mcp_server_id,
              mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
              destination_summary, data_source_summary, source_owner_identity,
              source_owner_trust, destination_owner_identity, destination_owner_trust,
              export_summary, payload_preview_json, status
            )
            VALUES (
              'approval:local-test', 'Approve local test MCP call',
              'tool_invocation:local-test', 'mcp_server:local-test',
              'mcp_tool:local-test:read', 'agent:primary', 'human:local',
              'human:local', 'Local test recipient', 'Local test MCP result',
              'kevin@example.com', 'trusted', 'person@example.com', 'untrusted',
              'Local test data leaves the MCP boundary', '{}', 'pending'
            );
            "#,
            )?;
            Ok(())
        })
        .await
        .expect("mcp control-plane tables should accept valid rows");
}

#[tokio::test]
async fn mcp_server_transport_constraint_rejects_removed_sse_kind() {
    let store = test_store().await;

    let error = store
        .with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO mcp_servers (
                  mcp_server_id, display_name, transport_kind, safe_config_json,
                  auth_status, health_status, enabled
                ) VALUES (
                  'mcp_server:removed-sse', 'Removed SSE', 'sse', '{}',
                  'none', 'unknown', 0
                )
                "#,
                [],
            )?;
            Ok(())
        })
        .await
        .expect_err("removed SSE transport should violate the schema constraint");

    assert!(error.to_string().contains("transport_kind"));
}

#[tokio::test]
async fn export_decision_creates_manual_approval_request() {
    let store = test_store().await;

    let approval = store
        .create_mcp_approval_request(NewMcpApprovalRequest {
            approval_id: "approval:mcp:1".to_string(),
            action_summary: "Share Google Doc".to_string(),
            tool_invocation_id: "tool_invocation:mcp:1".to_string(),
            mcp_server_id: Some("mcp_server:google".to_string()),
            mcp_tool_id: Some("mcp_tool:google:share_doc".to_string()),
            requester_actor_id: "agent:primary".to_string(),
            owner_scope_id: "human:local".to_string(),
            active_scope_id: "human:local".to_string(),
            destination_summary: "person@example.com".to_string(),
            data_source_summary: "Google Doc: Project plan".to_string(),
            source_owner_identity: "kevin@example.com".to_string(),
            source_owner_trust: "trusted".to_string(),
            destination_owner_identity: "person@example.com".to_string(),
            destination_owner_trust: "untrusted".to_string(),
            export_summary: "Document title and share permission".to_string(),
            payload_preview: json!({
                "recipient": "person@example.com",
                "api_token": "secret-token",
                "headers": {
                    "Authorization": "Bearer secret",
                    "cookie": "session=secret"
                }
            }),
        })
        .await
        .expect("approval");

    assert_eq!(approval.approval_id, "approval:mcp:1");
    assert_eq!(approval.action_summary, "Share Google Doc");
    assert_eq!(approval.tool_invocation_id, "tool_invocation:mcp:1");
    assert_eq!(approval.mcp_server_id.as_deref(), Some("mcp_server:google"));
    assert_eq!(
        approval.mcp_tool_id.as_deref(),
        Some("mcp_tool:google:share_doc")
    );
    assert_eq!(approval.requester_actor_id, "agent:primary");
    assert_eq!(approval.owner_scope_id, "human:local");
    assert_eq!(approval.active_scope_id, "human:local");
    assert_eq!(approval.destination_summary, "person@example.com");
    assert_eq!(approval.data_source_summary, "Google Doc: Project plan");
    assert_eq!(approval.source_owner_identity, "kevin@example.com");
    assert_eq!(approval.source_owner_trust, "trusted");
    assert_eq!(approval.destination_owner_identity, "person@example.com");
    assert_eq!(approval.destination_owner_trust, "untrusted");
    assert_eq!(
        approval.export_summary,
        "Document title and share permission"
    );
    assert_eq!(
        approval.payload_preview,
        json!({
            "recipient": "person@example.com",
            "api_token": "[redacted]",
            "headers": {
                "Authorization": "[redacted]",
                "cookie": "[redacted]"
            }
        })
    );
    assert_eq!(approval.status, "pending");

    let pending = store
        .list_mcp_approval_requests(Some("pending"))
        .await
        .expect("pending approvals");
    assert_eq!(pending, vec![approval]);
}

#[tokio::test]
async fn creates_and_lists_mcp_server_with_discovered_tool() {
    let store = test_store().await;

    let server = store
        .create_mcp_server(NewMcpServer {
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
        .upsert_discovered_mcp_tool(NewMcpTool {
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
        .create_mcp_server(NewMcpServer {
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
async fn calibration_blocks_unresolved_ownership_until_reviewed() {
    let store = test_store_with_mcp_tool().await;

    let calibration = store
        .save_tool_calibration(google_read_calibration(
            McpTrustClassification::Mixed,
            McpTrustClassification::None,
            McpCalibrationStatus::BlockedUnresolvedOwnership,
            "fingerprint_1",
        ))
        .await
        .expect("save calibration");

    assert_eq!(calibration.calibration_id, "tool_calibration:read_doc");
    assert_eq!(calibration.mcp_tool_id, "mcp_tool:google:read_doc");
    assert_eq!(
        calibration.read_classification,
        McpTrustClassification::Mixed
    );
    assert_eq!(
        calibration.status,
        McpCalibrationStatus::BlockedUnresolvedOwnership
    );
    assert_eq!(calibration.reviewed_by.as_deref(), Some("human:local"));
    assert_eq!(
        calibration.reviewed_metadata_fingerprint.as_deref(),
        Some("fingerprint_1")
    );

    let persisted = store
        .get_tool_calibration("mcp_tool:google:read_doc")
        .await
        .expect("get calibration")
        .expect("calibration exists");
    assert_eq!(persisted, calibration);
}

#[tokio::test]
async fn ready_mixed_calibration_is_rejected_without_ownership_enforcement() {
    let store = test_store_with_mcp_tool().await;

    let error = store
        .save_tool_calibration(google_read_calibration(
            McpTrustClassification::Mixed,
            McpTrustClassification::None,
            McpCalibrationStatus::Ready,
            "fingerprint_1",
        ))
        .await
        .expect_err("ready mixed calibration should fail closed");

    assert!(
        error
            .to_string()
            .contains("unsupported without ownership enforcement")
    );
}

#[tokio::test]
async fn persisted_ready_mixed_calibration_is_blocked_on_read_and_execution() {
    let store = test_store_with_mcp_tool().await;
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE mcp_servers SET enabled = 1, health_status = 'healthy' WHERE mcp_server_id = 'mcp_server:google'",
                [],
            )?;
            conn.execute(
                r#"
                INSERT INTO tool_calibrations (
                  calibration_id, mcp_tool_id, read_classification, write_classification,
                  export_classification, status, reviewed_by, reviewed_metadata_fingerprint
                ) VALUES (
                  'tool_calibration:read_doc', 'mcp_tool:google:read_doc', 'mixed', 'none',
                  'none', 'ready', 'human:local', 'fingerprint_1'
                )
                "#,
                [],
            )?;
            Ok(())
        })
        .await
        .expect("insert legacy ready mixed calibration");

    let calibration = store
        .get_tool_calibration("mcp_tool:google:read_doc")
        .await
        .expect("get calibration")
        .expect("calibration");
    assert_eq!(
        calibration.status,
        McpCalibrationStatus::BlockedUnresolvedOwnership
    );

    let server = store
        .get_mcp_server("mcp_server:google")
        .await
        .expect("get server")
        .expect("server");
    assert!(!server.enabled, "stored enabled bit must not control reads");

    let tool = store
        .list_mcp_tools_for_server("mcp_server:google")
        .await
        .expect("tools")
        .into_iter()
        .next()
        .expect("tool");
    let mut stale_server = server;
    stale_server.enabled = true;
    let mut stale_calibration = calibration;
    stale_calibration.status = McpCalibrationStatus::Ready;
    assert_eq!(
        mcp_tool_ineligibility(&stale_server, &tool, Some(&stale_calibration)),
        Some(McpToolIneligibility::ToolNotCalibrated)
    );
}

#[tokio::test]
async fn ready_calibration_requires_at_least_one_non_none_classification() {
    let store = test_store_with_mcp_tool().await;

    let error = store
        .save_tool_calibration(google_read_calibration(
            McpTrustClassification::None,
            McpTrustClassification::None,
            McpCalibrationStatus::Ready,
            "fingerprint_1",
        ))
        .await
        .expect_err("ready calibration with no allowed axes should fail");

    assert!(error.to_string().contains("at least one non-none"));
}

#[tokio::test]
async fn ready_calibration_requires_current_metadata_fingerprint() {
    let store = test_store_with_mcp_tool().await;

    let error = store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_2"))
        .await
        .expect_err("stale reviewed fingerprint should fail");

    assert!(error.to_string().contains("does not match current"));
}

#[tokio::test]
async fn ready_calibration_enables_mcp_server() {
    let store = test_store_with_mcp_tool().await;

    let before = store
        .get_mcp_server("mcp_server:google")
        .await
        .expect("get server")
        .expect("server");
    assert!(!before.enabled);

    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save ready calibration");

    let after = store
        .get_mcp_server("mcp_server:google")
        .await
        .expect("get server")
        .expect("server");
    assert!(after.enabled);
}

#[tokio::test]
async fn batch_calibration_rolls_back_saved_rows_and_enabled_state_on_mid_batch_failure() {
    let store = test_store_with_mcp_tool().await;
    upsert_google_tool(&store, "write_doc", json!({}), "fingerprint_write").await;

    let error = store
        .save_tool_calibrations(vec![
            ready_google_calibration(
                "shared",
                "read_doc",
                McpTrustClassification::Trusted,
                McpTrustClassification::None,
                "fingerprint_1",
            ),
            ready_google_calibration(
                "shared",
                "write_doc",
                McpTrustClassification::None,
                McpTrustClassification::Trusted,
                "fingerprint_write",
            ),
        ])
        .await
        .expect_err("duplicate calibration id should fail during batch save");

    assert!(error.to_string().contains("duplicate calibration id"));
    assert!(
        store
            .get_tool_calibration("mcp_tool:google:read_doc")
            .await
            .expect("read calibration")
            .is_none()
    );
    assert!(
        store
            .get_tool_calibration("mcp_tool:google:write_doc")
            .await
            .expect("write calibration")
            .is_none()
    );
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
async fn batch_calibration_rolls_back_after_partial_write_failure() {
    let store = test_store_with_mcp_tool().await;
    upsert_google_tool(&store, "write_doc", json!({}), "fingerprint_write").await;

    let error = store
        .save_tool_calibrations(vec![
            ready_google_calibration(
                "read_doc",
                "read_doc",
                McpTrustClassification::Trusted,
                McpTrustClassification::None,
                "fingerprint_1",
            ),
            ready_google_calibration(
                "write_doc",
                "write_doc",
                McpTrustClassification::None,
                McpTrustClassification::Trusted,
                "stale_fingerprint",
            ),
        ])
        .await
        .expect_err("stale second calibration should fail whole batch");

    assert!(error.to_string().contains("does not match current"));
    assert!(
        store
            .get_tool_calibration("mcp_tool:google:read_doc")
            .await
            .expect("read calibration")
            .is_none()
    );
    assert!(
        store
            .get_tool_calibration("mcp_tool:google:write_doc")
            .await
            .expect("write calibration")
            .is_none()
    );
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
async fn calibration_id_cannot_move_between_tools() {
    let store = test_store_with_mcp_tool().await;
    upsert_google_tool(
        &store,
        "write_doc",
        json!({"destructiveHint": true}),
        "fingerprint_write",
    )
    .await;

    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save first calibration");

    let error = store
        .save_tool_calibration(ready_google_calibration(
            "read_doc",
            "write_doc",
            McpTrustClassification::None,
            McpTrustClassification::Trusted,
            "fingerprint_write",
        ))
        .await
        .expect_err("calibration id cannot move to another tool");

    assert!(error.to_string().contains("already belongs"));
}

#[tokio::test]
async fn rediscovered_tool_metadata_invalidates_reviewed_calibration() {
    let store = test_store_with_mcp_tool().await;
    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save calibration");

    store
        .upsert_discovered_mcp_tool(NewMcpTool {
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
        .create_mcp_server(NewMcpServer {
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
        .upsert_discovered_mcp_tool(NewMcpTool {
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

async fn test_store_with_mcp_tool() -> crate::NoemaStore {
    let store = test_store().await;
    store
        .create_mcp_server(google_server())
        .await
        .expect("create server");
    upsert_google_tool(
        &store,
        "read_doc",
        json!({"readOnlyHint": true}),
        "fingerprint_1",
    )
    .await;
    store
}

fn google_server() -> NewMcpServer {
    NewMcpServer {
        mcp_server_id: "mcp_server:google".to_string(),
        display_name: "Google".to_string(),
        transport_kind: McpTransportKind::Stdio,
        safe_config: json!({}),
    }
}

fn google_tool(name: &str, annotations: serde_json::Value, fingerprint: &str) -> NewMcpTool {
    NewMcpTool {
        mcp_tool_id: format!("mcp_tool:google:{name}"),
        mcp_server_id: "mcp_server:google".to_string(),
        name: name.to_string(),
        description: Some(name.replace('_', " ")),
        input_schema: json!({"type": "object"}),
        output_schema: None,
        annotations,
        metadata_fingerprint: fingerprint.to_string(),
    }
}

async fn upsert_google_tool(
    store: &crate::NoemaStore,
    name: &str,
    annotations: serde_json::Value,
    fingerprint: &str,
) {
    store
        .upsert_discovered_mcp_tool(google_tool(name, annotations, fingerprint))
        .await
        .expect("upsert tool");
}

fn ready_google_read_calibration(reviewed_metadata_fingerprint: &str) -> NewToolCalibration {
    google_read_calibration(
        McpTrustClassification::Trusted,
        McpTrustClassification::None,
        McpCalibrationStatus::Ready,
        reviewed_metadata_fingerprint,
    )
}

fn ready_google_calibration(
    calibration_name: &str,
    tool_name: &str,
    read_classification: McpTrustClassification,
    write_classification: McpTrustClassification,
    reviewed_metadata_fingerprint: &str,
) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: format!("tool_calibration:{calibration_name}"),
        mcp_tool_id: format!("mcp_tool:google:{tool_name}"),
        read_classification,
        write_classification,
        export_classification: McpTrustClassification::None,
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(reviewed_metadata_fingerprint.to_string()),
    }
}

fn google_read_calibration(
    read_classification: McpTrustClassification,
    write_classification: McpTrustClassification,
    status: McpCalibrationStatus,
    reviewed_metadata_fingerprint: &str,
) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: "tool_calibration:read_doc".to_string(),
        mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
        read_classification,
        write_classification,
        export_classification: McpTrustClassification::None,
        status,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(reviewed_metadata_fingerprint.to_string()),
    }
}

#[tokio::test]
async fn mcp_tool_upsert_replaces_discovered_metadata() {
    let store = test_store().await;
    store
        .create_mcp_server(google_server())
        .await
        .expect("create server");

    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            description: Some("Old description".to_string()),
            ..google_tool("read", json!({}), "fingerprint:v1")
        })
        .await
        .expect("upsert old tool");

    let updated = store
        .upsert_discovered_mcp_tool(NewMcpTool {
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

#[tokio::test]
async fn mcp_control_plane_schema_rejects_invalid_enum_values() {
    let store = test_store().await;

    let error = store
        .with_connection(|conn| {
            conn.execute(
                r#"
            INSERT INTO mcp_servers (
              mcp_server_id, display_name, transport_kind, safe_config_json,
              auth_status, health_status, enabled
            )
            VALUES (
              'mcp_server:invalid-transport', 'Invalid Transport', 'websocket',
              '{}', 'none', 'unknown', 0
            )
            "#,
                [],
            )?;
            Ok(())
        })
        .await
        .expect_err("invalid MCP transport should be rejected");

    assert!(
        error.to_string().contains("transport_kind") || error.to_string().contains("websocket"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn approval_request_schema_rejects_invalid_status() {
    let store = test_store().await;

    let error = store
        .with_connection(|conn| {
            conn.execute(
                r#"
            INSERT INTO approval_requests (
              approval_id, action_summary, tool_invocation_id, requester_actor_id,
              owner_scope_id, active_scope_id, destination_summary, data_source_summary,
              source_owner_identity, source_owner_trust, destination_owner_identity,
              destination_owner_trust, export_summary, payload_preview_json, status
            )
            VALUES (
              'approval:invalid-status', 'Invalid approval status test',
              'tool_invocation:invalid-status', 'agent:primary', 'human:local',
              'human:local', 'Destination', 'Data source', 'kevin@example.com',
              'trusted', 'person@example.com', 'untrusted', 'Exported data',
              '{}', 'deferred'
            )
            "#,
                [],
            )?;
            Ok(())
        })
        .await
        .expect_err("invalid approval status should be rejected");

    assert!(
        error.to_string().contains("status") || error.to_string().contains("deferred"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn terminal_approval_request_requires_decision_evidence() {
    let store = test_store().await;

    let error = store
        .with_connection(|conn| {
            conn.execute(
                r#"
            INSERT INTO approval_requests (
              approval_id, action_summary, tool_invocation_id, requester_actor_id,
              owner_scope_id, active_scope_id, destination_summary, data_source_summary,
              source_owner_identity, source_owner_trust, destination_owner_identity,
              destination_owner_trust, export_summary, payload_preview_json, status,
              decision_actor_id, decided_at
            )
            VALUES (
              'approval:approved-without-decision',
              'Approved approval decision evidence test',
              'tool_invocation:approved-without-decision', 'agent:primary',
              'human:local', 'human:local', 'Destination', 'Data source',
              'kevin@example.com', 'trusted', 'person@example.com', 'untrusted',
              'Exported data', '{}', 'approved', NULL, NULL
            )
            "#,
                [],
            )?;
            Ok(())
        })
        .await
        .expect_err("terminal approval without decision evidence should be rejected");

    assert!(
        error.to_string().contains("decision_actor_id")
            || error.to_string().contains("decided_at")
            || error.to_string().contains("approved"),
        "unexpected error: {error}"
    );
}
