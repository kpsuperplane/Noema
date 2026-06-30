use super::test_store;
use crate::{TrustedIdentitySelectorKind, normalize_trusted_identity_value};

#[test]
fn trusted_identity_selectors_normalize_email_phone_and_domain() {
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, " Kevin@Example.COM "),
        Some("kevin@example.com".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(
            TrustedIdentitySelectorKind::Email,
            " Kevin+Noema_1@Example.COM "
        ),
        Some("kevin+noema_1@example.com".to_string())
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
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "+1-415-555-0100"),
        Some("+14155550100".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "+1-415-555-0100abc"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "+1+4155550100"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "415.555.0100"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, "1-415-555-0100"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, " ext. "),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, " "),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, "kevin@@example.com"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, "kevin@example"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, ".kevin@example.com"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(
            TrustedIdentitySelectorKind::Email,
            "kevin..x@example.com"
        ),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, "bad()@example.com"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(
            TrustedIdentitySelectorKind::Domain,
            "https://example.com"
        ),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Domain, "bad-.example.com"),
        None
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Domain, "example com"),
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
              owner_extractors = [
                {
                  source: 'arguments',
                  selector_kind: 'email',
                  path: '/owner/email'
                }
              ],
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

            CREATE type::record('tool_invocations', 'local_test_invocation') SET
              tool_invocation_id = 'tool_invocation:local-test',
              mcp_server_id = 'mcp_server:local-test',
              mcp_tool_id = 'mcp_tool:local-test:read',
              conversation_id = 'conversation:local-test',
              turn_id = 'turn:local-test',
              requesting_actor_id = 'agent:primary',
              status = 'proposed',
              policy_decision = {},
              proposal_payload = {},
              result_summary = {},
              updated_at = time::now();

            CREATE type::record('quarantined_tool_results', 'local_test_result') SET
              quarantine_id = 'quarantine:local-test',
              tool_invocation_id = 'tool_invocation:local-test',
              validation_status = 'pending',
              owner_trust = 'unresolved',
              release_status = 'quarantined',
              owner_evidence = {},
              scan_summary = {},
              raw_result_ref = NONE,
              released_payload = {},
              updated_at = time::now();

            CREATE type::record('approval_requests', 'local_test_approval') SET
              approval_id = 'approval:local-test',
              action_summary = 'Approve local test MCP call',
              mcp_server_id = 'mcp_server:local-test',
              mcp_tool_id = 'mcp_tool:local-test:read',
              requester_actor_id = 'agent:primary',
              owner_scope_id = 'human:local',
              payload_preview = {},
              status = 'pending',
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

#[tokio::test]
async fn trusted_identity_schema_rejects_empty_identity_fields() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('trusted_identity_selectors', 'empty_value') SET
              selector_id = 'trusted_identity:empty-value',
              owner_scope_id = 'human:local',
              selector_kind = 'email',
              normalized_value = '',
              effect = 'trust',
              issuer_actor_id = 'human:local',
              updated_at = time::now();
            "#,
        )
        .await
        .expect("empty trusted identity query")
        .check()
        .expect_err("empty normalized identity value should be rejected");

    assert!(
        error.to_string().contains("normalized_value"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn trusted_identity_schema_rejects_invalid_normalized_shapes() {
    let store = test_store().await;
    let cases = [
        ("email_missing_at", "email", "kevin.example.com"),
        ("email_whitespace", "email", "kevin @example.com"),
        ("domain_missing_dot", "domain", "example"),
        ("domain_with_slash", "domain", "example.com/path"),
        ("domain_bad_label", "domain", "bad-.example.com"),
        ("phone_without_plus", "phone", "14155550100"),
        ("phone_with_space", "phone", "+1415 5550100"),
    ];

    for (record_id, selector_kind, normalized_value) in cases {
        let query = format!(
            r#"
            CREATE type::record('trusted_identity_selectors', '{record_id}') SET
              selector_id = 'trusted_identity:{record_id}',
              owner_scope_id = 'human:local',
              selector_kind = '{selector_kind}',
              normalized_value = '{normalized_value}',
              effect = 'trust',
              issuer_actor_id = 'human:local',
              updated_at = time::now();
            "#
        );

        let error = store
            .db()
            .query(query.as_str())
            .await
            .expect("invalid trusted identity query")
            .check()
            .expect_err("invalid trusted identity shape should be rejected");

        assert!(
            error.to_string().contains("normalized_value")
                || error.to_string().contains(normalized_value),
            "unexpected error for {record_id}: {error}"
        );
    }
}

#[tokio::test]
async fn tool_calibration_schema_rejects_malformed_owner_extractors() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('tool_calibrations', 'malformed_extractor') SET
              calibration_id = 'tool_calibration:malformed-extractor',
              mcp_tool_id = 'mcp_tool:malformed-extractor',
              read_classification = 'mixed',
              write_classification = 'none',
              export_classification = 'none',
              owner_extractors = [
                {
                  source: 'arguments',
                  selector_kind: 'email',
                  path: ''
                }
              ],
              enabled_agent_ids = [],
              enabled_scope_ids = [],
              status = 'ready',
              updated_at = time::now();
            "#,
        )
        .await
        .expect("malformed extractor query")
        .check()
        .expect_err("malformed owner extractor should be rejected");

    assert!(
        error.to_string().contains("owner_extractors"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn tool_invocation_schema_rejects_invalid_status() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('tool_invocations', 'invalid_status') SET
              tool_invocation_id = 'tool_invocation:invalid-status',
              requesting_actor_id = 'agent:primary',
              status = 'running',
              policy_decision = {},
              proposal_payload = {},
              result_summary = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid tool invocation query")
        .check()
        .expect_err("invalid tool invocation status should be rejected");

    assert!(
        error.to_string().contains("status") || error.to_string().contains("running"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn quarantined_tool_result_schema_rejects_invalid_status() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('quarantined_tool_results', 'invalid_status') SET
              quarantine_id = 'quarantine:invalid-status',
              tool_invocation_id = 'tool_invocation:invalid-status',
              validation_status = 'valid',
              owner_trust = 'trusted',
              release_status = 'streamed',
              owner_evidence = {},
              scan_summary = {},
              released_payload = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid quarantine query")
        .check()
        .expect_err("invalid quarantine release status should be rejected");

    assert!(
        error.to_string().contains("release_status") || error.to_string().contains("streamed"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn approval_request_schema_rejects_invalid_status() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('approval_requests', 'invalid_status') SET
              approval_id = 'approval:invalid-status',
              action_summary = 'Invalid approval status test',
              requester_actor_id = 'agent:primary',
              owner_scope_id = 'human:local',
              payload_preview = {},
              status = 'deferred',
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid approval query")
        .check()
        .expect_err("invalid approval status should be rejected");

    assert!(
        error.to_string().contains("status") || error.to_string().contains("deferred"),
        "unexpected error: {error}"
    );
}
