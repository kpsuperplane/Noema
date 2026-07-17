use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Error returned when persisted MCP vocabulary contains an unknown value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid {kind}: {value}")]
pub struct McpModelValueError {
    kind: &'static str,
    value: String,
}

impl McpModelValueError {
    fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

macro_rules! persisted_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)?
        }
        kind = $kind:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[serde(rename = $wire)]
                $variant,
            )+
        }

        impl $name {
            /// Return the stable persisted snake_case representation.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = McpModelValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(McpModelValueError::new($kind, value)),
                }
            }
        }
    };
}

persisted_enum! {
    /// Effective trust classification for an MCP tool policy axis.
    pub enum McpTrustClassification {
        /// The tool does not exercise this policy axis.
        None => "none",
        /// The tool operates only on trusted-owner data for this axis.
        Trusted => "trusted",
        /// The tool operates on untrusted-owner data for this axis.
        Untrusted => "untrusted",
        /// Trust depends on resolved ownership for this axis.
        Mixed => "mixed",
    }
    kind = "mcp_trust_classification"
}

persisted_enum! {
    /// Transport used to connect to an MCP server.
    pub enum McpTransportKind {
        /// Local stdio MCP transport.
        Stdio => "stdio",
        /// Remote Streamable HTTP MCP transport.
        StreamableHttp => "streamable_http",
    }
    kind = "mcp_transport_kind"
}

persisted_enum! {
    /// Review status for a calibrated MCP tool.
    pub enum McpCalibrationStatus {
        /// The tool metadata exists but needs human or admin review.
        NeedsReview => "needs_review",
        /// The tool cannot be enabled because owner resolution is incomplete.
        BlockedUnresolvedOwnership => "blocked_unresolved_ownership",
        /// The tool is reviewed and ready for gateway use.
        Ready => "ready",
        /// The tool is intentionally disabled.
        Disabled => "disabled",
    }
    kind = "mcp_calibration_status"
}

persisted_enum! {
    /// Last known MCP server health state.
    pub enum McpServerHealthStatus {
        /// Health has not been checked.
        Unknown => "unknown",
        /// Server is reachable and healthy.
        Healthy => "healthy",
        /// Server is unavailable.
        Unavailable => "unavailable",
    }
    kind = "mcp_server_health_status"
}

persisted_enum! {
    /// Last known MCP server authentication state.
    pub enum McpServerAuthStatus {
        /// Server does not require authentication.
        None => "none",
        /// Server requires authentication.
        NeedsAuth => "needs_auth",
        /// Server is authenticated.
        Authenticated => "authenticated",
        /// Authentication is unavailable or failed externally.
        Unavailable => "unavailable",
    }
    kind = "mcp_server_auth_status"
}

/// Input for creating an MCP server metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServer {
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
    /// Durable collision-resistant MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
    /// Whether this server has at least one callable read-only tool.
    pub enabled: bool,
    /// Last known server health.
    pub health_status: McpServerHealthStatus,
    /// Last known server authentication state.
    pub auth_status: McpServerAuthStatus,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
    /// Random generation for this exact connection identity.
    pub authority_generation: String,
}

/// One validated tool captured from MCP metadata discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct McpDiscoveredTool {
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
    /// Versioned SHA-256 fingerprint of the canonical metadata snapshot.
    pub metadata_fingerprint: String,
}

/// Persisted MCP tool discovery read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolRecord {
    /// Durable collision-resistant MCP tool id.
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
    /// Stable calibration id.
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

/// One tool and its current calibration in a control-plane view.
#[derive(Debug, Clone, PartialEq)]
pub struct McpControlPlaneTool {
    /// Discovered tool metadata.
    pub tool: McpToolRecord,
    /// Current calibration, when one exists.
    pub calibration: Option<ToolCalibrationRecord>,
}

/// Joined MCP server control-plane view.
#[derive(Debug, Clone, PartialEq)]
pub struct McpControlPlaneServer {
    /// Server metadata.
    pub server: McpServerRecord,
    /// Discovered tools with current calibrations.
    pub tools: Vec<McpControlPlaneTool>,
}

impl McpCalibrationStatus {
    /// Whether this state must reference the exact reviewed metadata snapshot.
    #[must_use]
    pub const fn requires_reviewed_metadata(self) -> bool {
        matches!(self, Self::BlockedUnresolvedOwnership | Self::Ready)
    }
}

impl NewToolCalibration {
    /// Whether at least one policy axis grants access.
    #[must_use]
    pub fn has_enabled_classification(&self) -> bool {
        [
            self.read_classification,
            self.write_classification,
            self.export_classification,
        ]
        .iter()
        .any(|classification| *classification != McpTrustClassification::None)
    }

    /// Whether any policy axis still requires ownership resolution.
    #[must_use]
    pub fn has_mixed_classification(&self) -> bool {
        [
            self.read_classification,
            self.write_classification,
            self.export_classification,
        ]
        .contains(&McpTrustClassification::Mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_enum_wire_values_are_stable_and_fail_closed() {
        let transport = [
            (McpTransportKind::Stdio, "stdio"),
            (McpTransportKind::StreamableHttp, "streamable_http"),
        ];
        for (value, wire) in transport {
            assert_eq!(value.as_str(), wire);
            assert_eq!(wire.parse::<McpTransportKind>().expect("parse"), value);
            assert_eq!(
                serde_json::to_string(&value).expect("serialize"),
                format!("\"{wire}\"")
            );
        }
        assert!("sse".parse::<McpTransportKind>().is_err());

        for wire in ["none", "trusted", "untrusted", "mixed"] {
            let value = wire.parse::<McpTrustClassification>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        for wire in [
            "needs_review",
            "blocked_unresolved_ownership",
            "ready",
            "disabled",
        ] {
            let value = wire.parse::<McpCalibrationStatus>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        for wire in ["unknown", "healthy", "unavailable"] {
            let value = wire.parse::<McpServerHealthStatus>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        for wire in ["none", "needs_auth", "authenticated", "unavailable"] {
            let value = wire.parse::<McpServerAuthStatus>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        assert!("future".parse::<McpCalibrationStatus>().is_err());
        assert!("future".parse::<McpServerHealthStatus>().is_err());
        assert!("future".parse::<McpServerAuthStatus>().is_err());
        assert!("future".parse::<McpTrustClassification>().is_err());
    }
}
