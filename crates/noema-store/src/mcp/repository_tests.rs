use noema_capabilities_mcp::{
    McpDataSharingPolicy, McpDiscoveredTool, McpDiscoveryCommit, McpInitialDiscoveryCommit,
    McpRepository, McpRepositoryErrorKind, McpServerAuthStatus, McpServerHealthStatus,
    McpToolHintSource, McpToolPolicyOverride, McpToolPolicyStatus, McpTransportKind,
    McpUnsafeActionPolicy, NewMcpServer,
};
use serde_json::json;

#[tokio::test]
async fn discovery_makes_complete_tools_ready_and_only_partial_tools_pending() {
    let store = crate::tests::test_store().await;
    let committed = store
        .commit_initial_discovery(McpInitialDiscoveryCommit {
            server: new_server(),
            tools: vec![
                complete_tool("complete", "fingerprint:complete"),
                partial_tool("partial", "fingerprint:partial"),
            ],
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("initial discovery");

    let complete = policy_named(&committed, "complete");
    assert_eq!(complete.status, McpToolPolicyStatus::Ready);
    assert_eq!(
        complete.read_only.source,
        Some(McpToolHintSource::Annotation)
    );
    let partial = policy_named(&committed, "partial");
    assert_eq!(partial.status, McpToolPolicyStatus::Pending);
    assert_eq!(partial.read_only.value, Some(true));
    assert_eq!(partial.idempotent.value, None);
    assert_eq!(committed.server.available_tool_count, 1);
    assert_eq!(committed.server.pending_tool_count, 1);

    let mut completion = partial.clone();
    completion.idempotent.value = Some(true);
    completion.idempotent.source = Some(McpToolHintSource::Model);
    completion.destructive.value = Some(false);
    completion.destructive.source = Some(McpToolHintSource::Model);
    completion.open_world.value = Some(false);
    completion.open_world.source = Some(McpToolHintSource::Model);
    completion.status = McpToolPolicyStatus::Ready;
    let merged = store
        .complete_tool_policy(completion)
        .await
        .expect("completion")
        .expect("current pending policy");
    assert_eq!(merged.read_only.source, Some(McpToolHintSource::Annotation));
    assert_eq!(merged.idempotent.source, Some(McpToolHintSource::Model));
}

#[tokio::test]
async fn disabling_and_reenabling_preserves_effective_hints_without_reclassification() {
    let store = crate::tests::test_store().await;
    let initial = seed_partial(&store).await;
    let mut completion = policy_named(&initial, "partial").clone();
    completion.idempotent.value = Some(true);
    completion.idempotent.source = Some(McpToolHintSource::Model);
    completion.destructive.value = Some(true);
    completion.destructive.source = Some(McpToolHintSource::SafeDefault);
    completion.open_world.value = Some(true);
    completion.open_world.source = Some(McpToolHintSource::SafeDefault);
    completion.status = McpToolPolicyStatus::Defaulted;
    let classified = store
        .complete_tool_policy(completion)
        .await
        .expect("completion")
        .expect("current pending policy");
    let mut expected = classified.clone();
    expected.policy_revision += 2;

    let disabled = store
        .set_tool_enabled(classified.mcp_tool_id.clone(), false)
        .await
        .expect("disable tool");
    assert_eq!(disabled.status, McpToolPolicyStatus::Disabled);

    let reenabled = store
        .set_tool_enabled(classified.mcp_tool_id, true)
        .await
        .expect("re-enable tool");
    assert_eq!(reenabled, expected);
}

#[tokio::test]
async fn stale_completion_cannot_overwrite_changed_metadata() {
    let store = crate::tests::test_store().await;
    let initial = seed_partial(&store).await;
    let stale = policy_named(&initial, "partial").clone();

    let changed = store
        .commit_discovery(McpDiscoveryCommit {
            mcp_server_id: initial.server.mcp_server_id.clone(),
            expected_authority_generation: initial.server.authority_generation.clone(),
            tools: vec![partial_tool("partial", "fingerprint:v2")],
            health_status: McpServerHealthStatus::Healthy,
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("metadata change");
    assert_eq!(
        policy_named(&changed, "partial").status,
        McpToolPolicyStatus::Pending
    );

    let mut stale_completion = stale;
    for hint in [
        &mut stale_completion.idempotent,
        &mut stale_completion.destructive,
        &mut stale_completion.open_world,
    ] {
        hint.value = Some(false);
        hint.source = Some(McpToolHintSource::Model);
    }
    stale_completion.status = McpToolPolicyStatus::Ready;
    assert!(
        store
            .complete_tool_policy(stale_completion)
            .await
            .expect("stale completion")
            .is_none()
    );
}

#[tokio::test]
async fn metadata_change_discards_human_override_and_reapplies_annotations() {
    let store = crate::tests::test_store().await;
    let initial = seed_partial(&store).await;
    let tool = &initial.tools[0].tool;
    store
        .save_tool_override(McpToolPolicyOverride {
            mcp_tool_id: tool.mcp_tool_id.clone(),
            read_only: false,
            idempotent: false,
            destructive: true,
            open_world: true,
            metadata_fingerprint: tool.metadata_fingerprint.clone(),
        })
        .await
        .expect("human override");

    let changed = store
        .commit_discovery(McpDiscoveryCommit {
            mcp_server_id: initial.server.mcp_server_id,
            expected_authority_generation: initial.server.authority_generation,
            tools: vec![partial_tool("partial", "fingerprint:v2")],
            health_status: McpServerHealthStatus::Healthy,
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("metadata change");
    let reset = policy_named(&changed, "partial");
    assert_eq!(reset.status, McpToolPolicyStatus::Pending);
    assert_eq!(reset.read_only.value, Some(true));
    assert_eq!(reset.read_only.source, Some(McpToolHintSource::Annotation));
    assert!(reset.idempotent.source.is_none());
}

#[tokio::test]
async fn incompatible_provider_policy_is_rejected_by_storage() {
    let store = crate::tests::test_store().await;
    let initial = seed_partial(&store).await;
    let error = store
        .save_provider_policy(noema_capabilities_mcp::McpProviderPolicyUpdate {
            mcp_server_id: initial.server.mcp_server_id,
            data_sharing_policy: McpDataSharingPolicy::ReviewEveryCall,
            unsafe_action_policy: McpUnsafeActionPolicy::NeverAsk,
        })
        .await
        .expect_err("invalid provider policy");
    assert_eq!(error.kind(), McpRepositoryErrorKind::Conflict);
}

async fn seed_partial(store: &crate::NoemaStore) -> noema_capabilities_mcp::McpControlPlaneServer {
    store
        .commit_initial_discovery(McpInitialDiscoveryCommit {
            server: new_server(),
            tools: vec![partial_tool("partial", "fingerprint:v1")],
            auth_status: McpServerAuthStatus::None,
        })
        .await
        .expect("initial discovery")
}

fn policy_named<'a>(
    server: &'a noema_capabilities_mcp::McpControlPlaneServer,
    name: &str,
) -> &'a noema_capabilities_mcp::McpToolPolicyRecord {
    server
        .tools
        .iter()
        .find(|tool| tool.tool.name == name)
        .and_then(|tool| tool.policy.as_ref())
        .expect("tool policy")
}

fn new_server() -> NewMcpServer {
    NewMcpServer {
        display_name: "Docs".to_string(),
        transport_kind: McpTransportKind::Stdio,
        safe_config: json!({"command": "docs-server"}),
    }
}

fn complete_tool(name: &str, fingerprint: &str) -> McpDiscoveredTool {
    McpDiscoveredTool {
        annotations: json!({
            "readOnlyHint": true,
            "idempotentHint": true,
            "destructiveHint": false,
            "openWorldHint": false
        }),
        ..partial_tool(name, fingerprint)
    }
}

fn partial_tool(name: &str, fingerprint: &str) -> McpDiscoveredTool {
    McpDiscoveredTool {
        name: name.to_string(),
        description: Some(format!("{name} documents")),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({"readOnlyHint": true}),
        metadata_fingerprint: fingerprint.to_string(),
    }
}
