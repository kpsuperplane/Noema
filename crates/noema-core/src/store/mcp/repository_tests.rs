use noema_capabilities_mcp::{
    McpCalibrationStatus, McpConnectionReplacement, McpDiscoveredTool, McpDiscoveryCommit,
    McpFailureStatus, McpInitialDiscoveryCommit, McpRepository, McpRepositoryErrorKind,
    McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, McpTrustClassification,
    NewMcpServer, NewToolCalibration,
};
use serde_json::json;

#[tokio::test]
async fn initial_discovery_is_atomic_and_allocates_opaque_ids() {
    let store = crate::store::tests::test_store().await;
    let error = store
        .commit_initial_discovery(McpInitialDiscoveryCommit {
            server: new_server(),
            tools: vec![tool("read", "fingerprint:read"), tool("read", "duplicate")],
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect_err("duplicate discovery must fail");
    assert_eq!(error.kind(), McpRepositoryErrorKind::Conflict);
    assert!(
        store
            .control_plane_catalog()
            .await
            .expect("catalog")
            .is_empty()
    );

    let committed = store
        .commit_initial_discovery(McpInitialDiscoveryCommit {
            server: new_server(),
            tools: vec![
                tool("read", "fingerprint:read"),
                tool("removed", "fingerprint:removed"),
            ],
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("initial discovery");
    assert!(committed.server.mcp_server_id.starts_with("mcp_server:"));
    assert!(
        committed
            .server
            .authority_generation
            .starts_with("mcp_generation:")
    );
    assert_eq!(committed.tools.len(), 2);
    assert!(
        committed
            .tools
            .iter()
            .all(|tool| tool.tool.mcp_tool_id.starts_with("mcp_tool:"))
    );
    assert!(
        committed
            .tools
            .iter()
            .all(|tool| tool.calibration.is_none())
    );
}

#[tokio::test]
async fn discovery_reconciliation_preserves_ids_invalidates_reviews_and_removes_missing_tools() {
    let store = crate::store::tests::test_store().await;
    let initial = seed_two_tools(&store).await;
    let read = initial
        .tools
        .iter()
        .find(|tool| tool.tool.name == "read")
        .expect("read tool")
        .tool
        .clone();
    let removed = initial
        .tools
        .iter()
        .find(|tool| tool.tool.name == "removed")
        .expect("removed tool")
        .tool
        .clone();
    store
        .save_calibrations(vec![
            ready_calibration("calibration:read", &read.mcp_tool_id, "fingerprint:read"),
            ready_calibration(
                "calibration:removed",
                &removed.mcp_tool_id,
                "fingerprint:removed",
            ),
        ])
        .await
        .expect("calibrations");

    let reconciled = store
        .commit_discovery(McpDiscoveryCommit {
            mcp_server_id: initial.server.mcp_server_id.clone(),
            expected_authority_generation: initial.server.authority_generation.clone(),
            tools: vec![tool("read", "fingerprint:read:v2")],
            health_status: McpServerHealthStatus::Healthy,
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("reconcile discovery");

    assert_eq!(reconciled.tools.len(), 1);
    assert_eq!(reconciled.tools[0].tool.mcp_tool_id, read.mcp_tool_id);
    assert_eq!(
        reconciled.tools[0]
            .calibration
            .as_ref()
            .expect("invalidated calibration")
            .status,
        McpCalibrationStatus::NeedsReview
    );
    assert!(!reconciled.server.enabled);
    assert!(
        store
            .invocation_snapshot(initial.server.mcp_server_id, removed.mcp_tool_id)
            .await
            .expect("snapshot")
            .is_none()
    );
}

#[tokio::test]
async fn generation_fences_replacement_failures_and_two_phase_delete() {
    let store = crate::store::tests::test_store().await;
    let initial = seed_two_tools(&store).await;
    let stale = store
        .replace_connection(McpConnectionReplacement {
            mcp_server_id: initial.server.mcp_server_id.clone(),
            expected_authority_generation: "stale".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({"command": "replacement"}),
        })
        .await
        .expect_err("stale generation");
    assert_eq!(stale.kind(), McpRepositoryErrorKind::Conflict);

    let replaced = store
        .replace_connection(McpConnectionReplacement {
            mcp_server_id: initial.server.mcp_server_id.clone(),
            expected_authority_generation: initial.server.authority_generation.clone(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({"command": "replacement"}),
        })
        .await
        .expect("replace connection");
    assert_ne!(
        replaced.authority_generation,
        initial.server.authority_generation
    );
    assert!(!replaced.enabled);
    assert_eq!(replaced.health_status, McpServerHealthStatus::Unknown);
    assert!(
        !store
            .record_failure_status(McpFailureStatus {
                mcp_server_id: replaced.mcp_server_id.clone(),
                expected_authority_generation: initial.server.authority_generation,
                health_status: McpServerHealthStatus::Unavailable,
                auth_status: McpServerAuthStatus::NeedsAuth,
            })
            .await
            .expect("stale failure projection")
    );

    let ticket = store
        .begin_delete(replaced.mcp_server_id.clone())
        .await
        .expect("begin delete")
        .expect("delete ticket");
    assert!(
        !store
            .finish_delete(noema_capabilities_mcp::McpDeleteTicket {
                mcp_server_id: ticket.mcp_server_id.clone(),
                deletion_generation: "forged".to_string(),
            })
            .await
            .expect("forged ticket")
    );
    assert!(store.finish_delete(ticket).await.expect("finish delete"));
    assert!(
        store
            .control_plane_server(replaced.mcp_server_id)
            .await
            .expect("server lookup")
            .is_none()
    );
}

async fn seed_two_tools(
    store: &crate::NoemaStore,
) -> noema_capabilities_mcp::McpControlPlaneServer {
    store
        .commit_initial_discovery(McpInitialDiscoveryCommit {
            server: new_server(),
            tools: vec![
                tool("read", "fingerprint:read"),
                tool("removed", "fingerprint:removed"),
            ],
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("initial discovery")
}

fn new_server() -> NewMcpServer {
    NewMcpServer {
        display_name: "Docs".to_string(),
        transport_kind: McpTransportKind::Stdio,
        safe_config: json!({"command": "docs-server"}),
    }
}

fn tool(name: &str, fingerprint: &str) -> McpDiscoveredTool {
    McpDiscoveredTool {
        name: name.to_string(),
        description: Some(format!("{name} documents")),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({}),
        metadata_fingerprint: fingerprint.to_string(),
    }
}

fn ready_calibration(
    calibration_id: &str,
    mcp_tool_id: &str,
    fingerprint: &str,
) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: calibration_id.to_string(),
        mcp_tool_id: mcp_tool_id.to_string(),
        read_classification: McpTrustClassification::Trusted,
        write_classification: McpTrustClassification::None,
        export_classification: McpTrustClassification::None,
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:reviewer".to_string()),
        reviewed_metadata_fingerprint: Some(fingerprint.to_string()),
    }
}
