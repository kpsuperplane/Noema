use serde_json::json;

use super::test_store;
use crate::{
    McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind,
    McpTrustClassification, NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    NewTrustedIdentitySelector, OwnerExtractor, OwnerExtractorSource, StoreError,
    TrustedIdentitySelectorEffect, TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};

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
              tool_invocation_id = 'tool_invocation:local-test',
              mcp_server_id = 'mcp_server:local-test',
              mcp_tool_id = 'mcp_tool:local-test:read',
              requester_actor_id = 'agent:primary',
              owner_scope_id = 'human:local',
              active_scope_id = 'human:local',
              destination_summary = 'Local test recipient',
              data_source_summary = 'Local test MCP result',
              source_owner_identity = 'kevin@example.com',
              source_owner_trust = 'trusted',
              destination_owner_identity = 'person@example.com',
              destination_owner_trust = 'untrusted',
              export_summary = 'Local test data leaves the MCP boundary',
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
        .save_tool_calibration(ready_mixed_calibration("fingerprint_1"))
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
    let store = test_store().await;
    store
        .create_mcp_server(NewMcpServer {
            mcp_server_id: "mcp_server:google".to_string(),
            display_name: "Google".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({}),
        })
        .await
        .expect("create server");
    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            mcp_server_id: "mcp_server:google".to_string(),
            name: "read_doc".to_string(),
            description: Some("Read a document".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint_1".to_string(),
        })
        .await
        .expect("upsert tool");

    let calibration = store
        .save_tool_calibration(NewToolCalibration {
            calibration_id: "tool_calibration:read_doc".to_string(),
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            read_classification: McpTrustClassification::Mixed,
            write_classification: McpTrustClassification::None,
            export_classification: McpTrustClassification::None,
            owner_extractors: Vec::new(),
            status: McpCalibrationStatus::BlockedUnresolvedOwnership,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("fingerprint_1".to_string()),
        })
        .await
        .expect("save calibration");

    assert_eq!(calibration.calibration_id, "tool_calibration:read_doc");
    assert_eq!(calibration.mcp_tool_id, "mcp_tool:google:read_doc");
    assert_eq!(
        calibration.read_classification,
        McpTrustClassification::Mixed
    );
    assert_eq!(calibration.owner_extractors, Vec::new());
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
async fn ready_mixed_calibration_requires_owner_extractor() {
    let store = test_store_with_mcp_tool().await;

    let error = store
        .save_tool_calibration(NewToolCalibration {
            calibration_id: "tool_calibration:read_doc".to_string(),
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            read_classification: McpTrustClassification::Mixed,
            write_classification: McpTrustClassification::None,
            export_classification: McpTrustClassification::None,
            owner_extractors: Vec::new(),
            status: McpCalibrationStatus::Ready,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("fingerprint_1".to_string()),
        })
        .await
        .expect_err("ready mixed calibration without extractor should fail");

    assert!(error.to_string().contains("requires an owner extractor"));
}

#[tokio::test]
async fn ready_calibration_requires_current_metadata_fingerprint() {
    let store = test_store_with_mcp_tool().await;

    let error = store
        .save_tool_calibration(ready_mixed_calibration("fingerprint_2"))
        .await
        .expect_err("stale reviewed fingerprint should fail");

    assert!(error.to_string().contains("does not match current"));
}

#[tokio::test]
async fn calibration_id_cannot_move_between_tools() {
    let store = test_store_with_mcp_tool().await;
    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:google:write_doc".to_string(),
            mcp_server_id: "mcp_server:google".to_string(),
            name: "write_doc".to_string(),
            description: Some("Write a document".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({"destructiveHint": true}),
            metadata_fingerprint: "fingerprint_write".to_string(),
        })
        .await
        .expect("upsert second tool");

    store
        .save_tool_calibration(ready_mixed_calibration("fingerprint_1"))
        .await
        .expect("save first calibration");

    let error = store
        .save_tool_calibration(NewToolCalibration {
            calibration_id: "tool_calibration:read_doc".to_string(),
            mcp_tool_id: "mcp_tool:google:write_doc".to_string(),
            read_classification: McpTrustClassification::None,
            write_classification: McpTrustClassification::Trusted,
            export_classification: McpTrustClassification::None,
            owner_extractors: Vec::new(),
            status: McpCalibrationStatus::Ready,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("fingerprint_write".to_string()),
        })
        .await
        .expect_err("calibration id cannot move to another tool");

    assert!(error.to_string().contains("already belongs"));
}

#[tokio::test]
async fn rediscovered_tool_metadata_invalidates_reviewed_calibration() {
    let store = test_store_with_mcp_tool().await;
    store
        .save_tool_calibration(ready_mixed_calibration("fingerprint_1"))
        .await
        .expect("save calibration");

    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            mcp_server_id: "mcp_server:google".to_string(),
            name: "read_doc".to_string(),
            description: Some("Read a document".to_string()),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}}),
            output_schema: None,
            annotations: json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint_2".to_string(),
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
}

#[tokio::test]
async fn stores_trusted_identity_selector_normalized() {
    let store = test_store().await;

    let selector = store
        .create_trusted_identity_selector(NewTrustedIdentitySelector {
            selector_id: "trusted_identity:human-local:email".to_string(),
            owner_scope_id: "human:local".to_string(),
            selector_kind: TrustedIdentitySelectorKind::Email,
            raw_value: "Kevin+Noema_1@Example.COM".to_string(),
            effect: TrustedIdentitySelectorEffect::Trust,
            issuer_actor_id: "human:local".to_string(),
        })
        .await
        .expect("create selector");

    assert_eq!(selector.selector_id, "trusted_identity:human-local:email");
    assert_eq!(selector.owner_scope_id, "human:local");
    assert_eq!(selector.selector_kind, TrustedIdentitySelectorKind::Email);
    assert_eq!(selector.normalized_value, "kevin+noema_1@example.com");
    assert_eq!(selector.effect, TrustedIdentitySelectorEffect::Trust);
    assert_eq!(selector.issuer_actor_id, "human:local");
    assert_eq!(selector.revoked_at, None);

    let selectors = store
        .list_trusted_identity_selectors("human:local")
        .await
        .expect("list selectors");
    assert_eq!(selectors, vec![selector]);
}

async fn test_store_with_mcp_tool() -> crate::NoemaStore {
    let store = test_store().await;
    store
        .create_mcp_server(NewMcpServer {
            mcp_server_id: "mcp_server:google".to_string(),
            display_name: "Google".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({}),
        })
        .await
        .expect("create server");
    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            mcp_server_id: "mcp_server:google".to_string(),
            name: "read_doc".to_string(),
            description: Some("Read a document".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint_1".to_string(),
        })
        .await
        .expect("upsert tool");
    store
}

fn ready_mixed_calibration(reviewed_metadata_fingerprint: &str) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: "tool_calibration:read_doc".to_string(),
        mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
        read_classification: McpTrustClassification::Mixed,
        write_classification: McpTrustClassification::None,
        export_classification: McpTrustClassification::None,
        owner_extractors: vec![OwnerExtractor {
            source: OwnerExtractorSource::Arguments,
            selector_kind: TrustedIdentitySelectorKind::Email,
            path: "/owner/email".to_string(),
        }],
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(reviewed_metadata_fingerprint.to_string()),
    }
}

#[tokio::test]
async fn create_trusted_identity_selector_rejects_invalid_raw_value() {
    let store = test_store().await;

    let error = store
        .create_trusted_identity_selector(NewTrustedIdentitySelector {
            selector_id: "trusted_identity:human-local:phone".to_string(),
            owner_scope_id: "human:local".to_string(),
            selector_kind: TrustedIdentitySelectorKind::Phone,
            raw_value: "14155550100".to_string(),
            effect: TrustedIdentitySelectorEffect::Trust,
            issuer_actor_id: "human:local".to_string(),
        })
        .await
        .expect_err("invalid raw phone should be rejected before insert");

    assert!(
        matches!(error, StoreError::Schema(message) if message.contains("invalid trusted identity selector value"))
    );
    assert!(
        store
            .get_trusted_identity_selector("trusted_identity:human-local:phone")
            .await
            .expect("get selector")
            .is_none()
    );
}

#[tokio::test]
async fn mcp_tool_upsert_replaces_discovered_metadata() {
    let store = test_store().await;
    store
        .create_mcp_server(NewMcpServer {
            mcp_server_id: "mcp_server:local-test".to_string(),
            display_name: "Local Test".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({}),
        })
        .await
        .expect("create server");

    store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:local-test:read".to_string(),
            mcp_server_id: "mcp_server:local-test".to_string(),
            name: "read".to_string(),
            description: Some("Old description".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({}),
            metadata_fingerprint: "fingerprint:v1".to_string(),
        })
        .await
        .expect("upsert old tool");

    let updated = store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:local-test:read".to_string(),
            mcp_server_id: "mcp_server:local-test".to_string(),
            name: "read".to_string(),
            description: Some("New description".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({"destructiveHint": false}),
            metadata_fingerprint: "fingerprint:v2".to_string(),
        })
        .await
        .expect("upsert replacement tool");

    assert_eq!(updated.description.as_deref(), Some("New description"));
    assert_eq!(updated.metadata_fingerprint, "fingerprint:v2");
    assert_eq!(updated.annotations, json!({"destructiveHint": false}));
    let tools = store
        .list_mcp_tools_for_server("mcp_server:local-test")
        .await
        .expect("list tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0], updated);
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
        ("email_uppercase", "email", "Kevin@Example.COM"),
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
              tool_invocation_id = 'tool_invocation:invalid-status',
              requester_actor_id = 'agent:primary',
              owner_scope_id = 'human:local',
              active_scope_id = 'human:local',
              destination_summary = 'Destination',
              data_source_summary = 'Data source',
              source_owner_identity = 'kevin@example.com',
              source_owner_trust = 'trusted',
              destination_owner_identity = 'person@example.com',
              destination_owner_trust = 'untrusted',
              export_summary = 'Exported data',
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

#[tokio::test]
async fn terminal_approval_request_requires_decision_evidence() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('approval_requests', 'approved_without_decision') SET
              approval_id = 'approval:approved-without-decision',
              action_summary = 'Approved approval decision evidence test',
              tool_invocation_id = 'tool_invocation:approved-without-decision',
              requester_actor_id = 'agent:primary',
              owner_scope_id = 'human:local',
              active_scope_id = 'human:local',
              destination_summary = 'Destination',
              data_source_summary = 'Data source',
              source_owner_identity = 'kevin@example.com',
              source_owner_trust = 'trusted',
              destination_owner_identity = 'person@example.com',
              destination_owner_trust = 'untrusted',
              export_summary = 'Exported data',
              payload_preview = {},
              status = 'approved',
              decision_actor_id = NONE,
              decided_at = NONE,
              updated_at = time::now();
            "#,
        )
        .await
        .expect("terminal approval query")
        .check()
        .expect_err("terminal approval without decision evidence should be rejected");

    assert!(
        error.to_string().contains("decision_actor_id")
            || error.to_string().contains("decided_at")
            || error.to_string().contains("approved"),
        "unexpected error: {error}"
    );
}
