use serde_json::Value;

use crate::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};

/// Input for creating an MCP server metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
}

/// Persisted MCP server settings read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerRecord {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
    /// Whether this server is enabled.
    pub enabled: bool,
    /// Last known server health.
    pub health_status: McpServerHealthStatus,
    /// Last known server authentication state.
    pub auth_status: McpServerAuthStatus,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
}

/// Last known MCP server health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerHealthStatus {
    /// Health has not been checked.
    Unknown,
    /// Server is reachable and healthy.
    Healthy,
    /// Server is unavailable.
    Unavailable,
}

/// Last known MCP server authentication state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerAuthStatus {
    /// Server does not require authentication.
    None,
    /// Server requires authentication.
    NeedsAuth,
    /// Server is authenticated.
    Authenticated,
    /// Authentication is unavailable or failed externally.
    Unavailable,
}

/// Input captured from MCP tool discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpTool {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Value,
    /// Optional MCP output schema.
    pub output_schema: Option<Value>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Value,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
}

/// Persisted MCP tool discovery read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolRecord {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Value,
    /// Optional MCP output schema.
    pub output_schema: Option<Value>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Value,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
    /// Discovery timestamp string.
    pub discovered_at: String,
}

/// Input for saving reviewed MCP tool calibration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewToolCalibration {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: McpTrustClassification,
    /// Effective write classification.
    pub write_classification: McpTrustClassification,
    /// Effective export classification.
    pub export_classification: McpTrustClassification,
    /// Review/gateway readiness status.
    pub status: McpCalibrationStatus,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Persisted MCP tool calibration read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCalibrationRecord {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: McpTrustClassification,
    /// Effective write classification.
    pub write_classification: McpTrustClassification,
    /// Effective export classification.
    pub export_classification: McpTrustClassification,
    /// Review/gateway readiness status.
    pub status: McpCalibrationStatus,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

impl McpServerHealthStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Unavailable => "unavailable",
        }
    }
}

impl McpServerAuthStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NeedsAuth => "needs_auth",
            Self::Authenticated => "authenticated",
            Self::Unavailable => "unavailable",
        }
    }
}

impl McpCalibrationStatus {
    pub(super) const fn requires_reviewed_metadata(self) -> bool {
        matches!(self, Self::BlockedUnresolvedOwnership | Self::Ready)
    }
}

impl NewToolCalibration {
    pub(super) fn has_enabled_classification(&self) -> bool {
        [
            self.read_classification,
            self.write_classification,
            self.export_classification,
        ]
        .iter()
        .any(|classification| *classification != McpTrustClassification::None)
    }

    pub(super) fn has_mixed_classification(&self) -> bool {
        [
            self.read_classification,
            self.write_classification,
            self.export_classification,
        ]
        .contains(&McpTrustClassification::Mixed)
    }
}
