//! Capability Gateway policy core.

pub mod examination;
pub mod ownership;

use crate::McpTrustClassification;

pub use examination::{
    ReadExaminationInput, contains_prompt_injection_marker, examine_read_result,
};
pub use ownership::{ResolvedOwner, resolve_owner_from_json};

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
    /// Run examination before deciding whether to release or proceed.
    RequireExamination,
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

    if input.owner_trust == OwnerTrust::Unresolved {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "ownership_unresolved",
        };
    }

    if input.axis == CapabilityAxis::Export {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireApproval,
            reason: "v1_export_requires_manual_approval",
        };
    }

    match (input.axis, input.classification, input.owner_trust) {
        (_, McpTrustClassification::Trusted, OwnerTrust::Trusted) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Allow,
            reason: "trusted_owner",
        },
        (CapabilityAxis::Read, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireExamination,
            reason: "read_requires_examination",
        },
        (CapabilityAxis::Write, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireExamination,
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
    fn untrusted_read_requires_examination() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Read,
            classification: McpTrustClassification::Untrusted,
            owner_trust: OwnerTrust::Untrusted,
        });

        assert_eq!(
            decision.outcome,
            CapabilityDecisionOutcome::RequireExamination
        );
        assert_eq!(decision.reason, "read_requires_examination");
    }

    #[test]
    fn mixed_write_requires_examination() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Write,
            classification: McpTrustClassification::Mixed,
            owner_trust: OwnerTrust::Mixed,
        });

        assert_eq!(
            decision.outcome,
            CapabilityDecisionOutcome::RequireExamination
        );
        assert_eq!(decision.reason, "write_requires_examination");
    }

    #[test]
    fn unresolved_export_denies_before_approval() {
        let decision = evaluate_capability_policy(CapabilityPolicyInput {
            axis: CapabilityAxis::Export,
            classification: McpTrustClassification::Trusted,
            owner_trust: OwnerTrust::Unresolved,
        });

        assert_eq!(decision.outcome, CapabilityDecisionOutcome::Deny);
        assert_eq!(decision.reason, "ownership_unresolved");
    }

    #[test]
    fn v1_policy_matrix_is_explicit() {
        let cases = [
            (
                CapabilityAxis::Read,
                McpTrustClassification::Trusted,
                OwnerTrust::Trusted,
                CapabilityDecisionOutcome::Allow,
                "trusted_owner",
            ),
            (
                CapabilityAxis::Read,
                McpTrustClassification::Trusted,
                OwnerTrust::Untrusted,
                CapabilityDecisionOutcome::RequireExamination,
                "read_requires_examination",
            ),
            (
                CapabilityAxis::Read,
                McpTrustClassification::Untrusted,
                OwnerTrust::Untrusted,
                CapabilityDecisionOutcome::RequireExamination,
                "read_requires_examination",
            ),
            (
                CapabilityAxis::Read,
                McpTrustClassification::Mixed,
                OwnerTrust::Mixed,
                CapabilityDecisionOutcome::RequireExamination,
                "read_requires_examination",
            ),
            (
                CapabilityAxis::Read,
                McpTrustClassification::Mixed,
                OwnerTrust::Unresolved,
                CapabilityDecisionOutcome::Deny,
                "ownership_unresolved",
            ),
            (
                CapabilityAxis::Write,
                McpTrustClassification::Trusted,
                OwnerTrust::Trusted,
                CapabilityDecisionOutcome::Allow,
                "trusted_owner",
            ),
            (
                CapabilityAxis::Write,
                McpTrustClassification::Trusted,
                OwnerTrust::Untrusted,
                CapabilityDecisionOutcome::RequireExamination,
                "write_requires_examination",
            ),
            (
                CapabilityAxis::Write,
                McpTrustClassification::Untrusted,
                OwnerTrust::Untrusted,
                CapabilityDecisionOutcome::RequireExamination,
                "write_requires_examination",
            ),
            (
                CapabilityAxis::Write,
                McpTrustClassification::Mixed,
                OwnerTrust::Mixed,
                CapabilityDecisionOutcome::RequireExamination,
                "write_requires_examination",
            ),
            (
                CapabilityAxis::Write,
                McpTrustClassification::Mixed,
                OwnerTrust::Unresolved,
                CapabilityDecisionOutcome::Deny,
                "ownership_unresolved",
            ),
            (
                CapabilityAxis::Export,
                McpTrustClassification::Trusted,
                OwnerTrust::Trusted,
                CapabilityDecisionOutcome::RequireApproval,
                "v1_export_requires_manual_approval",
            ),
            (
                CapabilityAxis::Export,
                McpTrustClassification::Untrusted,
                OwnerTrust::Untrusted,
                CapabilityDecisionOutcome::RequireApproval,
                "v1_export_requires_manual_approval",
            ),
            (
                CapabilityAxis::Export,
                McpTrustClassification::Mixed,
                OwnerTrust::Mixed,
                CapabilityDecisionOutcome::RequireApproval,
                "v1_export_requires_manual_approval",
            ),
            (
                CapabilityAxis::Export,
                McpTrustClassification::Mixed,
                OwnerTrust::Unresolved,
                CapabilityDecisionOutcome::Deny,
                "ownership_unresolved",
            ),
            (
                CapabilityAxis::Export,
                McpTrustClassification::None,
                OwnerTrust::Trusted,
                CapabilityDecisionOutcome::Deny,
                "axis_not_supported",
            ),
        ];

        for (axis, classification, owner_trust, expected_outcome, expected_reason) in cases {
            let decision = evaluate_capability_policy(CapabilityPolicyInput {
                axis,
                classification,
                owner_trust,
            });

            assert_eq!(decision.outcome, expected_outcome);
            assert_eq!(decision.reason, expected_reason);
        }
    }
}
