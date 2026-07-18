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
        plan_work_transition(Some(WorkflowStageBehavior::Intake), WorkTransition::Queue).is_ok()
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
        plan_work_transition(Some(WorkflowStageBehavior::Active), WorkTransition::Cancel).unwrap();
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
            has_current_contract: true,
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
