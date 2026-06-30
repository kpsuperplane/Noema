//! Capability Gateway policy core.

use crate::McpTrustClassification;

/// MCP capability axis being evaluated for a proposed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAxis {
    /// The operation may bring data from an MCP destination into Noema.
    Read,
    /// The operation may mutate state inside an MCP destination.
    Write,
    /// The operation may share data beyond the MCP destination trust boundary.
    Export,
}

/// Effective trust of the owner resolved for a proposed capability operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerTrust {
    /// The resolved owner is trusted for the active context.
    Trusted,
    /// The resolved owner is not trusted for the active context.
    Untrusted,
    /// The operation spans trusted and untrusted ownership.
    Mixed,
    /// Ownership could not be resolved.
    Unresolved,
}

/// Top-level Capability Gateway policy outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityDecisionOutcome {
    /// Permit the operation or release.
    Allow,
    /// Permit only a sanitized read result release.
    AllowSanitized,
    /// Require manual approval before proceeding.
    RequireApproval,
    /// Deny the operation or release.
    Deny,
}

/// Inputs needed for the deterministic V1 capability policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityPolicyInput {
    /// Policy axis being evaluated.
    pub axis: CapabilityAxis,
    /// Reviewed Noema-owned classification for the axis.
    pub classification: McpTrustClassification,
    /// Trust state after deterministic ownership resolution.
    pub owner_trust: OwnerTrust,
}

/// Deterministic Capability Gateway policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicyDecision {
    /// Top-level action the caller should take.
    pub outcome: CapabilityDecisionOutcome,
    /// Stable machine-readable reason for audit and tests.
    pub reason: &'static str,
}

/// Evaluate the V1 Capability Gateway policy for a single MCP capability axis.
#[must_use]
pub fn evaluate_capability_policy(input: CapabilityPolicyInput) -> CapabilityPolicyDecision {
    if input.classification == McpTrustClassification::None {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "axis_not_supported",
        };
    }

    if input.axis == CapabilityAxis::Export {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireApproval,
            reason: "v1_export_requires_manual_approval",
        };
    }

    if input.owner_trust == OwnerTrust::Unresolved {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "ownership_unresolved",
        };
    }

    match (input.axis, input.classification, input.owner_trust) {
        (_, McpTrustClassification::Trusted, OwnerTrust::Trusted) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Allow,
            reason: "trusted_owner",
        },
        (CapabilityAxis::Read, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::AllowSanitized,
            reason: "read_requires_examination",
        },
        (CapabilityAxis::Write, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireApproval,
            reason: "write_requires_examination",
        },
        _ => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "unsupported_decision",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::McpTrustClassification;

    #[test]
    fn export_always_requires_approval_in_v1() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Export,
            classification: McpTrustClassification::Trusted,
            owner_trust: OwnerTrust::Trusted,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::RequireApproval);
    }

    #[test]
    fn trusted_read_allows_without_examination() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Read,
            classification: McpTrustClassification::Trusted,
            owner_trust: OwnerTrust::Trusted,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::Allow);
    }

    #[test]
    fn unsupported_axis_classification_denies() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Read,
            classification: McpTrustClassification::None,
            owner_trust: OwnerTrust::Trusted,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::Deny);
        assert_eq!(decision.reason, "axis_not_supported");
    }

    #[test]
    fn unresolved_ownership_denies_non_export_axes() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Write,
            classification: McpTrustClassification::Mixed,
            owner_trust: OwnerTrust::Unresolved,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::Deny);
        assert_eq!(decision.reason, "ownership_unresolved");
    }

    #[test]
    fn untrusted_read_requires_sanitized_release() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Read,
            classification: McpTrustClassification::Untrusted,
            owner_trust: OwnerTrust::Untrusted,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::AllowSanitized);
        assert_eq!(decision.reason, "read_requires_examination");
    }

    #[test]
    fn mixed_write_requires_approval() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Write,
            classification: McpTrustClassification::Mixed,
            owner_trust: OwnerTrust::Mixed,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::RequireApproval);
        assert_eq!(decision.reason, "write_requires_examination");
    }
}
