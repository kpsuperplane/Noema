use noema_capabilities_mcp::{
    McpCalibrationStatus, McpConnectionReplacement, McpDiscoveredTool, McpDiscoveryCommit,
    McpFailureStatus, McpInitialDiscoveryCommit, McpRepository, McpRepositoryErrorKind,
    McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, McpTrustClassification,
    NewMcpServer, NewToolCalibration,
};
use serde_json::json;

#[tokio::test]
async fn initial_discovery_is_atomic_and_allocates_opaque_ids() {
    let store = crate::tests::test_store().await;
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
    assert_eq!(committed.server.display_name, "Docs");
    assert_eq!(committed.server.transport_kind, McpTransportKind::Stdio);
    assert_eq!(
        committed.server.safe_config,
        json!({"command": "docs-server"})
    );
    assert_eq!(
        committed.server.health_status,
        McpServerHealthStatus::Healthy
    );
}

#[tokio::test]
async fn discovery_reconciliation_preserves_ids_invalidates_reviews_and_removes_missing_tools() {
    let store = crate::tests::test_store().await;
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
    let store = crate::tests::test_store().await;
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

#[tokio::test]
async fn calibration_batch_rolls_back_after_a_partial_write_failure() {
    let store = crate::tests::test_store().await;
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
        .with_connection(|connection| {
            connection.execute_batch(&format!(
                r#"
                CREATE TEMP TRIGGER fail_second_calibration
                BEFORE INSERT ON tool_calibrations
                WHEN NEW.mcp_tool_id = '{}'
                BEGIN
                  SELECT RAISE(ABORT, 'injected calibration failure');
                END;
                "#,
                removed.mcp_tool_id
            ))?;
            Ok(())
        })
        .await
        .expect("install failure trigger");

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
        .expect_err("second write must roll back the batch");

    let persisted = store
        .control_plane_server(initial.server.mcp_server_id)
        .await
        .expect("server");
    assert!(
        persisted
            .expect("server exists")
            .tools
            .iter()
            .all(|tool| tool.calibration.is_none())
    );
}

#[tokio::test]
async fn calibration_validation_and_projection_fail_closed() {
    let store = crate::tests::test_store().await;
    let initial = seed_two_tools(&store).await;
    let read = initial
        .tools
        .iter()
        .find(|tool| tool.tool.name == "read")
        .expect("read tool")
        .tool
        .clone();

    for invalid in [
        NewToolCalibration {
            read_classification: McpTrustClassification::None,
            ..ready_calibration("calibration:none", &read.mcp_tool_id, "fingerprint:read")
        },
        NewToolCalibration {
            read_classification: McpTrustClassification::Mixed,
            ..ready_calibration("calibration:mixed", &read.mcp_tool_id, "fingerprint:read")
        },
        ready_calibration("calibration:stale", &read.mcp_tool_id, "fingerprint:stale"),
    ] {
        let error = store
            .save_calibrations(vec![invalid])
            .await
            .expect_err("invalid ready calibration must fail closed");
        assert_eq!(error.kind(), McpRepositoryErrorKind::Conflict);
    }

    let mut blocked = ready_calibration("calibration:read", &read.mcp_tool_id, "fingerprint:read");
    blocked.read_classification = McpTrustClassification::Mixed;
    blocked.status = McpCalibrationStatus::BlockedUnresolvedOwnership;
    let saved = store
        .save_calibrations(vec![blocked])
        .await
        .expect("persist unresolved ownership review");
    assert_eq!(
        saved[0].status,
        McpCalibrationStatus::BlockedUnresolvedOwnership
    );

    store
        .save_calibrations(vec![ready_calibration(
            "calibration:read",
            &read.mcp_tool_id,
            "fingerprint:read",
        )])
        .await
        .expect("persist ready read review");
    assert!(
        store
            .control_plane_server(initial.server.mcp_server_id.clone())
            .await
            .expect("server")
            .expect("server exists")
            .server
            .enabled
    );

    let mut write_capable =
        ready_calibration("calibration:read", &read.mcp_tool_id, "fingerprint:read");
    write_capable.write_classification = McpTrustClassification::Trusted;
    store
        .save_calibrations(vec![write_capable])
        .await
        .expect("persist write-capable review");
    let mut export_capable =
        ready_calibration("calibration:read", &read.mcp_tool_id, "fingerprint:read");
    export_capable.export_classification = McpTrustClassification::Untrusted;
    store
        .save_calibrations(vec![export_capable])
        .await
        .expect("persist export-capable review");
    let removed = initial
        .tools
        .iter()
        .find(|tool| tool.tool.name == "removed")
        .expect("removed tool");
    let moved_id = ready_calibration(
        "calibration:read",
        &removed.tool.mcp_tool_id,
        "fingerprint:removed",
    );
    let error = store
        .save_calibrations(vec![moved_id])
        .await
        .expect_err("calibration id cannot move between tools");
    assert_eq!(error.kind(), McpRepositoryErrorKind::Conflict);
    let persisted = store
        .control_plane_server(initial.server.mcp_server_id.clone())
        .await
        .expect("server")
        .expect("server exists");
    assert!(!persisted.server.enabled);

    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE tool_calibrations SET read_classification = 'mixed', write_classification = 'none', status = 'ready' WHERE mcp_tool_id = ?1",
                [&read.mcp_tool_id],
            )?;
            Ok(())
        })
        .await
        .expect("simulate unsupported persisted review");
    let projected = store
        .invocation_snapshot(initial.server.mcp_server_id, read.mcp_tool_id)
        .await
        .expect("snapshot")
        .expect("snapshot exists")
        .calibration
        .expect("calibration");
    assert_eq!(
        projected.status,
        McpCalibrationStatus::BlockedUnresolvedOwnership
    );
}

#[tokio::test]
async fn mcp_schema_rejects_removed_and_unknown_transports() {
    let store = crate::tests::test_store().await;
    for (id, transport) in [("removed-sse", "sse"), ("unknown", "websocket")] {
        let error = store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, safe_config_json, auth_status, health_status, enabled) VALUES (?1, 'Invalid', ?2, '{}', 'none', 'unknown', 0)",
                    [id, transport],
                )?;
                Ok(())
            })
            .await
            .expect_err("invalid transport must violate the schema constraint");
        assert!(error.to_string().contains("transport_kind"));
    }
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
