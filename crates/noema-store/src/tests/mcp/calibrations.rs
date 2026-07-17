use super::{support::*, *};

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

    store
        .save_tool_calibration(google_read_calibration(
            McpTrustClassification::Untrusted,
            McpTrustClassification::None,
            McpCalibrationStatus::Ready,
            "fingerprint_1",
        ))
        .await
        .expect("save untrusted read calibration");
    assert!(
        store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("get server")
            .expect("server")
            .enabled
    );
}

#[tokio::test]
async fn ready_write_or_export_calibration_does_not_enable_mcp_server() {
    let store = test_store_with_mcp_tool().await;
    for (write_classification, export_classification) in [
        (
            McpTrustClassification::Trusted,
            McpTrustClassification::None,
        ),
        (
            McpTrustClassification::None,
            McpTrustClassification::Untrusted,
        ),
    ] {
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "tool_calibration:read_doc".to_string(),
                mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification,
                export_classification,
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:local".to_string()),
                reviewed_metadata_fingerprint: Some("fingerprint_1".to_string()),
            })
            .await
            .expect("save calibration");
        assert!(
            !store
                .get_mcp_server("mcp_server:google")
                .await
                .expect("server")
                .expect("server exists")
                .enabled
        );
    }
}

#[tokio::test]
async fn stale_ready_fingerprint_is_not_projected_enabled() {
    let store = test_store_with_mcp_tool().await;
    store
        .save_tool_calibration(ready_google_read_calibration("fingerprint_1"))
        .await
        .expect("save calibration");
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE mcp_tools SET metadata_fingerprint = 'fingerprint_2' WHERE mcp_tool_id = 'mcp_tool:google:read_doc'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("simulate stale persisted calibration");

    assert!(
        !store
            .get_mcp_server("mcp_server:google")
            .await
            .expect("server")
            .expect("server exists")
            .enabled
    );
    assert!(
        !store
            .list_mcp_servers()
            .await
            .expect("servers")
            .into_iter()
            .next()
            .expect("server exists")
            .enabled
    );
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
