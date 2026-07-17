use super::*;

#[tokio::test]
async fn mcp_control_plane_tables_bootstrap() {
    let store = test_store().await;

    assert_eq!(store.schema_version().await.expect("schema version"), 2);
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
