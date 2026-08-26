use serde::{Deserialize, Serialize};

use crate::{
    RunKind, RunStatus, TaskGateKind, TaskRecoveryReason, WorkDomainError, WorkflowStageBehavior,
    error::invalid_input, gate::recovery_fields_are_valid,
};

#[path = "planning/failure.rs"]
mod failure;

pub use failure::reported_failure_facts;

/// Durable failed-run facts consumed by reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkFailedRunFacts {
    pub run_kind: RunKind,
    pub status: RunStatus,
    pub retryable: bool,
    pub retries_exhausted: bool,
    pub recovery_reason: Option<TaskRecoveryReason>,
}

/// Durable facts consumed by the reconciler decision function.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkReconciliationSnapshot {
    pub stage_behavior: WorkflowStageBehavior,
    pub has_open_gate: bool,
    pub has_runnable_run: bool,
    pub has_run_waiting_for_approval: bool,
    pub resolved_gate_resume_run_kind: Option<RunKind>,
    pub failed_run: Option<WorkFailedRunFacts>,
}

/// One deterministic reconciler action or intentional idle result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkReconciliationAction {
    /// Durable facts intentionally require no mutation.
    Idle,
    /// Queue one role-specific child run.
    QueueRun {
        /// Role for the one queued child run.
        run_kind: RunKind,
    },
    /// Atomically move a resolved Waiting task to Queue and enqueue its role.
    MoveToQueueAndQueueRun {
        /// Role resumed after the resolved gate.
        run_kind: RunKind,
    },
    /// Open a human Recovery gate.
    OpenRecoveryGate {
        /// Closed reason for the gate.
        reason: TaskRecoveryReason,
        /// Explicit safe continuation role, if any.
        retry_run_kind: Option<RunKind>,
    },
    /// Fence runnable work found in terminal history.
    FenceStaleRuns,
}

impl WorkReconciliationAction {
    /// Validate recovery facts carried by this action.
    /// # Errors
    /// Returns [`WorkDomainError`] when the action has an invalid Recovery
    /// pairing.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if let Self::OpenRecoveryGate {
            reason,
            retry_run_kind,
        } = self
            && !recovery_fields_are_valid(TaskGateKind::Recovery, Some(*reason), *retry_run_kind)
        {
            return Err(invalid_input(
                "reconciliation.recovery",
                "recovery reason and continuation role are inconsistent",
            ));
        }
        Ok(())
    }
}

/// Purely derive the next safe action from durable facts.
/// # Errors
/// Returns [`WorkDomainError`] when the snapshot contains contradictory run,
/// gate, review, or failure facts that cannot be reconciled safely.
pub fn plan_reconciliation_action(
    snapshot: WorkReconciliationSnapshot,
) -> Result<WorkReconciliationAction, WorkDomainError> {
    use WorkflowStageBehavior::*;

    if let Some(failed_run) = &snapshot.failed_run {
        validate_failed_run_facts(failed_run)?;
    }
    if snapshot.stage_behavior.is_terminal() {
        return Ok(if snapshot.has_runnable_run {
            WorkReconciliationAction::FenceStaleRuns
        } else {
            WorkReconciliationAction::Idle
        });
    }
    if snapshot.stage_behavior == HumanGate {
        return if snapshot.has_runnable_run {
            Ok(WorkReconciliationAction::FenceStaleRuns)
        } else if snapshot.has_open_gate {
            Ok(WorkReconciliationAction::Idle)
        } else if let Some(run_kind) = snapshot.resolved_gate_resume_run_kind {
            Ok(WorkReconciliationAction::MoveToQueueAndQueueRun { run_kind })
        } else {
            invariant_recovery()
        };
    }
    if snapshot.has_open_gate {
        return Err(invalid_input(
            "reconciliation.gate",
            "an open gate must leave the task in Waiting",
        ));
    }
    if snapshot.has_runnable_run {
        return Ok(WorkReconciliationAction::Idle);
    }
    if let Some(failed_run) = snapshot.failed_run {
        return plan_failed_run_action(failed_run);
    }
    if snapshot.stage_behavior == Dispatch {
        let run_kind = snapshot
            .resolved_gate_resume_run_kind
            .unwrap_or(RunKind::Planner);
        return Ok(WorkReconciliationAction::QueueRun { run_kind });
    }
    if snapshot.stage_behavior == Active {
        if snapshot.has_run_waiting_for_approval {
            return Ok(WorkReconciliationAction::Idle);
        }
        return Ok(WorkReconciliationAction::OpenRecoveryGate {
            reason: TaskRecoveryReason::InvariantFault,
            retry_run_kind: None,
        });
    }
    Ok(WorkReconciliationAction::Idle)
}

fn validate_failed_run_facts(facts: &WorkFailedRunFacts) -> Result<(), WorkDomainError> {
    if !matches!(facts.status, RunStatus::Failed | RunStatus::Interrupted) {
        return Err(invalid_input(
            "reconciliation.failed_run.status",
            "failed facts must be Failed or Interrupted",
        ));
    }
    match (
        facts.retryable,
        facts.retries_exhausted,
        facts.recovery_reason,
    ) {
        (true, false, None) => Ok(()),
        (true, false, Some(_)) => Err(invalid_input(
            "reconciliation.failed_run.recovery_reason",
            "an under-limit retry cannot carry a recovery reason",
        )),
        (true, true, None) => Ok(()),
        (true, true, Some(TaskRecoveryReason::InfrastructureRetriesExhausted))
        | (true, true, Some(TaskRecoveryReason::ReviewRoundsExhausted)) => Ok(()),
        (true, true, Some(_)) => Err(invalid_input(
            "reconciliation.failed_run.recovery_reason",
            "retryable exhausted facts have an incompatible recovery reason",
        )),
        (false, true, _) => Err(invalid_input(
            "reconciliation.failed_run.retries_exhausted",
            "nonretryable facts cannot be marked retries exhausted",
        )),
        (false, false, Some(TaskRecoveryReason::InvariantFault))
        | (false, false, Some(TaskRecoveryReason::UnsafeEffectUncertain))
        | (false, false, Some(TaskRecoveryReason::ConfigurationUnavailable)) => Ok(()),
        (false, false, Some(_)) => Err(invalid_input(
            "reconciliation.failed_run.recovery_reason",
            "recovery reason is inconsistent with nonretryable facts",
        )),
        (false, false, None) => Err(invalid_input(
            "reconciliation.failed_run.recovery_reason",
            "nonretryable facts require a recovery reason",
        )),
    }
}

fn plan_failed_run_action(
    facts: WorkFailedRunFacts,
) -> Result<WorkReconciliationAction, WorkDomainError> {
    if facts.retryable && !facts.retries_exhausted {
        return Ok(WorkReconciliationAction::QueueRun {
            run_kind: facts.run_kind,
        });
    }
    let reason = facts
        .recovery_reason
        .unwrap_or(TaskRecoveryReason::InfrastructureRetriesExhausted);
    let retry_run_kind = match reason {
        TaskRecoveryReason::InvariantFault => None,
        TaskRecoveryReason::ReviewRoundsExhausted => Some(RunKind::Executor),
        TaskRecoveryReason::InfrastructureRetriesExhausted
        | TaskRecoveryReason::UnsafeEffectUncertain
        | TaskRecoveryReason::ConfigurationUnavailable => Some(facts.run_kind),
    };
    Ok(WorkReconciliationAction::OpenRecoveryGate {
        reason,
        retry_run_kind,
    })
}

fn invariant_recovery() -> Result<WorkReconciliationAction, WorkDomainError> {
    Ok(WorkReconciliationAction::OpenRecoveryGate {
        reason: TaskRecoveryReason::InvariantFault,
        retry_run_kind: None,
    })
}
