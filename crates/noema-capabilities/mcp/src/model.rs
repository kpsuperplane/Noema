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
    /// Whether ordinary calls may share context with an MCP provider directly.
    pub enum McpDataSharingPolicy {
        /// Otherwise-safe calls may execute without approval.
        AllowAutomatically => "allow_automatically",
        /// Every call is routed through unsafe-action policy.
        ReviewEveryCall => "review_every_call",
    }
    kind = "mcp_data_sharing_policy"
}

persisted_enum! {
    /// How calls derived as unsafe are admitted.
    pub enum McpUnsafeActionPolicy {
        /// Persist the proposal and require human approval without model review.
        AlwaysAsk => "always_ask",
        /// Let the configured reviewer approve or escalate the call.
        ReviewerMayApprove => "reviewer_may_approve",
        /// Execute without an approval prompt.
        NeverAsk => "never_ask",
    }
    kind = "mcp_unsafe_action_policy"
}

persisted_enum! {
    /// Durable readiness state for one effective MCP tool policy.
    pub enum McpToolPolicyStatus {
        /// One or more behavior hints still require classification.
        Pending => "pending",
        /// All behavior hints are available from annotations, inference, or a human.
        Ready => "ready",
        /// Missing hints were filled with pessimistic defaults.
        Defaulted => "defaulted",
        /// The user intentionally disabled this tool.
        Disabled => "disabled",
    }
    kind = "mcp_tool_policy_status"
}

persisted_enum! {
    /// Authority that supplied one effective MCP tool hint.
    pub enum McpToolHintSource {
        /// Supplied by MCP tool annotations.
        Annotation => "annotation",
        /// Inferred by the classification model.
        Model => "model",
        /// Filled from MCP's pessimistic defaults after classification failed.
        SafeDefault => "safe_default",
        /// Supplied as a complete human override.
        Human => "human",
    }
    kind = "mcp_tool_hint_source"
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

/// One effective hint value and its durable source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolHint {
    /// Effective value, or `None` while classification is pending.
    pub value: Option<bool>,
    /// Authority that supplied the effective value.
    pub source: Option<McpToolHintSource>,
}

/// Persisted effective MCP tool policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolPolicyRecord {
    /// Durable MCP tool identifier.
    pub mcp_tool_id: String,
    /// Effective read-only hint.
    pub read_only: McpToolHint,
    /// Effective idempotency hint.
    pub idempotent: McpToolHint,
    /// Effective destructive hint.
    pub destructive: McpToolHint,
    /// Effective open-world hint.
    pub open_world: McpToolHint,
    /// Durable classification state.
    pub status: McpToolPolicyStatus,
    /// Monotonic revision used to fence stale completions and bindings.
    pub policy_revision: u64,
    /// Exact metadata snapshot covered by this policy.
    pub metadata_fingerprint: String,
}

/// Complete human override for one exact tool snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolPolicyOverride {
    /// Durable MCP tool identifier.
    pub mcp_tool_id: String,
    /// Human-selected read-only value.
    pub read_only: bool,
    /// Human-selected idempotency value.
    pub idempotent: bool,
    /// Human-selected destructive value.
    pub destructive: bool,
    /// Human-selected open-world value.
    pub open_world: bool,
    /// Exact metadata snapshot being overridden.
    pub metadata_fingerprint: String,
}

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

impl McpToolPolicyRecord {
    /// Whether all four behavior values are known and the tool may be advertised.
    #[must_use]
    pub const fn is_callable(&self) -> bool {
        matches!(
            self.status,
            McpToolPolicyStatus::Ready | McpToolPolicyStatus::Defaulted
        ) && self.read_only.value.is_some()
            && self.idempotent.value.is_some()
            && self.destructive.value.is_some()
            && self.open_world.value.is_some()
    }

    /// Whether the effective tool behavior is potentially risky.
    #[must_use]
    pub fn is_risky(&self) -> bool {
        self.destructive.value == Some(true)
            || (self.read_only.value == Some(false) && self.open_world.value == Some(true))
    }
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
    if data_sharing == McpDataSharingPolicy::ReviewEveryCall
        && unsafe_actions == McpUnsafeActionPolicy::NeverAsk
    {
        return Err(McpModelValueError::new(
            "mcp_provider_policy",
            "review_every_call+never_ask",
        ));
    }
    Ok(())
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
