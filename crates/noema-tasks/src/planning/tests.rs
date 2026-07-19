use super::*;

#[test]
fn transition_matrix_covers_capture_delegate_and_gate_lineage() {
    let capture = plan_work_transition(None, WorkTransition::Capture).unwrap();
    assert_eq!(capture.to_behavior, WorkflowStageBehavior::Intake);
    for (has_complete_intent, expected) in [(false, RunKind::Planner), (true, RunKind::Executor)] {
        let plan = plan_work_transition(
            None,
            WorkTransition::Delegate {
                has_complete_intent,
            },
        )
        .unwrap();
        assert_eq!(plan.queue_run_kind, Some(expected));
    }
    for transition in [
        WorkTransition::Answer {
            resume_run_kind: RunKind::Reviewer,
        },
        WorkTransition::Retry {
            resume_run_kind: RunKind::Reviewer,
        },
    ] {
        let plan =
            plan_work_transition(Some(WorkflowStageBehavior::HumanGate), transition).unwrap();
        assert_eq!(plan.resume_run_kind, Some(RunKind::Reviewer));
        assert_eq!(plan.queue_run_kind, Some(RunKind::Reviewer));
    }
}

#[test]
fn transition_matrix_covers_inbox_review_terminal_and_invalid_actions() {
    assert!(
        plan_work_transition(Some(WorkflowStageBehavior::Intake), WorkTransition::Queue).is_ok()
    );
    let changes = plan_work_transition(
        Some(WorkflowStageBehavior::Acceptance),
        WorkTransition::RequestChanges,
    )
    .unwrap();
    assert!(changes.generation_increment);
    let reopen = plan_work_transition(
        Some(WorkflowStageBehavior::TerminalCancelled),
        WorkTransition::Reopen,
    )
    .unwrap();
    assert_eq!(reopen.to_behavior, WorkflowStageBehavior::Intake);
    assert!(
        plan_work_transition(
            Some(WorkflowStageBehavior::TerminalSuccess),
            WorkTransition::Queue,
        )
        .is_err()
    );
}
