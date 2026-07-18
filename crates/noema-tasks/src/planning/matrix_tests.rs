use super::*;
use crate::SafeErrorCode;

#[test]
fn reported_failure_matrix_preserves_recovery_authority() {
    struct Case {
        error_code: &'static str,
        retryable: bool,
        retries_exhausted: bool,
        expected: WorkReconciliationAction,
    }

    let cases = [
        Case {
            error_code: "temporary_transport",
            retryable: true,
            retries_exhausted: false,
            expected: WorkReconciliationAction::QueueRun {
                run_kind: RunKind::Executor,
            },
        },
        Case {
            error_code: "temporary_transport",
            retryable: true,
            retries_exhausted: true,
            expected: WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::InfrastructureRetriesExhausted,
                retry_run_kind: Some(RunKind::Executor),
            },
        },
        Case {
            error_code: "unsafe_effect_uncertain",
            retryable: true,
            retries_exhausted: false,
            expected: WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::UnsafeEffectUncertain,
                retry_run_kind: Some(RunKind::Executor),
            },
        },
        Case {
            error_code: "configuration_unavailable",
            retryable: true,
            retries_exhausted: false,
            expected: WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::ConfigurationUnavailable,
                retry_run_kind: Some(RunKind::Executor),
            },
        },
        Case {
            error_code: "invariant_fault",
            retryable: true,
            retries_exhausted: false,
            expected: WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::InvariantFault,
                retry_run_kind: None,
            },
        },
        Case {
            error_code: "unclassified_permanent_failure",
            retryable: false,
            retries_exhausted: false,
            expected: WorkReconciliationAction::OpenRecoveryGate {
                reason: TaskRecoveryReason::InvariantFault,
                retry_run_kind: None,
            },
        },
    ];

    for case in cases {
        let error_code = SafeErrorCode::new(case.error_code).expect("safe error code");
        let facts = reported_failure_facts(
            RunKind::Executor,
            RunStatus::Failed,
            &error_code,
            case.retryable,
            case.retries_exhausted,
        );
        let action = plan_reconciliation_action(WorkReconciliationSnapshot {
            stage_behavior: WorkflowStageBehavior::Active,
            has_current_contract: true,
            failed_run: Some(facts),
            ..Default::default()
        })
        .expect("reported failure should produce one safe action");
        assert_eq!(action, case.expected, "error code {}", case.error_code);
    }
}

#[test]
fn reconciliation_failure_and_gate_matrix_is_fail_closed() {
    let queue_failure = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Dispatch,
        has_current_contract: true,
        failed_run: Some(WorkFailedRunFacts {
            run_kind: RunKind::Executor,
            status: RunStatus::Failed,
            retryable: true,
            retries_exhausted: false,
            recovery_reason: None,
        }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        queue_failure,
        WorkReconciliationAction::QueueRun {
            run_kind: RunKind::Executor
        }
    );

    let contradictory = WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Dispatch,
        has_current_contract: true,
        failed_run: Some(WorkFailedRunFacts {
            run_kind: RunKind::Executor,
            status: RunStatus::Failed,
            retryable: true,
            retries_exhausted: false,
            recovery_reason: Some(TaskRecoveryReason::InvariantFault),
        }),
        ..Default::default()
    };
    assert!(plan_reconciliation_action(contradictory).is_err());

    let missing_reason = WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Active,
        has_current_contract: true,
        failed_run: Some(WorkFailedRunFacts {
            run_kind: RunKind::Executor,
            status: RunStatus::Failed,
            retryable: false,
            retries_exhausted: false,
            recovery_reason: None,
        }),
        ..Default::default()
    };
    assert!(plan_reconciliation_action(missing_reason).is_err());

    let waiting_stale = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::HumanGate,
        has_open_gate: true,
        has_runnable_run: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(waiting_stale, WorkReconciliationAction::FenceStaleRuns);

    let waiting_resolved = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::HumanGate,
        has_current_contract: true,
        resolved_gate_resume_run_kind: Some(RunKind::Executor),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        waiting_resolved,
        WorkReconciliationAction::MoveToQueueAndQueueRun {
            run_kind: RunKind::Executor
        }
    );

    let waiting_resolved_with_runnable_run =
        plan_reconciliation_action(WorkReconciliationSnapshot {
            stage_behavior: WorkflowStageBehavior::HumanGate,
            has_current_contract: true,
            has_runnable_run: true,
            resolved_gate_resume_run_kind: Some(RunKind::Executor),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        waiting_resolved_with_runnable_run,
        WorkReconciliationAction::FenceStaleRuns
    );

    let role_mismatch = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Dispatch,
        has_current_contract: true,
        resolved_gate_resume_run_kind: Some(RunKind::Planner),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        role_mismatch,
        WorkReconciliationAction::OpenRecoveryGate {
            reason: TaskRecoveryReason::InvariantFault,
            retry_run_kind: None,
        }
    );

    for (reason, role) in [
        (
            TaskRecoveryReason::InfrastructureRetriesExhausted,
            Some(RunKind::Executor),
        ),
        (
            TaskRecoveryReason::UnsafeEffectUncertain,
            Some(RunKind::Reviewer),
        ),
        (
            TaskRecoveryReason::ConfigurationUnavailable,
            Some(RunKind::Planner),
        ),
        (
            TaskRecoveryReason::ReviewRoundsExhausted,
            Some(RunKind::Executor),
        ),
        (TaskRecoveryReason::InvariantFault, None),
    ] {
        WorkReconciliationAction::OpenRecoveryGate {
            reason,
            retry_run_kind: role,
        }
        .validate()
        .unwrap();
    }
}

#[test]
fn active_review_handoffs_require_a_current_contract() {
    let submission_without_contract = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Active,
        submission_waiting_for_review: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        submission_without_contract,
        WorkReconciliationAction::OpenRecoveryGate {
            reason: TaskRecoveryReason::InvariantFault,
            retry_run_kind: None,
        }
    );

    let changes_without_contract = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Active,
        review_requested_changes: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(changes_without_contract, submission_without_contract);

    let exhausted_without_contract = plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Active,
        review_requested_changes: true,
        review_rounds_exhausted: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(exhausted_without_contract, submission_without_contract);
}
