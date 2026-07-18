#[cfg(feature = "transport")]
use std::collections::BTreeMap;

use serde_json::json;

use crate::{
    McpCalibrationStatus, McpControlPlaneServer, McpControlPlaneTool, McpServerAuthStatus,
    McpServerHealthStatus, McpServerRecord, McpToolRecord, McpTransportKind,
    McpTrustClassification, ToolCalibrationRecord,
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
        health_status: McpServerHealthStatus::Unknown,
        auth_status: McpServerAuthStatus::None,
        tool_count: 0,
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
        annotations: json!({"readOnlyHint": true}),
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
    let calibration = ToolCalibrationRecord {
        calibration_id: "calibration:read".to_string(),
        mcp_tool_id: tool.mcp_tool_id.clone(),
        read_classification: McpTrustClassification::Trusted,
        write_classification: McpTrustClassification::None,
        export_classification: McpTrustClassification::None,
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(tool.metadata_fingerprint.clone()),
    };
    McpControlPlaneServer {
        server: McpServerRecord {
            display_name: "Docs".to_string(),
            health_status: McpServerHealthStatus::Healthy,
            tool_count: 1,
            ..server_record(
                "mcp:docs",
                McpTransportKind::Stdio,
                json!({"command": "docs-server", "args": [], "cwd": null, "env": {}}),
            )
        },
        tools: vec![McpControlPlaneTool {
            tool,
            calibration: Some(calibration),
        }],
    }
}
