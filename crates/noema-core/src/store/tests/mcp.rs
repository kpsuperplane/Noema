use super::test_store;
use crate::{TrustedIdentitySelectorKind, normalize_trusted_identity_value};

#[test]
fn trusted_identity_selectors_normalize_email_phone_and_domain() {
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, " Kevin@Example.COM "),
        Some("kevin@example.com".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Domain, " Example.COM "),
        Some("example.com".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, " +1 (415) 555-0100 "),
        Some("+14155550100".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "415.555.0100"),
        Some("4155550100".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, " ext. "),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, " "),
        None
    );
}

#[tokio::test]
async fn mcp_control_plane_tables_bootstrap() {
    let store = test_store().await;

    assert_eq!(store.schema_version().await.expect("schema version"), 1);
    store
        .db()
        .query(
            r#"
            CREATE type::record('mcp_servers', 'local_test') SET
              mcp_server_id = 'mcp_server:local-test',
              display_name = 'Local Test',
              transport_kind = 'stdio',
              safe_config = {},
              auth_status = 'none',
              health_status = 'unknown',
              enabled = false,
              updated_at = time::now();

            CREATE type::record('mcp_tools', 'local_test_read') SET
              mcp_tool_id = 'mcp_tool:local-test:read',
              mcp_server_id = 'mcp_server:local-test',
              name = 'read',
              description = 'Read metadata',
              input_schema = {},
              output_schema = NONE,
              annotations = {},
              metadata_fingerprint = 'fingerprint:local-test:read',
              discovered_at = '2026-06-30T00:00:00Z',
              updated_at = time::now();

            CREATE type::record('tool_calibrations', 'local_test_read') SET
              calibration_id = 'tool_calibration:local-test:read',
              mcp_tool_id = 'mcp_tool:local-test:read',
              read_classification = 'trusted',
              write_classification = 'none',
              export_classification = 'none',
              owner_extractors = [],
              enabled_agent_ids = [],
              enabled_scope_ids = [],
              status = 'needs_review',
              updated_at = time::now();

            CREATE type::record('trusted_identity_selectors', 'human_local_email') SET
              selector_id = 'trusted_identity:human-local:email',
              owner_scope_id = 'human:local',
              selector_kind = 'email',
              normalized_value = 'kevin@example.com',
              effect = 'trust',
              issuer_actor_id = 'human:local',
              updated_at = time::now();
            "#,
        )
        .await
        .expect("mcp schema bootstrap query")
        .check()
        .expect("mcp control-plane tables should accept valid rows");
}

#[tokio::test]
async fn mcp_control_plane_schema_rejects_invalid_enum_values() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('mcp_servers', 'invalid_transport') SET
              mcp_server_id = 'mcp_server:invalid-transport',
              display_name = 'Invalid Transport',
              transport_kind = 'websocket',
              safe_config = {},
              auth_status = 'none',
              health_status = 'unknown',
              enabled = false,
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid mcp server query")
        .check()
        .expect_err("invalid MCP transport should be rejected");

    assert!(
        error.to_string().contains("transport_kind") || error.to_string().contains("websocket"),
        "unexpected error: {error}"
    );
}
