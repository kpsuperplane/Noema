//! Planner terminal replay and exhausted-review notification regressions.

use noema_tasks::{
    CancelTask, CaptureTask, CommandMeta, CriterionOutcome, NewTaskReview, NewTaskSubmission,
    NewTaskValidationCriterion, ReopenTask, RunKind, SubmissionCriterionEvidence, TaskComplexity,
    TaskPrecondition, TaskProvenance, TaskReviewCriterion, TaskReviewVerdict, TaskSourceKind,
    WorkCommand, WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    CompletePlan, NoemaStore, PlanTerminal, StoreError, SubmitPlan, SubmitTaskResult,
    SubmitTaskReview, WorkCommandService, WorkRunFence, WorkRunTerminal,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:agent:planner-replay";

struct PlannerFixture {
    store: NoemaStore,
    service: WorkCommandService,
    task: noema_tasks::TaskRecord,
    fence: WorkRunFence,
}

async fn planner_fixture(max_review_rounds: Option<u32>) -> PlannerFixture {
    let store = open_ephemeral_store().await.expect("open store");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    if let Some(max_review_rounds) = max_review_rounds {
        store
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE task_execution_policy SET max_review_rounds = ?1 WHERE policy_id = 'default'",
                        [max_review_rounds],
                    )
                    .map(|_| ())
                    .map_err(StoreError::Sqlite)
            })
            .await
            .expect("set review bound");
    }
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);
    let captured = service
        .execute(WorkCommand::CaptureTask(CaptureTask {
            meta: metadata("planner:capture"),
            workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
            title: "Planner replay task".to_string(),
            description_markdown: "Exercise durable planner terminals".to_string(),
            project_id: None,
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::WorkUi,
                created_by_actor_id: "actor:human:local".to_string(),
                ..TaskProvenance::default()
            },
        }))
        .await
        .expect("capture task")
        .task
        .expect("captured task");
    let task = service
        .execute(WorkCommand::QueueTask(noema_tasks::QueueTask {
            meta: metadata("planner:queue"),
            precondition: TaskPrecondition {
                task_id: captured.task_id,
                expected_revision: captured.revision,
                expected_generation: captured.generation,
            },
        }))
        .await
        .expect("queue task")
        .task
        .expect("queued task");
    let claim = service
        .claim_next_work_run("worker:planner-replay", 60, &[])
        .await
        .expect("claim planner")
        .expect("planner run");
    assert_eq!(claim.run.run_kind, RunKind::Planner);
    let fence = WorkRunFence {
        run_id: claim.run.run_id,
        lease_token: claim.lease_token,
        task_generation: task.generation,
        contract_id: None,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:planner:start")
        .await
        .expect("start planner");
    PlannerFixture {
        store,
        service,
        task,
        fence,
    }
}

fn metadata(key: &str) -> CommandMeta {
    CommandMeta {
        actor_id: "actor:human:local".to_string(),
        causation_id: None,
        correlation_id: format!("correlation:{key}"),
        idempotency_key: Some(format!("idem:{key}")),
    }
}

fn complete_plan(fixture: &PlannerFixture, request: &str) -> WorkRunTerminal {
    WorkRunTerminal::Plan(SubmitPlan {
        fence: fixture.fence.clone(),
        terminal: PlanTerminal::Complete(CompletePlan {
            request_markdown: request.to_string(),
            execution_plan_markdown: "Perform the bounded plan steps.".to_string(),
            criteria: vec![NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "The result contains durable evidence.".to_string(),
                expected_evidence: Some("One evidence line".to_string()),
            }],
            complexity: TaskComplexity::Simple,
        }),
    })
}

fn blocking_plan(fixture: &PlannerFixture, prompt: &str) -> WorkRunTerminal {
    WorkRunTerminal::Plan(SubmitPlan {
        fence: fixture.fence.clone(),
        terminal: PlanTerminal::BlockingQuestion {
            prompt_markdown: prompt.to_string(),
            context_markdown: "The planner needs one bounded decision.".to_string(),
            gate_kind: noema_tasks::TaskGateKind::Clarification,
        },
    })
}

async fn count(store: &NoemaStore, sql: &str, id: &str) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row(sql, [id], |row| row.get(0))
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("count durable rows")
}

async fn event_kind(store: &NoemaStore, sequence: u64) -> String {
    let sequence = i64::try_from(sequence).expect("event sequence fits SQLite");
    store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT event_kind FROM work_events WHERE event_sequence = ?1",
                    [sequence],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load event kind")
}

#[tokio::test]
async fn planner_complete_replays_after_lease_clear_with_the_original_queue_marker() {
    let fixture = planner_fixture(None).await;
    let terminal = complete_plan(&fixture, "Execute the replay test.");
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:planner:complete",
        )
        .await
        .expect("complete planner");
    assert_eq!(
        event_kind(&fixture.store, first.event_sequence).await,
        "run.queued"
    );

    let claimed_child = fixture
        .service
        .claim_next_work_run("worker:planner-child", 60, &[])
        .await
        .expect("claim executor child")
        .expect("executor child");
    assert_eq!(claimed_child.run.run_kind, RunKind::Executor);

    let current = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load planned task")
        .expect("planned task exists")
        .task;
    let cancelled = fixture
        .service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: metadata("planner:complete:cancel"),
            precondition: TaskPrecondition {
                task_id: current.task_id,
                expected_revision: current.revision,
                expected_generation: current.generation,
            },
            reason: Some("Advance generation before planner replay.".to_string()),
        }))
        .await
        .expect("cancel planned task")
        .task
        .expect("cancelled task");
    let reopened = fixture
        .service
        .execute(WorkCommand::ReopenTask(ReopenTask {
            meta: metadata("planner:complete:reopen"),
            precondition: TaskPrecondition {
                task_id: cancelled.task_id,
                expected_revision: cancelled.revision,
                expected_generation: cancelled.generation,
            },
        }))
        .await
        .expect("reopen planned task")
        .task
        .expect("reopened task");
    assert!(reopened.generation > fixture.fence.task_generation);

    let replay = fixture
        .service
        .record_work_run_terminal(terminal, ACTOR, None, "correlation:planner:complete-replay")
        .await
        .expect("late complete replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.contract_id, first.contract_id);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_execution_contracts WHERE task_id = ?1 AND origin = 'planned'",
            fixture.task.task_id.as_str(),
        )
        .await,
        1
    );

    let error = fixture
        .service
        .record_work_run_terminal(
            complete_plan(&fixture, "A divergent executable request."),
            ACTOR,
            None,
            "correlation:planner:complete-divergent",
        )
        .await
        .expect_err("divergent completed plan must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn planner_blocking_question_replays_one_gate_and_waiting_notification() {
    let fixture = planner_fixture(None).await;
    let terminal = blocking_plan(&fixture, "Which bounded option should be used?");
    let first = fixture
        .service
        .record_work_run_terminal(terminal.clone(), ACTOR, None, "correlation:planner:block")
        .await
        .expect("block planner");
    assert_eq!(
        event_kind(&fixture.store, first.event_sequence).await,
        "notification.queued"
    );
    let replay = fixture
        .service
        .record_work_run_terminal(terminal, ACTOR, None, "correlation:planner:block-replay")
        .await
        .expect("late blocking replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_gates WHERE originating_run_id = ?1",
            fixture.fence.run_id.as_str(),
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM work_notification_outbox o JOIN work_events e ON e.event_sequence = o.event_sequence WHERE e.run_id = ?1 AND o.notification_kind = 'task_waiting'",
            fixture.fence.run_id.as_str(),
        )
        .await,
        1
    );

    let error = fixture
        .service
        .record_work_run_terminal(
            blocking_plan(&fixture, "A different question."),
            ACTOR,
            None,
            "correlation:planner:block-divergent",
        )
        .await
        .expect_err("divergent blocking question must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn exhausted_review_changes_enqueue_and_replay_one_recovery_notification() {
    let fixture = planner_fixture(Some(1)).await;
    fixture
        .service
        .record_work_run_terminal(
            complete_plan(&fixture, "Execute the recovery notification test."),
            ACTOR,
            None,
            "correlation:recovery:plan",
        )
        .await
        .expect("complete planner");
    let detail = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load planned task")
        .expect("task exists");
    let task = detail.task;
    let contract = detail.current_contract.expect("planned contract");
    let executor = fixture
        .service
        .claim_next_work_run("worker:recovery:executor", 60, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    let executor_fence = WorkRunFence {
        run_id: executor.run.run_id.clone(),
        lease_token: executor.lease_token,
        task_generation: task.generation,
        contract_id: Some(contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &executor_fence,
            ACTOR,
            None,
            "correlation:recovery:executor",
        )
        .await
        .expect("start executor");
    fixture
        .service
        .record_work_run_terminal(
            WorkRunTerminal::TaskResult(SubmitTaskResult {
                fence: executor_fence,
                submission: NewTaskSubmission {
                    submission_id: Some("submission:recovery-notification".to_string()),
                    task_id: task.task_id.clone(),
                    contract_id: contract.contract_id.clone(),
                    executor_run_id: executor.run.run_id,
                    review_round: 1,
                    summary: "Recovery-bound result".to_string(),
                    result_markdown: "A result that the reviewer rejects.".to_string(),
                    criteria: contract
                        .criteria
                        .iter()
                        .map(|criterion| SubmissionCriterionEvidence {
                            criterion_id: criterion.criterion_id.clone(),
                            evidence_markdown: "Executor evidence".to_string(),
                        })
                        .collect(),
                    artifact_ids: Vec::new(),
                },
            }),
            ACTOR,
            None,
            "correlation:recovery:submission",
        )
        .await
        .expect("submit result");
    let reviewer = fixture
        .service
        .claim_next_work_run("worker:recovery:reviewer", 60, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let reviewer_fence = WorkRunFence {
        run_id: reviewer.run.run_id.clone(),
        lease_token: reviewer.lease_token,
        task_generation: task.generation,
        contract_id: Some(contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:recovery:reviewer",
        )
        .await
        .expect("start reviewer");
    let terminal = WorkRunTerminal::Review(SubmitTaskReview {
        fence: reviewer_fence,
        review: NewTaskReview {
            review_id: Some("review:recovery-notification".to_string()),
            task_id: task.task_id.clone(),
            contract_id: contract.contract_id.clone(),
            reviewer_run_id: reviewer.run.run_id.clone(),
            reviewed_submission_id: "submission:recovery-notification".to_string(),
            review_attempt_index: 1,
            supersedes_review_id: None,
            overall_verdict: TaskReviewVerdict::RequestChanges,
            human_gate_kind: None,
            overall_feedback: "The review bound is exhausted.".to_string(),
            criteria: contract
                .criteria
                .iter()
                .map(|criterion| TaskReviewCriterion {
                    criterion_id: criterion.criterion_id.clone(),
                    outcome: CriterionOutcome::Fail,
                    evidence_markdown: Some("Reviewer evidence".to_string()),
                    feedback: Some("Needs human-authorized recovery".to_string()),
                })
                .collect(),
        },
    });
    let first = fixture
        .service
        .record_work_run_terminal(terminal.clone(), ACTOR, None, "correlation:recovery:review")
        .await
        .expect("open recovery gate");
    assert_eq!(
        event_kind(&fixture.store, first.event_sequence).await,
        "notification.queued"
    );
    assert!(first.gate_id.is_some());
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM work_notification_outbox o JOIN work_events e ON e.event_sequence = o.event_sequence WHERE e.run_id = ?1 AND o.notification_kind = 'task_recovery' AND o.destination_id = 'human:local'",
            reviewer.run.run_id.as_str(),
        )
        .await,
        1
    );

    let replay = fixture
        .service
        .record_work_run_terminal(terminal, ACTOR, None, "correlation:recovery:review-replay")
        .await
        .expect("replay exhausted review");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM work_notification_outbox o JOIN work_events e ON e.event_sequence = o.event_sequence WHERE e.run_id = ?1 AND o.notification_kind = 'task_recovery'",
            reviewer.run.run_id.as_str(),
        )
        .await,
        1
    );
}
