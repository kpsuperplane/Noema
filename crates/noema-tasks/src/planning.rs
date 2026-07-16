use crate::{
    NewTaskSubmission, RunKind, RunStatus, TaskDomainError, TaskReviewCriterion, TaskReviewVerdict,
    TaskStatus, error::invalid_operation, review::validate_review_verdict,
};

/// Maximum automatic child resumptions after infrastructure interruption.
const MAX_AUTOMATIC_RESUMES: i64 = 3;

/// Persisted state needed to validate an executor submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionState {
    /// Owning task id.
    pub task_id: String,
    /// Current task state.
    pub task_status: TaskStatus,
    /// Current task revision.
    pub task_revision_index: i64,
    /// Task's current run id.
    pub latest_run_id: Option<String>,
    /// Candidate executor run id.
    pub run_id: String,
    /// Task id recorded on the run.
    pub run_task_id: String,
    /// Candidate run role.
    pub run_kind: RunKind,
    /// Candidate run state.
    pub run_status: RunStatus,
    /// Candidate run revision.
    pub run_revision_index: i64,
}

/// Normalized submission authorized by a coherent task/run projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionPlan {
    /// Normalized immutable submission.
    pub submission: NewTaskSubmission,
}

/// Normalize an executor submission and validate its task/run linkage.
///
/// # Errors
///
/// Returns a task-domain error when the task or run is not executable, their
/// identities or revisions disagree, or the submission body is malformed.
pub fn plan_submission(
    submission: NewTaskSubmission,
    expected_criterion_ids: &[String],
    state: SubmissionState,
) -> Result<SubmissionPlan, TaskDomainError> {
    let submission = submission.normalized(expected_criterion_ids)?;
    if !matches!(
        state.task_status,
        TaskStatus::Executing | TaskStatus::RevisionRequested
    ) {
        return Err(invalid_operation("task is not executable"));
    }
    if state.run_kind != RunKind::Executor || state.run_status != RunStatus::Running {
        return Err(invalid_operation(
            "submission requires the active executor run",
        ));
    }
    if state.task_id != submission.task_id
        || state.run_task_id != submission.task_id
        || state.run_id != submission.executor_run_id
        || state.latest_run_id.as_deref() != Some(submission.executor_run_id.as_str())
    {
        return Err(invalid_operation(
            "submission task and executor lineage do not match",
        ));
    }
    if state.task_revision_index != submission.revision_index
        || state.run_revision_index != submission.revision_index
    {
        return Err(invalid_operation(
            "submission revision does not match the active task run",
        ));
    }
    Ok(SubmissionPlan { submission })
}

/// Deterministic result of applying a reviewer decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewPlan {
    /// Next durable task status.
    pub task_status: TaskStatus,
    /// Next task revision index.
    pub revision_index: i64,
    /// Whether persistence must queue a new executor run.
    pub queue_executor: bool,
}

/// Validate a review disposition and derive its next task state.
///
/// # Errors
///
/// Returns a task-domain error when the verdict contradicts criterion outcomes
/// or revision arithmetic overflows.
pub fn plan_review(
    revision_index: i64,
    max_review_rounds: i64,
    verdict: TaskReviewVerdict,
    criteria: &[TaskReviewCriterion],
) -> Result<ReviewPlan, TaskDomainError> {
    if revision_index < 0 || max_review_rounds < 1 {
        return Err(invalid_operation(
            "review state has invalid revision bounds",
        ));
    }
    validate_review_verdict(verdict, criteria)?;
    let plan = match verdict {
        TaskReviewVerdict::Approve => ReviewPlan {
            task_status: TaskStatus::Completed,
            revision_index,
            queue_executor: false,
        },
        TaskReviewVerdict::NeedsHuman => ReviewPlan {
            task_status: TaskStatus::WaitingForHuman,
            revision_index,
            queue_executor: false,
        },
        TaskReviewVerdict::RequestChanges => {
            let next_revision = revision_index
                .checked_add(1)
                .ok_or_else(|| invalid_operation("task revision index is exhausted"))?;
            if next_revision < max_review_rounds {
                ReviewPlan {
                    task_status: TaskStatus::RevisionRequested,
                    revision_index: next_revision,
                    queue_executor: true,
                }
            } else {
                ReviewPlan {
                    task_status: TaskStatus::WaitingForHuman,
                    revision_index,
                    queue_executor: false,
                }
            }
        }
    };
    Ok(plan)
}

/// Pure inputs needed to plan an owner-authorized continuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualContinuationInput {
    /// Current task status.
    pub task_status: TaskStatus,
    /// Current task revision.
    pub task_revision_index: i64,
    /// Latest run kind.
    pub parent_run_kind: RunKind,
    /// Latest run status.
    pub parent_run_status: RunStatus,
    /// Latest run attempt.
    pub parent_attempt_index: i64,
    /// Latest run revision.
    pub parent_revision_index: i64,
    /// Whether the parent reviewer already committed a review.
    pub committed_review: bool,
    /// Optional human continuation guidance.
    pub message: Option<String>,
}

/// Deterministic child-run lineage for manual continuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualContinuationPlan {
    /// Child run role.
    pub run_kind: RunKind,
    /// Child attempt index.
    pub attempt_index: i64,
    /// Child revision index.
    pub revision_index: i64,
    /// Next task state.
    pub task_status: TaskStatus,
    /// Normalized optional resume guidance.
    pub message: Option<String>,
}

/// Validate a failed or human-blocked task and plan its child-run lineage.
///
/// # Errors
///
/// Returns a task-domain error when the task/run cannot be continued, human
/// guidance is missing, or lineage arithmetic overflows.
pub fn plan_manual_continuation(
    input: ManualContinuationInput,
) -> Result<ManualContinuationPlan, TaskDomainError> {
    if input.task_revision_index < 0
        || input.parent_attempt_index < 0
        || input.parent_revision_index < 0
        || input.task_revision_index != input.parent_revision_index
    {
        return Err(invalid_operation(
            "task continuation lineage has invalid indexes",
        ));
    }
    if input.committed_review
        && (input.parent_run_kind != RunKind::Reviewer
            || input.parent_run_status != RunStatus::Completed)
    {
        return Err(invalid_operation(
            "committed review continuation requires a completed reviewer run",
        ));
    }
    if !matches!(
        input.task_status,
        TaskStatus::Failed | TaskStatus::WaitingForHuman
    ) {
        return Err(invalid_operation(
            "only failed or human-blocked tasks can be continued",
        ));
    }
    if !matches!(
        input.parent_run_status,
        RunStatus::Failed
            | RunStatus::WaitingForApproval
            | RunStatus::Interrupted
            | RunStatus::Completed
    ) {
        return Err(invalid_operation("latest task run cannot be continued"));
    }
    let message = input
        .message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if input.task_status == TaskStatus::WaitingForHuman && message.is_none() {
        return Err(invalid_operation(
            "human-blocked task continuation requires a message",
        ));
    }

    if input.committed_review {
        let revision_index = input
            .task_revision_index
            .checked_add(1)
            .ok_or_else(|| invalid_operation("task revision index is exhausted"))?;
        return Ok(ManualContinuationPlan {
            run_kind: RunKind::Executor,
            attempt_index: 0,
            revision_index,
            task_status: TaskStatus::Queued,
            message,
        });
    }

    let attempt_index = input
        .parent_attempt_index
        .checked_add(1)
        .ok_or_else(|| invalid_operation("task continuation attempt index is exhausted"))?;
    let task_status = match input.parent_run_kind {
        RunKind::Executor => TaskStatus::Queued,
        RunKind::Reviewer => TaskStatus::Reviewing,
    };
    Ok(ManualContinuationPlan {
        run_kind: input.parent_run_kind,
        attempt_index,
        revision_index: input.parent_revision_index,
        task_status,
        message,
    })
}

/// Pure state needed to decide automatic interrupted-run recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomaticRecoveryInput {
    /// Whether this is still the task's latest run.
    pub is_latest: bool,
    /// Whether cancellation has been requested.
    pub cancellation_requested: bool,
    /// Current task state.
    pub task_status: TaskStatus,
    /// Interrupted run role.
    pub run_kind: RunKind,
    /// Interrupted run attempt index.
    pub attempt_index: i64,
    /// Number of prior automatic resumptions.
    pub retry_count: i64,
}

/// Deterministic recovery disposition for an interrupted run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticRecoveryPlan {
    /// The run is stale, cancelled, or belongs to a closed task.
    NoAction,
    /// Automatic retries are exhausted and the task should fail.
    FailTask,
    /// Queue a child run with the supplied lineage and task state.
    QueueChild {
        /// Child attempt index.
        attempt_index: i64,
        /// Child retry count.
        retry_count: i64,
        /// Task state while the child is queued.
        task_status: TaskStatus,
    },
}

/// Plan recovery without reading or mutating persistence.
///
/// # Errors
///
/// Returns a task-domain error when lineage counters are negative or exhausted.
pub fn plan_automatic_recovery(
    input: AutomaticRecoveryInput,
) -> Result<AutomaticRecoveryPlan, TaskDomainError> {
    if input.attempt_index < 0 || input.retry_count < 0 {
        return Err(invalid_operation(
            "automatic recovery lineage cannot be negative",
        ));
    }
    if !input.is_latest
        || input.cancellation_requested
        || matches!(
            input.task_status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    {
        return Ok(AutomaticRecoveryPlan::NoAction);
    }
    if input.retry_count >= MAX_AUTOMATIC_RESUMES {
        return Ok(AutomaticRecoveryPlan::FailTask);
    }
    let attempt_index = input
        .attempt_index
        .checked_add(1)
        .ok_or_else(|| invalid_operation("automatic recovery attempt index is exhausted"))?;
    let retry_count = input
        .retry_count
        .checked_add(1)
        .ok_or_else(|| invalid_operation("automatic recovery retry count is exhausted"))?;
    let task_status = match input.run_kind {
        RunKind::Executor => TaskStatus::Queued,
        RunKind::Reviewer => TaskStatus::Reviewing,
    };
    Ok(AutomaticRecoveryPlan::QueueChild {
        attempt_index,
        retry_count,
        task_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CriterionOutcome, TaskReviewCriterion, TaskReviewVerdict};

    fn criterion(outcome: CriterionOutcome) -> TaskReviewCriterion {
        TaskReviewCriterion {
            criterion_id: "criterion:1".to_string(),
            outcome,
            evidence_markdown: None,
            feedback: None,
        }
    }

    fn submission() -> NewTaskSubmission {
        NewTaskSubmission {
            submission_id: None,
            task_id: "task:1".to_string(),
            executor_run_id: "run:1".to_string(),
            revision_index: 2,
            summary: "Done".to_string(),
            result_markdown: "Result".to_string(),
            criteria: vec![crate::SubmissionCriterionEvidence {
                criterion_id: "criterion:1".to_string(),
                evidence_markdown: "Evidence".to_string(),
            }],
            artifact_ids: Vec::new(),
        }
    }

    fn submission_state() -> SubmissionState {
        SubmissionState {
            task_id: "task:1".to_string(),
            task_status: TaskStatus::Executing,
            task_revision_index: 2,
            latest_run_id: Some("run:1".to_string()),
            run_id: "run:1".to_string(),
            run_task_id: "task:1".to_string(),
            run_kind: RunKind::Executor,
            run_status: RunStatus::Running,
            run_revision_index: 2,
        }
    }

    #[test]
    fn submission_planner_requires_active_matching_executor_lineage() {
        let expected = vec!["criterion:1".to_string()];
        assert_eq!(
            plan_submission(submission(), &expected, submission_state())
                .expect("active executor submission")
                .submission
                .summary,
            "Done"
        );
        for invalid in [
            SubmissionState {
                task_status: TaskStatus::Reviewing,
                ..submission_state()
            },
            SubmissionState {
                run_kind: RunKind::Reviewer,
                ..submission_state()
            },
            SubmissionState {
                run_status: RunStatus::Completed,
                ..submission_state()
            },
            SubmissionState {
                run_revision_index: 1,
                ..submission_state()
            },
            SubmissionState {
                latest_run_id: Some("run:other".to_string()),
                ..submission_state()
            },
        ] {
            assert!(plan_submission(submission(), &expected, invalid).is_err());
        }
    }

    #[test]
    fn review_planner_preserves_completion_revision_and_bounds_changes() {
        assert_eq!(
            plan_review(
                1,
                3,
                TaskReviewVerdict::Approve,
                &[criterion(CriterionOutcome::Pass)]
            )
            .expect("approval"),
            ReviewPlan {
                task_status: TaskStatus::Completed,
                revision_index: 1,
                queue_executor: false,
            }
        );
        assert!(
            plan_review(
                1,
                3,
                TaskReviewVerdict::NeedsHuman,
                &[criterion(CriterionOutcome::Pass)]
            )
            .is_err()
        );
        assert!(
            plan_review(
                1,
                3,
                TaskReviewVerdict::RequestChanges,
                &[criterion(CriterionOutcome::Uncertain)]
            )
            .is_err()
        );
        assert_eq!(
            plan_review(
                1,
                3,
                TaskReviewVerdict::RequestChanges,
                &[criterion(CriterionOutcome::Fail)]
            )
            .expect("changes"),
            ReviewPlan {
                task_status: TaskStatus::RevisionRequested,
                revision_index: 2,
                queue_executor: true,
            }
        );
        assert_eq!(
            plan_review(
                2,
                3,
                TaskReviewVerdict::RequestChanges,
                &[criterion(CriterionOutcome::Fail)]
            )
            .expect("exhausted changes"),
            ReviewPlan {
                task_status: TaskStatus::WaitingForHuman,
                revision_index: 2,
                queue_executor: false,
            }
        );
    }

    #[test]
    fn continuation_planner_supports_reviewer_and_committed_review_lineage() {
        let reviewer = plan_manual_continuation(ManualContinuationInput {
            task_status: TaskStatus::WaitingForHuman,
            task_revision_index: 1,
            parent_run_kind: RunKind::Reviewer,
            parent_run_status: RunStatus::WaitingForApproval,
            parent_attempt_index: 2,
            parent_revision_index: 1,
            committed_review: false,
            message: Some("continue review".to_string()),
        })
        .expect("reviewer continuation");
        assert_eq!(reviewer.run_kind, RunKind::Reviewer);
        assert_eq!(reviewer.task_status, TaskStatus::Reviewing);
        assert_eq!(reviewer.attempt_index, 3);

        let executor = plan_manual_continuation(ManualContinuationInput {
            committed_review: true,
            ..ManualContinuationInput {
                task_status: TaskStatus::Failed,
                task_revision_index: 1,
                parent_run_kind: RunKind::Reviewer,
                parent_run_status: RunStatus::Completed,
                parent_attempt_index: 1,
                parent_revision_index: 1,
                committed_review: false,
                message: None,
            }
        })
        .expect("post-review continuation");
        assert_eq!(executor.run_kind, RunKind::Executor);
        assert_eq!(executor.revision_index, 2);
        assert_eq!(executor.attempt_index, 0);
        assert!(
            plan_manual_continuation(ManualContinuationInput {
                parent_attempt_index: -1,
                ..ManualContinuationInput {
                    task_status: TaskStatus::Failed,
                    task_revision_index: 1,
                    parent_run_kind: RunKind::Executor,
                    parent_run_status: RunStatus::Failed,
                    parent_attempt_index: 0,
                    parent_revision_index: 1,
                    committed_review: false,
                    message: None,
                }
            })
            .is_err()
        );
        assert!(
            plan_manual_continuation(ManualContinuationInput {
                committed_review: true,
                ..ManualContinuationInput {
                    task_status: TaskStatus::Failed,
                    task_revision_index: 1,
                    parent_run_kind: RunKind::Executor,
                    parent_run_status: RunStatus::Failed,
                    parent_attempt_index: 0,
                    parent_revision_index: 1,
                    committed_review: false,
                    message: None,
                }
            })
            .is_err()
        );
    }

    #[test]
    fn recovery_planner_retries_reviewers_and_exhausts_after_three() {
        assert_eq!(
            plan_automatic_recovery(AutomaticRecoveryInput {
                is_latest: true,
                cancellation_requested: false,
                task_status: TaskStatus::Reviewing,
                run_kind: RunKind::Reviewer,
                attempt_index: 2,
                retry_count: 2,
            })
            .expect("recovery"),
            AutomaticRecoveryPlan::QueueChild {
                attempt_index: 3,
                retry_count: 3,
                task_status: TaskStatus::Reviewing,
            }
        );
        assert_eq!(
            plan_automatic_recovery(AutomaticRecoveryInput {
                is_latest: true,
                cancellation_requested: false,
                task_status: TaskStatus::Reviewing,
                run_kind: RunKind::Reviewer,
                attempt_index: 3,
                retry_count: 3,
            })
            .expect("exhausted"),
            AutomaticRecoveryPlan::FailTask
        );
        assert_eq!(
            plan_automatic_recovery(AutomaticRecoveryInput {
                is_latest: false,
                cancellation_requested: false,
                task_status: TaskStatus::Executing,
                run_kind: RunKind::Executor,
                attempt_index: 1,
                retry_count: 1,
            })
            .expect("stale recovery"),
            AutomaticRecoveryPlan::NoAction
        );
        assert!(
            plan_automatic_recovery(AutomaticRecoveryInput {
                is_latest: true,
                cancellation_requested: false,
                task_status: TaskStatus::Executing,
                run_kind: RunKind::Executor,
                attempt_index: i64::MAX,
                retry_count: 0,
            })
            .is_err()
        );
    }
}
