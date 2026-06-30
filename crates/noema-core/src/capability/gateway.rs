//! Runtime Capability Gateway for provider-proposed tool calls.

use crate::NoemaStore;
use serde_json::{Value, json};

/// Runtime gateway facade.
pub struct CapabilityGateway<'a> {
    /// Canonical Noema store used by calibrated capability implementations.
    pub store: &'a NoemaStore,
}

/// Provider-proposed tool call to mediate through the Capability Gateway.
pub struct GatewayToolProposal<'a> {
    /// Provider-visible tool name.
    pub name: &'a str,
    /// Provider-supplied tool payload.
    pub payload: &'a Value,
    /// Agent proposing the tool call.
    pub agent_id: &'a str,
    /// Active governable scope ids for the proposal.
    pub scope_ids: &'a [String],
}

/// Gateway execution result released back into the runtime transcript.
#[derive(Debug, Clone)]
pub struct GatewayToolResult {
    /// Whether the proposed tool completed successfully.
    pub success: bool,
    /// Model-visible result payload.
    pub payload: Value,
    /// Whether this result should be fed back to the provider in the same turn.
    pub requires_provider_continuation: bool,
}

impl CapabilityGateway<'_> {
    /// Execute or deny a provider-proposed tool call.
    pub async fn execute_tool_proposal(
        &self,
        proposal: GatewayToolProposal<'_>,
    ) -> GatewayToolResult {
        let _ = (
            self.store,
            proposal.payload,
            proposal.agent_id,
            proposal.scope_ids,
        );

        if is_mcp_shaped_tool_name(proposal.name) {
            return GatewayToolResult {
                success: false,
                payload: json!({"error": "mcp_tool_not_calibrated"}),
                requires_provider_continuation: false,
            };
        }

        GatewayToolResult {
            success: false,
            payload: json!({"error": "unknown_tool"}),
            requires_provider_continuation: false,
        }
    }
}

/// Treat `mcp.<server>.<tool>` names as MCP-shaped for the first gateway slice.
#[must_use]
pub fn is_mcp_shaped_tool_name(name: &str) -> bool {
    let mut segments = name.split('.');
    matches!(segments.next(), Some("mcp"))
        && segments.next().is_some_and(|segment| !segment.is_empty())
        && segments.next().is_some_and(|segment| !segment.is_empty())
}
