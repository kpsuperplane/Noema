#[cfg(feature = "transport")]
use std::collections::BTreeMap;

use serde_json::json;

use crate::{
    McpControlPlaneServer, McpControlPlaneTool, McpDataSharingPolicy, McpServerAuthStatus,
    McpServerHealthStatus, McpServerRecord, McpToolHint, McpToolHintSource, McpToolPolicyRecord,
    McpToolPolicyStatus, McpToolRecord, McpTransportKind, McpUnsafeActionPolicy,
};
#[cfg(feature = "transport")]
use crate::{McpDiscoveredTool, McpSecretMaterial};

#[cfg(feature = "transport")]
pub(crate) fn secret_material(value: &str) -> McpSecretMaterial {
    McpSecretMaterial {
        env: BTreeMap::from([("TOKEN".to_string(), value.to_string())]),
        ..McpSecretMaterial::default()
    }
}

pub(crate) fn server_record(
    id: &str,
    transport_kind: McpTransportKind,
    safe_config: serde_json::Value,
) -> McpServerRecord {
    McpServerRecord {
        mcp_server_id: id.to_string(),
        display_name: "Test MCP".to_string(),
        transport_kind,
        safe_config,
        enabled: true,
        data_sharing_policy: Some(McpDataSharingPolicy::AllowAutomatically),
        unsafe_action_policy: Some(McpUnsafeActionPolicy::ReviewerMayApprove),
        policy_revision: 1,
        health_status: McpServerHealthStatus::Unknown,
        auth_status: McpServerAuthStatus::None,
        tool_count: 0,
        available_tool_count: 0,
        pending_tool_count: 0,
        defaulted_tool_count: 0,
        disabled_tool_count: 0,
        authority_generation: "generation:v1".to_string(),
    }
}

#[cfg(feature = "transport")]
pub(crate) fn discovered_tool() -> McpDiscoveredTool {
    McpDiscoveredTool {
        name: "read".to_string(),
        description: Some("Read documents".to_string()),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({
            "readOnlyHint": true,
            "idempotentHint": true,
            "destructiveHint": false,
            "openWorldHint": false
        }),
        metadata_fingerprint: "ignored".to_string(),
    }
}

pub(crate) fn ready_server() -> McpControlPlaneServer {
    let tool = McpToolRecord {
        mcp_tool_id: "mcp_tool:docs:read".to_string(),
        mcp_server_id: "mcp:docs".to_string(),
        name: "read".to_string(),
        description: Some("Read documents".to_string()),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({"readOnlyHint": true}),
        metadata_fingerprint: "fingerprint:v1".to_string(),
        discovered_at: "now".to_string(),
    };
    let annotation = |value| McpToolHint {
        value: Some(value),
        source: Some(McpToolHintSource::Annotation),
    };
    let policy = McpToolPolicyRecord {
        mcp_tool_id: tool.mcp_tool_id.clone(),
        read_only: annotation(true),
        idempotent: annotation(true),
        destructive: annotation(false),
        open_world: annotation(false),
        status: McpToolPolicyStatus::Ready,
        policy_revision: 1,
        metadata_fingerprint: tool.metadata_fingerprint.clone(),
    };
    McpControlPlaneServer {
        server: McpServerRecord {
            display_name: "Docs".to_string(),
            health_status: McpServerHealthStatus::Healthy,
            tool_count: 1,
            available_tool_count: 1,
            ..server_record(
                "mcp:docs",
                McpTransportKind::Stdio,
                json!({"command": "docs-server", "args": [], "cwd": null, "env": {}}),
            )
        },
        tools: vec![McpControlPlaneTool {
            tool,
            policy: Some(policy),
        }],
    }
}
