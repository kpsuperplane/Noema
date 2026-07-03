use serde_json::Value;

use crate::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, OwnerExtractor,
    TrustedIdentitySelectorKind,
};

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
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<OwnerExtractor>,
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
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<OwnerExtractor>,
    /// Review/gateway readiness status.
    pub status: McpCalibrationStatus,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Input for creating a trusted identity selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTrustedIdentitySelector {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// Unnormalized user-provided selector value.
    pub raw_value: String,
    /// Selector effect.
    pub effect: TrustedIdentitySelectorEffect,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
}

/// Trusted identity selector effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedIdentitySelectorEffect {
    /// Trust this identity for the owner scope.
    Trust,
    /// Restrict this identity for the owner scope.
    Restrict,
}

/// Persisted trusted identity selector read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedIdentitySelectorRecord {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// Normalized selector value.
    pub normalized_value: String,
    /// Selector effect.
    pub effect: TrustedIdentitySelectorEffect,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
    /// Revocation timestamp string, when revoked.
    pub revoked_at: Option<String>,
}

/// Input for creating a durable MCP approval request.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpApprovalRequest {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id.
    pub tool_invocation_id: String,
    /// Related MCP server id, when available.
    pub mcp_server_id: Option<String>,
    /// Related MCP tool id, when available.
    pub mcp_tool_id: Option<String>,
    /// Actor requesting approval.
    pub requester_actor_id: String,
    /// Governable owner scope for the approval.
    pub owner_scope_id: String,
    /// Active governable scope for the approval.
    pub active_scope_id: String,
    /// Destination or recipient summary.
    pub destination_summary: String,
    /// Data source summary.
    pub data_source_summary: String,
    /// Source owner identity label.
    pub source_owner_identity: String,
    /// Source owner trust label.
    pub source_owner_trust: String,
    /// Destination owner identity label.
    pub destination_owner_identity: String,
    /// Destination owner trust label.
    pub destination_owner_trust: String,
    /// What leaves the MCP destination trust boundary.
    pub export_summary: String,
    /// Safe payload preview for review surfaces.
    pub payload_preview: Value,
}

/// Durable MCP approval request metadata safe to show in Settings.
#[derive(Debug, Clone, PartialEq)]
pub struct McpApprovalRequestRecord {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id.
    pub tool_invocation_id: String,
    /// Related MCP server id, when available.
    pub mcp_server_id: Option<String>,
    /// Related MCP tool id, when available.
    pub mcp_tool_id: Option<String>,
    /// Actor requesting approval.
    pub requester_actor_id: String,
    /// Governable owner scope for the approval.
    pub owner_scope_id: String,
    /// Active governable scope for the approval.
    pub active_scope_id: String,
    /// Destination or recipient summary.
    pub destination_summary: String,
    /// Data source summary.
    pub data_source_summary: String,
    /// Source owner identity label.
    pub source_owner_identity: String,
    /// Source owner trust label.
    pub source_owner_trust: String,
    /// Destination owner identity label.
    pub destination_owner_identity: String,
    /// Destination owner trust label.
    pub destination_owner_trust: String,
    /// What leaves the MCP destination trust boundary.
    pub export_summary: String,
    /// Safe payload preview for review surfaces.
    pub payload_preview: Value,
    /// Current approval status.
    pub status: String,
    /// Actor who decided the request, when decided.
    pub decision_actor_id: Option<String>,
    /// Safe decision comment, when available.
    pub decision_comment: Option<String>,
    /// Decision timestamp string, when decided.
    pub decided_at: Option<String>,
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

impl TrustedIdentitySelectorEffect {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Trust => "trust",
            Self::Restrict => "restrict",
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
