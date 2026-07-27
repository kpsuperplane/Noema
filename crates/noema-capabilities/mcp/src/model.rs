use std::{fmt, str::FromStr};

use noema_capabilities::{
    CapabilityDataSharingPolicy, CapabilityToolHint, CapabilityToolHintSource,
    CapabilityToolPolicy, CapabilityToolPolicyOverride, CapabilityToolPolicyStatus,
    CapabilityUnsafeActionPolicy, validate_capability_connection_policy,
};
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

/// MCP compatibility name for the source-neutral data-sharing policy.
pub type McpDataSharingPolicy = CapabilityDataSharingPolicy;
/// MCP compatibility name for the source-neutral unsafe-action policy.
pub type McpUnsafeActionPolicy = CapabilityUnsafeActionPolicy;
/// MCP compatibility name for the source-neutral tool readiness state.
pub type McpToolPolicyStatus = CapabilityToolPolicyStatus;
/// MCP compatibility name for source-neutral behavior-hint provenance.
pub type McpToolHintSource = CapabilityToolHintSource;

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

/// Persisted MCP definition safe for explicit connection reuse.
#[derive(Debug, Clone, PartialEq)]
pub struct McpDefinitionRecord {
    /// Durable definition identity.
    pub mcp_definition_id: String,
    /// Human-visible definition name.
    pub display_name: String,
    /// Transport shared by connections on this exact revision.
    pub transport_kind: McpTransportKind,
    /// Non-secret command or endpoint configuration.
    pub safe_config: Value,
    /// Immutable revision required when adding a connection.
    pub definition_revision: String,
}

/// Persisted MCP server settings read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerRecord {
    /// Stable definition owning the exact non-secret transport configuration.
    pub mcp_definition_id: String,
    /// Immutable definition revision used to fence explicit connection reuse.
    pub definition_revision: String,
    /// Durable collision-resistant MCP server id.
    pub mcp_server_id: String,
    /// Optional human label distinguishing this account or installation.
    pub connection_label: Option<String>,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
    /// Whether this server has a complete provider policy and one callable tool.
    pub enabled: bool,
    /// Provider-level disclosure policy, when setup is complete.
    pub data_sharing_policy: Option<McpDataSharingPolicy>,
    /// Admission policy for calls derived as unsafe, when setup is complete.
    pub unsafe_action_policy: Option<McpUnsafeActionPolicy>,
    /// Monotonic provider-policy revision used to fence stale bindings.
    pub policy_revision: u64,
    /// Last known server health.
    pub health_status: McpServerHealthStatus,
    /// Last known server authentication state.
    pub auth_status: McpServerAuthStatus,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
    /// Number of callable ready or defaulted tools.
    pub available_tool_count: usize,
    /// Number of tools waiting for classification.
    pub pending_tool_count: usize,
    /// Number of tools using pessimistic fallback values.
    pub defaulted_tool_count: usize,
    /// Number of intentionally disabled tools.
    pub disabled_tool_count: usize,
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

/// MCP compatibility name for one source-neutral behavior hint.
pub type McpToolHint = CapabilityToolHint;
/// MCP compatibility name for one source-neutral effective tool policy.
pub type McpToolPolicyRecord = CapabilityToolPolicy;

/// MCP compatibility name for one exact source-neutral human override.
pub type McpToolPolicyOverride = CapabilityToolPolicyOverride;

/// One tool and its current behavior policy in a control-plane view.
#[derive(Debug, Clone, PartialEq)]
pub struct McpControlPlaneTool {
    /// Discovered tool metadata.
    pub tool: McpToolRecord,
    /// Current effective policy, when one exists.
    pub policy: Option<McpToolPolicyRecord>,
}

/// Joined MCP server control-plane view.
#[derive(Debug, Clone, PartialEq)]
pub struct McpControlPlaneServer {
    /// Server metadata.
    pub server: McpServerRecord,
    /// Discovered tools with current behavior policies.
    pub tools: Vec<McpControlPlaneTool>,
}

/// Validate one provider policy pair.
///
/// # Errors
///
/// Returns an error when review-every-call is combined with never-ask.
pub fn validate_provider_policy(
    data_sharing: McpDataSharingPolicy,
    unsafe_actions: McpUnsafeActionPolicy,
) -> Result<(), McpModelValueError> {
    validate_capability_connection_policy(data_sharing, unsafe_actions)
        .map_err(|_| McpModelValueError::new("mcp_provider_policy", "review_every_call+never_ask"))
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

        assert_eq!(
            "review_every_call"
                .parse::<McpDataSharingPolicy>()
                .expect("parse"),
            McpDataSharingPolicy::ReviewEveryCall
        );
        for wire in ["unknown", "healthy", "unavailable"] {
            let value = wire.parse::<McpServerHealthStatus>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        for wire in ["none", "needs_auth", "authenticated", "unavailable"] {
            let value = wire.parse::<McpServerAuthStatus>().expect("parse");
            assert_eq!(value.as_str(), wire);
        }
        assert!("future".parse::<McpServerHealthStatus>().is_err());
        assert!("future".parse::<McpServerAuthStatus>().is_err());
        assert!("future".parse::<McpToolPolicyStatus>().is_err());
        assert!(
            validate_provider_policy(
                McpDataSharingPolicy::ReviewEveryCall,
                McpUnsafeActionPolicy::NeverAsk,
            )
            .is_err()
        );
    }
}
