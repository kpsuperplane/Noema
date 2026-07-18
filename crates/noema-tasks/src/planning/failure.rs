use crate::{RunKind, RunStatus, SafeErrorCode, TaskRecoveryReason};

use super::WorkFailedRunFacts;

/// Normalize a reported run failure into the facts consumed by the pure
/// reconciliation planner.
///
/// Closed semantic error codes override a worker's retry hint. Unknown
/// non-retryable failures fail closed as invariant recovery.
#[must_use]
pub fn reported_failure_facts(
    run_kind: RunKind,
    status: RunStatus,
    error_code: &SafeErrorCode,
    retryable: bool,
    retries_exhausted: bool,
) -> WorkFailedRunFacts {
    let explicit_reason = match error_code.as_str() {
        "unsafe_effect_uncertain" => Some(TaskRecoveryReason::UnsafeEffectUncertain),
        "configuration_unavailable" => Some(TaskRecoveryReason::ConfigurationUnavailable),
        "invariant_fault" => Some(TaskRecoveryReason::InvariantFault),
        _ => None,
    };
    if let Some(recovery_reason) = explicit_reason {
        return WorkFailedRunFacts {
            run_kind,
            status,
            retryable: false,
            retries_exhausted: false,
            recovery_reason: Some(recovery_reason),
        };
    }
    if retryable {
        WorkFailedRunFacts {
            run_kind,
            status,
            retryable: true,
            retries_exhausted,
            recovery_reason: retries_exhausted
                .then_some(TaskRecoveryReason::InfrastructureRetriesExhausted),
        }
    } else {
        WorkFailedRunFacts {
            run_kind,
            status,
            retryable: false,
            retries_exhausted: false,
            recovery_reason: Some(TaskRecoveryReason::InvariantFault),
        }
    }
}
