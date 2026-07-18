use serde::{Deserialize, Serialize};

use crate::{
    RunKind, RunStatus, TaskRecoveryReason, WorkDomainError, WorkflowStageBehavior,
    error::invalid_input,
};

/// Semantic operation used by the pure task transition planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkTransition {
    /// Create a task in Inbox.
    Capture,
    /// Atomically create and authorize a delegated task, with an explicit
    /// complete/incomplete intent distinction.
    Delegate { has_complete_intent: bool },
    /// Edit Inbox capture fields.
    UpdateInbox,
    /// Authorize an Inbox task.
    Queue,
    /// Resolve a human gate and resume its opening role.
    Answer { resume_run_kind: RunKind },
    /// Retry a Recovery gate using its explicit continuation role.
    Retry { resume_run_kind: RunKind },
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
pub struct WorkTransitionPlan {
    /// Prior stage behavior, if the command targets an existing task.
    pub from_behavior: Option<WorkflowStageBehavior>,
    /// Resulting stage behavior.
    pub to_behavior: WorkflowStageBehavior,
    /// Whether task generation increments.
    pub generation_increment: bool,
    /// Whether task projection revision increments.
    pub revision_increment: bool,
    /// Runnable role to queue immediately, when deterministic.
    pub queue_run_kind: Option<RunKind>,
    /// Role resumed by Answer/Retry, when the caller supplied one.
    pub resume_run_kind: Option<RunKind>,
}

/// Decide an allowed task transition without reading persistence or text.
pub fn plan_work_transition(
    current: Option<WorkflowStageBehavior>,
    transition: WorkTransition,
) -> Result<WorkTransitionPlan, WorkDomainError> {
    use WorkTransition as Action;
    let result = match (current, transition) {
        (None, Action::Capture) => WorkTransitionPlan {
            from_behavior: None,
            to_behavior: WorkflowStageBehavior::Intake,
            generation_increment: false,
            revision_increment: false,
            queue_run_kind: None,
            resume_run_kind: None,
        },
        (
            None,
            Action::Delegate {
                has_complete_intent,
            },
        ) => WorkTransitionPlan {
            from_behavior: None,
            to_behavior: WorkflowStageBehavior::Dispatch,
            generation_increment: false,
            revision_increment: false,
            queue_run_kind: Some(if has_complete_intent {
                RunKind::Executor
            } else {
                RunKind::Planner
            }),
            resume_run_kind: None,
        },
        (Some(WorkflowStageBehavior::Intake), Action::UpdateInbox) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::Intake,
            generation_increment: false,
            revision_increment: true,
            queue_run_kind: None,
            resume_run_kind: None,
        },
        (Some(WorkflowStageBehavior::Intake), Action::Queue) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::Dispatch,
            generation_increment: false,
            revision_increment: true,
            queue_run_kind: Some(RunKind::Planner),
            resume_run_kind: None,
        },
        (Some(WorkflowStageBehavior::HumanGate), Action::Answer { resume_run_kind }) => {
            WorkTransitionPlan {
                from_behavior: current,
                to_behavior: WorkflowStageBehavior::Dispatch,
                generation_increment: false,
                revision_increment: true,
                queue_run_kind: Some(resume_run_kind),
                resume_run_kind: Some(resume_run_kind),
            }
        }
        (Some(WorkflowStageBehavior::HumanGate), Action::Retry { resume_run_kind }) => {
            WorkTransitionPlan {
                from_behavior: current,
                to_behavior: WorkflowStageBehavior::Dispatch,
                generation_increment: false,
                revision_increment: true,
                queue_run_kind: Some(resume_run_kind),
                resume_run_kind: Some(resume_run_kind),
            }
        }
        (Some(WorkflowStageBehavior::Acceptance), Action::Accept) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::TerminalSuccess,
            generation_increment: false,
            revision_increment: true,
            queue_run_kind: None,
            resume_run_kind: None,
        },
        (Some(WorkflowStageBehavior::Acceptance), Action::RequestChanges) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::Dispatch,
            generation_increment: true,
            revision_increment: true,
            queue_run_kind: Some(RunKind::Executor),
            resume_run_kind: None,
        },
        (
            Some(
                WorkflowStageBehavior::Intake
                | WorkflowStageBehavior::Dispatch
                | WorkflowStageBehavior::Active
                | WorkflowStageBehavior::HumanGate
                | WorkflowStageBehavior::Acceptance,
            ),
            Action::Cancel,
        ) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::TerminalCancelled,
            generation_increment: true,
            revision_increment: true,
            queue_run_kind: None,
            resume_run_kind: None,
        },
        (
            Some(WorkflowStageBehavior::TerminalSuccess | WorkflowStageBehavior::TerminalCancelled),
            Action::Reopen,
        ) => WorkTransitionPlan {
            from_behavior: current,
            to_behavior: WorkflowStageBehavior::Intake,
            generation_increment: true,
            revision_increment: true,
            queue_run_kind: None,
            resume_run_kind: None,
        },
        _ => return Err(WorkDomainError::InvalidTransition),
    };
    Ok(result)
}

/// Durable failed-run facts consumed by reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkFailedRunFacts {
    /// Role of the failed/interrupted run.
    pub run_kind: RunKind,
    /// Terminal/interrupted status.
    pub status: RunStatus,
    /// Whether the failure is safe to retry automatically.
    pub retryable: bool,
    /// Whether the automatic retry bound is exhausted.
    pub retries_exhausted: bool,
    /// Explicit recovery reason when a human gate is required.
    pub recovery_reason: Option<TaskRecoveryReason>,
}

/// Durable facts consumed by the reconciler decision function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkReconciliationSnapshot {
    /// Current task stage behavior.
    pub stage_behavior: WorkflowStageBehavior,
    /// Whether a complete current contract exists.
    pub has_current_contract: bool,
    /// Whether one open current-generation gate exists.
    pub has_open_gate: bool,
    /// Whether one queued/leased/running current-generation run exists.
    pub has_runnable_run: bool,
    /// Explicit role to resume after a resolved Clarification/Approval/Recovery gate.
    pub resolved_gate_resume_run_kind: Option<RunKind>,
    /// Whether the latest current-generation review is complete and approving.
    pub approved_review: bool,
    /// A completed Planner supplied valid contract facts after a crash.
    pub planner_plan_ready: bool,
    /// Current submission has no Reviewer run yet.
    pub submission_waiting_for_review: bool,
    /// Latest review requested automated changes.
    pub review_requested_changes: bool,
    /// Latest review round exhausted its configured bound.
    pub review_rounds_exhausted: bool,
    /// Failed/interrupted current run facts, when one needs recovery planning.
    pub failed_run: Option<WorkFailedRunFacts>,
}

impl Default for WorkReconciliationSnapshot {
    fn default() -> Self {
        Self {
            stage_behavior: WorkflowStageBehavior::Intake,
            has_current_contract: false,
            has_open_gate: false,
            has_runnable_run: false,
            resolved_gate_resume_run_kind: None,
            approved_review: false,
            planner_plan_ready: false,
            submission_waiting_for_review: false,
            review_requested_changes: false,
            review_rounds_exhausted: false,
            failed_run: None,
        }
    }
}

/// One deterministic reconciler action or intentional idle result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkReconciliationAction {
    /// Durable facts intentionally require no mutation.
    Idle,
    /// Queue one role-specific child run.
    QueueRun { run_kind: RunKind },
    /// Atomically materialize the validated Planner plan into an immutable
    /// contract and queue its first Executor.
    MaterializePlannedContractAndQueueExecutor,
    /// Atomically move a resolved Waiting task to Queue and enqueue its role.
    MoveToQueueAndQueueRun { run_kind: RunKind },
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
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if let Self::OpenRecoveryGate {
            reason,
            retry_run_kind,
        } = self
        {
            match (reason, retry_run_kind) {
                (TaskRecoveryReason::InvariantFault, None)
                | (TaskRecoveryReason::InfrastructureRetriesExhausted, Some(_))
                | (TaskRecoveryReason::ReviewRoundsExhausted, Some(RunKind::Executor))
                | (TaskRecoveryReason::UnsafeEffectUncertain, Some(_))
                | (TaskRecoveryReason::ConfigurationUnavailable, Some(_)) => {}
                _ => {
                    return Err(invalid_input(
                        "reconciliation.recovery",
                        "recovery reason and continuation role are inconsistent",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Validate run-role compatibility with the current contract presence.
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
            if validate_role_contract(run_kind, snapshot.has_current_contract).is_err() {
                invariant_recovery()
            } else {
                Ok(WorkReconciliationAction::MoveToQueueAndQueueRun { run_kind })
            }
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
        if validate_role_contract(run_kind, snapshot.has_current_contract).is_err() {
            return invariant_recovery();
        }
        return Ok(WorkReconciliationAction::QueueRun { run_kind });
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
            if validate_role_contract(RunKind::Reviewer, snapshot.has_current_contract).is_err() {
                return invariant_recovery();
            }
            return Ok(WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Reviewer,
            });
        }
        if snapshot.review_requested_changes {
            return if snapshot.review_rounds_exhausted {
                let recovery = WorkReconciliationAction::OpenRecoveryGate {
                    reason: TaskRecoveryReason::ReviewRoundsExhausted,
                    retry_run_kind: Some(RunKind::Executor),
                };
                if recovery
                    .validate_for_contract(snapshot.has_current_contract)
                    .is_err()
                {
                    invariant_recovery()
                } else {
                    Ok(recovery)
                }
            } else {
                if validate_role_contract(RunKind::Executor, snapshot.has_current_contract).is_err()
                {
                    invariant_recovery()
                } else {
                    Ok(WorkReconciliationAction::QueueRun {
                        run_kind: RunKind::Executor,
                    })
                }
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
    if validate_role_contract(facts.run_kind, has_current_contract).is_err() {
        return invariant_recovery();
    }
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
    let result = WorkReconciliationAction::OpenRecoveryGate {
        reason,
        retry_run_kind,
    };
    if result.validate_for_contract(has_current_contract).is_err() {
        invariant_recovery()
    } else {
        Ok(result)
    }
}

fn invariant_recovery() -> Result<WorkReconciliationAction, WorkDomainError> {
    action(WorkReconciliationAction::OpenRecoveryGate {
        reason: TaskRecoveryReason::InvariantFault,
        retry_run_kind: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_matrix_covers_capture_delegate_and_gate_lineage() {
        assert_eq!(
            plan_work_transition(None, WorkTransition::Capture)
                .unwrap()
                .to_behavior,
            WorkflowStageBehavior::Intake
        );
        assert_eq!(
            plan_work_transition(
                None,
                WorkTransition::Delegate {
                    has_complete_intent: false,
                },
            )
            .unwrap()
            .queue_run_kind,
            Some(RunKind::Planner)
        );
        assert_eq!(
            plan_work_transition(
                None,
                WorkTransition::Delegate {
                    has_complete_intent: true,
                },
            )
            .unwrap()
            .queue_run_kind,
            Some(RunKind::Executor)
        );
        let answer = plan_work_transition(
            Some(WorkflowStageBehavior::HumanGate),
            WorkTransition::Answer {
                resume_run_kind: RunKind::Reviewer,
            },
        )
        .unwrap();
        assert_eq!(answer.resume_run_kind, Some(RunKind::Reviewer));
        assert_eq!(answer.queue_run_kind, Some(RunKind::Reviewer));
        let retry = plan_work_transition(
            Some(WorkflowStageBehavior::HumanGate),
            WorkTransition::Retry {
                resume_run_kind: RunKind::Executor,
            },
        )
        .unwrap();
        assert_eq!(retry.queue_run_kind, Some(RunKind::Executor));
    }

    #[test]
    fn transition_matrix_covers_inbox_review_terminal_and_invalid_actions() {
        assert!(
            plan_work_transition(
                Some(WorkflowStageBehavior::Intake),
                WorkTransition::UpdateInbox
            )
            .is_ok()
        );
        assert!(
            plan_work_transition(Some(WorkflowStageBehavior::Intake), WorkTransition::Queue)
                .is_ok()
        );
        let accept = plan_work_transition(
            Some(WorkflowStageBehavior::Acceptance),
            WorkTransition::Accept,
        )
        .unwrap();
        assert_eq!(accept.to_behavior, WorkflowStageBehavior::TerminalSuccess);
        let changes = plan_work_transition(
            Some(WorkflowStageBehavior::Acceptance),
            WorkTransition::RequestChanges,
        )
        .unwrap();
        assert!(changes.generation_increment);
        let cancel =
            plan_work_transition(Some(WorkflowStageBehavior::Active), WorkTransition::Cancel)
                .unwrap();
        assert!(cancel.generation_increment);
        let reopen = plan_work_transition(
            Some(WorkflowStageBehavior::TerminalCancelled),
            WorkTransition::Reopen,
        )
        .unwrap();
        assert_eq!(reopen.to_behavior, WorkflowStageBehavior::Intake);
        assert!(
            plan_work_transition(
                Some(WorkflowStageBehavior::TerminalSuccess),
                WorkTransition::Queue
            )
            .is_err()
        );
    }

    #[test]
    fn reconciliation_covers_handoffs_reviews_failures_and_terminal_fencing() {
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Dispatch,
                has_current_contract: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Executor
            }
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Dispatch,
                resolved_gate_resume_run_kind: Some(RunKind::Reviewer),
                has_current_contract: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Reviewer
            }
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Active,
                approved_review: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::MoveToReview
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Active,
                planner_plan_ready: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::MaterializePlannedContractAndQueueExecutor
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Active,
                has_current_contract: true,
                submission_waiting_for_review: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Reviewer
            }
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Active,
                failed_run: Some(WorkFailedRunFacts {
                    run_kind: RunKind::Executor,
                    status: RunStatus::Interrupted,
                    retryable: true,
                    retries_exhausted: false,
                    recovery_reason: None,
                }),
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Executor
            }
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::Active,
                failed_run: Some(WorkFailedRunFacts {
                    run_kind: RunKind::Reviewer,
                    status: RunStatus::Failed,
                    retryable: false,
                    retries_exhausted: false,
                    recovery_reason: Some(TaskRecoveryReason::ConfigurationUnavailable),
                }),
                has_current_contract: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::ConfigurationUnavailable,
                retry_run_kind: Some(RunKind::Reviewer)
            }
        );
        assert_eq!(
            plan_reconciliation_action(WorkReconciliationSnapshot {
                stage_behavior: WorkflowStageBehavior::TerminalCancelled,
                has_runnable_run: true,
                ..Default::default()
            })
            .unwrap(),
            WorkReconciliationAction::FenceStaleRuns
        );
    }
}

#[cfg(test)]
mod matrix_tests;
