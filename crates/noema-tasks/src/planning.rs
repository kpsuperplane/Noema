use serde::{Deserialize, Serialize};

use crate::{
    RunKind, RunStatus, TaskGateKind, TaskRecoveryReason, WorkDomainError, WorkflowStageBehavior,
    error::invalid_input, gate::recovery_fields_are_valid,
};

#[path = "planning/failure.rs"]
mod failure;

pub use failure::reported_failure_facts;

/// Semantic operation used by the pure task transition planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkTransition {
    /// Create a task in Inbox.
    Capture,
    /// Atomically create and authorize a delegated task, with an explicit
    /// complete/incomplete intent distinction.
    Delegate {
        /// Whether delegation already carries a complete executable intent.
        has_complete_intent: bool,
    },
    /// Edit Inbox capture fields.
    UpdateInbox,
    /// Authorize an Inbox task.
    Queue,
    /// Resolve a human gate and resume its opening role.
    Answer {
        /// Role that opened the resolved gate and must resume safely.
        resume_run_kind: RunKind,
    },
    /// Retry a Recovery gate using its explicit continuation role.
    Retry {
        /// Explicit role authorized by the Recovery gate.
        resume_run_kind: RunKind,
    },
    /// Accept a reviewed task.
    Accept,
    /// Request a new contract revision.
    RequestChanges,
    /// Cancel nonterminal work.
    Cancel,
    /// Reopen terminal history.
    Reopen,
}

/// Pure transition result for one semantic command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkTransitionPlan {
    pub from_behavior: Option<WorkflowStageBehavior>,
    pub to_behavior: WorkflowStageBehavior,
    pub generation_increment: bool,
    pub revision_increment: bool,
    pub queue_run_kind: Option<RunKind>,
    pub resume_run_kind: Option<RunKind>,
}

macro_rules! plan {
    ($from:expr, $to:expr, $generation:expr, $revision:expr, $queue:expr, $resume:expr) => {
        WorkTransitionPlan {
            from_behavior: $from,
            to_behavior: $to,
            generation_increment: $generation,
            revision_increment: $revision,
            queue_run_kind: $queue,
            resume_run_kind: $resume,
        }
    };
}

/// Decide an allowed task transition without reading persistence or text.
/// # Errors
/// Returns [`WorkDomainError::InvalidTransition`] when the requested operation
/// is not allowed from the supplied current stage behavior.
#[rustfmt::skip]
pub fn plan_work_transition(
    current: Option<WorkflowStageBehavior>,
    transition: WorkTransition,
) -> Result<WorkTransitionPlan, WorkDomainError> {
    use WorkTransition as Action;
    let result = match (current, transition) {
        (None, Action::Capture) => plan!(None, WorkflowStageBehavior::Intake, false, false, None, None),
        (
            None,
            Action::Delegate {
                has_complete_intent,
            },
        ) => plan!(None, WorkflowStageBehavior::Dispatch, false, false, Some(if has_complete_intent {
                RunKind::Executor
            } else {
                RunKind::Planner
            }), None),
        (Some(WorkflowStageBehavior::Intake), Action::UpdateInbox) => plan!(current, WorkflowStageBehavior::Intake, false, true, None, None),
        (Some(WorkflowStageBehavior::Intake), Action::Queue) => plan!(current, WorkflowStageBehavior::Dispatch, false, true, Some(RunKind::Planner), None),
        (Some(WorkflowStageBehavior::HumanGate), Action::Answer { resume_run_kind } | Action::Retry { resume_run_kind }) => plan!(current, WorkflowStageBehavior::Dispatch, false, true, Some(resume_run_kind), Some(resume_run_kind)),
        (Some(WorkflowStageBehavior::Acceptance), Action::Accept) => plan!(current, WorkflowStageBehavior::TerminalSuccess, false, true, None, None),
        (Some(WorkflowStageBehavior::Acceptance), Action::RequestChanges) => plan!(current, WorkflowStageBehavior::Dispatch, true, true, Some(RunKind::Executor), None),
        (
            Some(
                WorkflowStageBehavior::Intake
                | WorkflowStageBehavior::Dispatch
                | WorkflowStageBehavior::Active
                | WorkflowStageBehavior::HumanGate
                | WorkflowStageBehavior::Acceptance,
            ),
            Action::Cancel,
        ) => plan!(current, WorkflowStageBehavior::TerminalCancelled, true, true, None, None),
        (
            Some(WorkflowStageBehavior::TerminalSuccess | WorkflowStageBehavior::TerminalCancelled),
            Action::Reopen,
        ) => plan!(current, WorkflowStageBehavior::Intake, true, true, None, None),
        _ => return Err(WorkDomainError::InvalidTransition),
    };
    Ok(result)
}

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
    pub has_current_contract: bool,
    pub has_open_gate: bool,
    pub has_runnable_run: bool,
    pub resolved_gate_resume_run_kind: Option<RunKind>,
    pub approved_review: bool,
    pub planner_plan_ready: bool,
    pub submission_waiting_for_review: bool,
    pub review_requested_changes: bool,
    pub review_rounds_exhausted: bool,
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
    /// Atomically materialize the validated Planner plan into an immutable
    /// contract and queue its first Executor.
    MaterializePlannedContractAndQueueExecutor,
    /// Atomically move a resolved Waiting task to Queue and enqueue its role.
    MoveToQueueAndQueueRun {
        /// Role resumed after the resolved gate.
        run_kind: RunKind,
    },
    /// Move an approved review to the human acceptance stage.
    MoveToReview,
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
    /// Validate the closed recovery reason/continuation-role matrix.
    /// # Errors
    /// Returns [`WorkDomainError`] when a Recovery action pairs a reason with
    /// an unsupported continuation role.
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

    /// Validate run-role compatibility with the current contract presence.
    /// # Errors
    /// Returns [`WorkDomainError`] when the action has an invalid Recovery
    /// pairing or its queued role is incompatible with contract presence.
    pub fn validate_for_contract(&self, has_current_contract: bool) -> Result<(), WorkDomainError> {
        self.validate()?;
        let run_kind = match self {
            Self::QueueRun { run_kind } | Self::MoveToQueueAndQueueRun { run_kind } => {
                Some(*run_kind)
            }
            Self::OpenRecoveryGate {
                retry_run_kind: Some(run_kind),
                ..
            } => Some(*run_kind),
            _ => None,
        };
        if let Some(run_kind) = run_kind {
            validate_role_contract(run_kind, has_current_contract)?;
        }
        Ok(())
    }
}

/// Purely derive the next safe action from durable facts.
/// # Errors
/// Returns [`WorkDomainError`] when the snapshot contains contradictory run,
/// gate, review, failure, or contract facts that cannot be reconciled safely.
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
            compatible_action(
                WorkReconciliationAction::MoveToQueueAndQueueRun { run_kind },
                snapshot.has_current_contract,
            )
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
        if snapshot.approved_review {
            return Err(invalid_input(
                "reconciliation.review",
                "an approved review cannot coexist with a runnable run",
            ));
        }
        return Ok(WorkReconciliationAction::Idle);
    }
    if let Some(failed_run) = snapshot.failed_run {
        return plan_failed_run_action(failed_run, snapshot.has_current_contract);
    }
    if snapshot.stage_behavior == Dispatch {
        let run_kind = snapshot
            .resolved_gate_resume_run_kind
            .or(if snapshot.has_current_contract {
                Some(RunKind::Executor)
            } else {
                Some(RunKind::Planner)
            })
            .expect("the fallback dispatch role is always present");
        return compatible_action(
            WorkReconciliationAction::QueueRun { run_kind },
            snapshot.has_current_contract,
        );
    }
    if snapshot.stage_behavior == Active {
        if snapshot.approved_review {
            return Ok(WorkReconciliationAction::MoveToReview);
        }
        if snapshot.planner_plan_ready && !snapshot.has_current_contract {
            return Ok(WorkReconciliationAction::MaterializePlannedContractAndQueueExecutor);
        }
        if snapshot.planner_plan_ready && snapshot.has_current_contract {
            return Ok(WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Executor,
            });
        }
        if snapshot.submission_waiting_for_review {
            return queue(RunKind::Reviewer, snapshot.has_current_contract);
        }
        if snapshot.review_requested_changes {
            return if snapshot.review_rounds_exhausted {
                compatible_action(
                    WorkReconciliationAction::OpenRecoveryGate {
                        reason: TaskRecoveryReason::ReviewRoundsExhausted,
                        retry_run_kind: Some(RunKind::Executor),
                    },
                    snapshot.has_current_contract,
                )
            } else {
                queue(RunKind::Executor, snapshot.has_current_contract)
            };
        }
        return action(WorkReconciliationAction::OpenRecoveryGate {
            reason: TaskRecoveryReason::InvariantFault,
            retry_run_kind: None,
        });
    }
    if snapshot.stage_behavior == Acceptance {
        return if snapshot.approved_review {
            Ok(WorkReconciliationAction::Idle)
        } else {
            action(WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::InvariantFault,
                retry_run_kind: None,
            })
        };
    }
    Ok(WorkReconciliationAction::Idle)
}

fn action(action: WorkReconciliationAction) -> Result<WorkReconciliationAction, WorkDomainError> {
    action.validate()?;
    Ok(action)
}

fn queue(
    run_kind: RunKind,
    has_current_contract: bool,
) -> Result<WorkReconciliationAction, WorkDomainError> {
    compatible_action(
        WorkReconciliationAction::QueueRun { run_kind },
        has_current_contract,
    )
}

fn compatible_action(
    action: WorkReconciliationAction,
    has_current_contract: bool,
) -> Result<WorkReconciliationAction, WorkDomainError> {
    if action.validate_for_contract(has_current_contract).is_err() {
        invariant_recovery()
    } else {
        Ok(action)
    }
}

fn validate_role_contract(
    run_kind: RunKind,
    has_current_contract: bool,
) -> Result<(), WorkDomainError> {
    let compatible = match run_kind {
        RunKind::Planner => !has_current_contract,
        RunKind::Executor | RunKind::Reviewer => has_current_contract,
    };
    if compatible {
        Ok(())
    } else {
        Err(invalid_input(
            "reconciliation.run_kind",
            "run role and contract presence are inconsistent",
        ))
    }
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
    has_current_contract: bool,
) -> Result<WorkReconciliationAction, WorkDomainError> {
    validate_failed_run_facts(&facts)?;
    if facts.retryable && !facts.retries_exhausted {
        return queue(facts.run_kind, has_current_contract);
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
    compatible_action(
        WorkReconciliationAction::OpenRecoveryGate {
            reason,
            retry_run_kind,
        },
        has_current_contract,
    )
}

fn invariant_recovery() -> Result<WorkReconciliationAction, WorkDomainError> {
    action(WorkReconciliationAction::OpenRecoveryGate {
        reason: TaskRecoveryReason::InvariantFault,
        retry_run_kind: None,
    })
}

#[cfg(test)]
#[path = "planning/tests.rs"]
mod tests;
