//! Focused terminal-evidence replay and lease-fence tests.

mod artifacts;
mod generation_advance;
mod replay_after_resolution;

use noema_tasks::{
    AnswerTask, CancelTask, CaptureTask, CommandMeta, CriterionOutcome, NewTaskReview,
    NewTaskSubmission, NewTaskValidationCriterion, RetryTask, RunKind, SubmissionCriterionEvidence,
    TaskComplexity, TaskGateAnswer, TaskGateKind, TaskPrecondition, TaskProvenance,
    TaskReviewCriterion, TaskReviewVerdict, TaskSourceKind, WorkCommand, WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    CompletePlan, NoemaStore, PlanTerminal, ReportRunFailure, ReportTaskBlocked, StoreError,
    SubmitPlan, SubmitTaskResult, SubmitTaskReview, WorkCommandService, WorkRunFence,
    WorkRunTerminal, WorkTaskValidAction,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:agent:test";

struct ExecutorFixture {
    store: NoemaStore,
    service: WorkCommandService,
    task: noema_tasks::TaskRecord,
    contract: noema_tasks::TaskExecutionContract,
    executor_fence: WorkRunFence,
}

async fn fixture() -> ExecutorFixture {
    let store = open_ephemeral_store().await.expect("open store");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);

    let captured = service
        .execute(capture_command())
        .await
        .expect("capture task")
        .task
        .expect("captured task");
    let queued = service
        .execute(WorkCommand::QueueTask(noema_tasks::QueueTask {
            meta: metadata("terminal:queue"),
            precondition: TaskPrecondition {
                task_id: captured.task_id.clone(),
                expected_revision: captured.revision,
                expected_generation: captured.generation,
            },
        }))
        .await
        .expect("queue task")
        .task
        .expect("queued task");

    let planner_claim = service
        .claim_next_work_run("worker:terminal:planner", 60, &[])
        .await
        .expect("claim planner")
        .expect("planner run");
    let planner_fence = WorkRunFence {
        run_id: planner_claim.run.run_id.clone(),
        lease_token: planner_claim.lease_token.clone(),
        task_generation: queued.generation,
        contract_id: None,
    };
    service
        .start_work_run(&planner_fence, ACTOR, None, "correlation:terminal:planner")
        .await
        .expect("start planner");
    let plan = WorkRunTerminal::Plan(SubmitPlan {
        fence: planner_fence.clone(),
        terminal: PlanTerminal::Complete(CompletePlan {
            request_markdown: "Execute the terminal evidence test.".to_string(),
            execution_plan_markdown: "Perform the bounded test steps.".to_string(),
            criteria: vec![NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "The result contains exact evidence.".to_string(),
                expected_evidence: Some("A concise evidence line".to_string()),
            }],
            complexity: TaskComplexity::Simple,
        }),
    });
    let planned = service
        .record_work_run_terminal(plan, ACTOR, None, "correlation:terminal:planner")
        .await
        .expect("complete planner");
    let detail = service
        .store()
        .get_work_task(&queued.task_id)
        .await
        .expect("load planned task")
        .expect("planned task exists");
    let task = detail.task;
    let contract = detail.current_contract.expect("planned contract");
    assert_eq!(planned.contract_id.as_ref(), Some(&contract.contract_id));

    let executor_claim = service
        .claim_next_work_run("worker:terminal:executor", 60, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    assert_eq!(executor_claim.run.run_kind, RunKind::Executor);
    let executor_fence = WorkRunFence {
        run_id: executor_claim.run.run_id.clone(),
        lease_token: executor_claim.lease_token.clone(),
        task_generation: task.generation,
        contract_id: Some(contract.contract_id.clone()),
    };
    service
        .start_work_run(
            &executor_fence,
            ACTOR,
            None,
            "correlation:terminal:executor",
        )
        .await
        .expect("start executor");

    ExecutorFixture {
        store,
        service,
        task,
        contract,
        executor_fence,
    }
}

fn metadata(key: &str) -> CommandMeta {
    CommandMeta {
        actor_id: "actor:human:local".to_string(),
        causation_id: None,
        correlation_id: format!("correlation:{key}"),
        idempotency_key: Some(key.to_string()),
    }
}

fn capture_command() -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: metadata("terminal:capture"),
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: "terminal evidence task".to_string(),
        description_markdown: "test planner/executor/reviewer terminals".to_string(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: "actor:human:local".to_string(),
            ..TaskProvenance::default()
        },
    })
}

fn executor_submission(
    fixture: &ExecutorFixture,
    summary: &str,
    evidence: &str,
    artifact_ids: Vec<String>,
) -> WorkRunTerminal {
    WorkRunTerminal::TaskResult(SubmitTaskResult {
        fence: fixture.executor_fence.clone(),
        submission: NewTaskSubmission {
            submission_id: Some("submission:terminal-test".to_string()),
            task_id: fixture.task.task_id.clone(),
            contract_id: fixture.contract.contract_id.clone(),
            executor_run_id: fixture.executor_fence.run_id.clone(),
            review_round: 1,
            summary: summary.to_string(),
            result_markdown: "A durable result body.".to_string(),
            criteria: fixture
                .contract
                .criteria
                .iter()
                .map(|criterion| SubmissionCriterionEvidence {
                    criterion_id: criterion.criterion_id.clone(),
                    evidence_markdown: evidence.to_string(),
                })
                .collect(),
            artifact_ids,
        },
    })
}

async fn running_reviewer(fixture: &ExecutorFixture, worker: &str) -> WorkRunFence {
    fixture
        .service
        .record_work_run_terminal(
            executor_submission(fixture, "summary", "evidence", Vec::new()),
            ACTOR,
            None,
            &format!("correlation:{worker}:executor"),
        )
        .await
        .expect("submit executor result");
    let claim = fixture
        .service
        .claim_next_work_run(worker, 60, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let fence = WorkRunFence {
        run_id: claim.run.run_id,
        lease_token: claim.lease_token,
        task_generation: fixture.task.generation,
        contract_id: Some(fixture.contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(&fence, ACTOR, None, &format!("correlation:{worker}:start"))
        .await
        .expect("start reviewer");
    fence
}

fn review_terminal(
    fixture: &ExecutorFixture,
    fence: &WorkRunFence,
    review_id: &str,
    verdict: TaskReviewVerdict,
    gate_kind: Option<TaskGateKind>,
) -> WorkRunTerminal {
    WorkRunTerminal::Review(SubmitTaskReview {
        fence: fence.clone(),
        review: NewTaskReview {
            review_id: Some(review_id.to_string()),
            task_id: fixture.task.task_id.clone(),
            contract_id: fixture.contract.contract_id.clone(),
            reviewer_run_id: fence.run_id.clone(),
            reviewed_submission_id: "submission:terminal-test".to_string(),
            review_attempt_index: 1,
            supersedes_review_id: None,
            overall_verdict: verdict,
            human_gate_kind: gate_kind,
            overall_feedback: "Human decision required".to_string(),
            criteria: fixture
                .contract
                .criteria
                .iter()
                .map(|criterion| TaskReviewCriterion {
                    criterion_id: criterion.criterion_id.clone(),
                    outcome: match verdict {
                        TaskReviewVerdict::Approve => CriterionOutcome::Pass,
                        TaskReviewVerdict::RequestChanges => CriterionOutcome::Fail,
                        TaskReviewVerdict::NeedsHuman => CriterionOutcome::Uncertain,
                    },
                    evidence_markdown: Some("review evidence".to_string()),
                    feedback: Some("review feedback".to_string()),
                })
                .collect(),
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

async fn count_without_id(store: &NoemaStore, sql: &str) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("count durable rows")
}

#[tokio::test]
async fn reported_failure_recovery_matrix_matches_advertised_actions() {
    struct Case {
        error_code: &'static str,
        retryable: bool,
        exhaust_retries: bool,
        expected_reason: noema_tasks::TaskRecoveryReason,
        expected_actions: Vec<WorkTaskValidAction>,
    }

    let cases = [
        Case {
            error_code: "temporary_transport",
            retryable: true,
            exhaust_retries: true,
            expected_reason: noema_tasks::TaskRecoveryReason::InfrastructureRetriesExhausted,
            expected_actions: vec![
                WorkTaskValidAction::Answer,
                WorkTaskValidAction::Retry,
                WorkTaskValidAction::Cancel,
            ],
        },
        Case {
            error_code: "unsafe_effect_uncertain",
            retryable: true,
            exhaust_retries: false,
            expected_reason: noema_tasks::TaskRecoveryReason::UnsafeEffectUncertain,
            expected_actions: vec![WorkTaskValidAction::Answer, WorkTaskValidAction::Cancel],
        },
        Case {
            error_code: "configuration_unavailable",
            retryable: true,
            exhaust_retries: false,
            expected_reason: noema_tasks::TaskRecoveryReason::ConfigurationUnavailable,
            expected_actions: vec![WorkTaskValidAction::Retry, WorkTaskValidAction::Cancel],
        },
        Case {
            error_code: "invariant_fault",
            retryable: true,
            exhaust_retries: false,
            expected_reason: noema_tasks::TaskRecoveryReason::InvariantFault,
            expected_actions: vec![WorkTaskValidAction::Cancel],
        },
        Case {
            error_code: "unclassified_permanent_failure",
            retryable: false,
            exhaust_retries: false,
            expected_reason: noema_tasks::TaskRecoveryReason::InvariantFault,
            expected_actions: vec![WorkTaskValidAction::Cancel],
        },
    ];

    for case in cases {
        let fixture = fixture().await;
        if case.exhaust_retries {
            fixture
                .store
                .with_connection(|connection| {
                    connection
                        .execute(
                            "UPDATE agent_runs SET max_automatic_retries = 0 WHERE run_id = ?1",
                            [fixture.executor_fence.run_id.as_str()],
                        )
                        .map_err(StoreError::Sqlite)
                })
                .await
                .expect("exhaust the run's frozen retry budget");
        }
        fixture
            .service
            .report_work_run_failure(
                ReportRunFailure {
                    fence: fixture.executor_fence.clone(),
                    status: noema_tasks::RunStatus::Failed,
                    error_code: noema_tasks::SafeErrorCode::new(case.error_code)
                        .expect("safe error code"),
                    error_message: None,
                    retryable: case.retryable,
                },
                ACTOR,
                None,
                &format!("correlation:failure-matrix:{}", case.error_code),
            )
            .await
            .expect("reported failure opens its planned recovery gate");
        let waiting = fixture
            .store
            .get_work_task(&fixture.task.task_id)
            .await
            .expect("load matrix recovery")
            .expect("matrix task exists");
        let gate = waiting.active_gate.expect("matrix recovery gate");
        assert_eq!(
            gate.recovery_reason,
            Some(case.expected_reason),
            "error code {}",
            case.error_code
        );
        assert_eq!(
            waiting.valid_actions, case.expected_actions,
            "error code {}",
            case.error_code
        );
    }
}

#[tokio::test]
async fn executor_terminal_replay_is_exact_and_divergence_is_rejected() {
    let fixture = fixture().await;
    let first = fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "same summary", "same evidence", Vec::new()),
            ACTOR,
            None,
            "correlation:terminal:executor",
        )
        .await
        .expect("submit executor result");
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_submissions WHERE executor_run_id = ?1",
            &fixture.executor_fence.run_id,
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM agent_runs WHERE parent_run_id = ?1 AND run_kind = 'reviewer'",
            &fixture.executor_fence.run_id,
        )
        .await,
        1
    );

    let replay = fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "same summary", "same evidence", Vec::new()),
            ACTOR,
            None,
            "correlation:terminal:executor-replay",
        )
        .await
        .expect("late exact replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_submissions WHERE executor_run_id = ?1",
            &fixture.executor_fence.run_id,
        )
        .await,
        1
    );

    for divergent in [
        executor_submission(&fixture, "different summary", "same evidence", Vec::new()),
        executor_submission(&fixture, "same summary", "different evidence", Vec::new()),
        executor_submission(
            &fixture,
            "same summary",
            "same evidence",
            vec!["artifact:divergent".to_string()],
        ),
    ] {
        let error = fixture
            .service
            .record_work_run_terminal(
                divergent,
                ACTOR,
                None,
                "correlation:terminal:executor-divergent",
            )
            .await
            .expect_err("divergent terminal replay must fail");
        assert!(matches!(
            error,
            StoreError::Work(WorkDomainError::IdempotencyConflict)
        ));
    }
}

#[tokio::test]
async fn executor_terminal_rejects_wrong_kind_generation_and_contract_fences() {
    let fixture = fixture().await;
    let wrong_kind = WorkRunTerminal::Review(SubmitTaskReview {
        fence: fixture.executor_fence.clone(),
        review: NewTaskReview {
            review_id: Some("review:wrong-kind".to_string()),
            task_id: fixture.task.task_id.clone(),
            contract_id: fixture.contract.contract_id.clone(),
            reviewer_run_id: fixture.executor_fence.run_id.clone(),
            reviewed_submission_id: "submission:missing".to_string(),
            review_attempt_index: 1,
            supersedes_review_id: None,
            overall_verdict: TaskReviewVerdict::Approve,
            human_gate_kind: None,
            overall_feedback: "wrong kind".to_string(),
            criteria: fixture
                .contract
                .criteria
                .iter()
                .map(|criterion| TaskReviewCriterion {
                    criterion_id: criterion.criterion_id.clone(),
                    outcome: CriterionOutcome::Pass,
                    evidence_markdown: Some("evidence".to_string()),
                    feedback: None,
                })
                .collect(),
        },
    });
    let kind_error = fixture
        .service
        .record_work_run_terminal(wrong_kind, ACTOR, None, "correlation:terminal:wrong-kind")
        .await
        .expect_err("review terminal on an executor must fail");
    assert!(matches!(
        kind_error,
        StoreError::Work(WorkDomainError::InvalidTransition)
    ));

    let mut wrong_generation = fixture.executor_fence.clone();
    wrong_generation.task_generation += 1;
    let generation_error = fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "summary", "evidence", Vec::new())
                .with_fence(wrong_generation),
            ACTOR,
            None,
            "correlation:terminal:wrong-generation",
        )
        .await
        .expect_err("wrong generation must fail");
    assert!(matches!(
        generation_error,
        StoreError::Work(WorkDomainError::StaleGeneration)
    ));

    let mut wrong_contract = fixture.executor_fence.clone();
    wrong_contract.contract_id = Some(
        noema_tasks::TaskContractId::new("contract:wrong").expect("valid wrong contract namespace"),
    );
    let contract_error = fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "summary", "evidence", Vec::new())
                .with_fence(wrong_contract),
            ACTOR,
            None,
            "correlation:terminal:wrong-contract",
        )
        .await
        .expect_err("wrong contract must fail");
    assert!(matches!(
        contract_error,
        StoreError::Work(WorkDomainError::RunFenced)
    ));
}

#[tokio::test]
async fn reviewer_terminal_replay_is_exact_and_verdict_divergence_is_rejected() {
    let fixture = fixture().await;
    fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "summary", "evidence", Vec::new()),
            ACTOR,
            None,
            "correlation:terminal:executor",
        )
        .await
        .expect("submit executor result");
    let reviewer_claim = fixture
        .service
        .claim_next_work_run("worker:terminal:reviewer", 60, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let reviewer_fence = WorkRunFence {
        run_id: reviewer_claim.run.run_id.clone(),
        lease_token: reviewer_claim.lease_token.clone(),
        task_generation: fixture.task.generation,
        contract_id: Some(fixture.contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:terminal:reviewer",
        )
        .await
        .expect("start reviewer");
    let review_terminal = |verdict: TaskReviewVerdict, feedback: &str| {
        WorkRunTerminal::Review(SubmitTaskReview {
            fence: reviewer_fence.clone(),
            review: NewTaskReview {
                review_id: Some("review:terminal-test".to_string()),
                task_id: fixture.task.task_id.clone(),
                contract_id: fixture.contract.contract_id.clone(),
                reviewer_run_id: reviewer_fence.run_id.clone(),
                reviewed_submission_id: "submission:terminal-test".to_string(),
                review_attempt_index: 1,
                supersedes_review_id: None,
                overall_verdict: verdict,
                human_gate_kind: None,
                overall_feedback: feedback.to_string(),
                criteria: fixture
                    .contract
                    .criteria
                    .iter()
                    .map(|criterion| TaskReviewCriterion {
                        criterion_id: criterion.criterion_id.clone(),
                        outcome: if verdict == TaskReviewVerdict::Approve {
                            CriterionOutcome::Pass
                        } else {
                            CriterionOutcome::Fail
                        },
                        evidence_markdown: Some("review evidence".to_string()),
                        feedback: Some("review feedback".to_string()),
                    })
                    .collect(),
            },
        })
    };
    let first = fixture
        .service
        .record_work_run_terminal(
            review_terminal(TaskReviewVerdict::Approve, "approved"),
            ACTOR,
            None,
            "correlation:terminal:reviewer",
        )
        .await
        .expect("submit review");
    let completed = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load reviewed task")
        .expect("reviewed task exists");
    assert_eq!(
        completed.task.latest_run_id.as_deref(),
        Some(reviewer_fence.run_id.as_str())
    );
    assert!(completed.current_run.is_none());
    let replay = fixture
        .service
        .record_work_run_terminal(
            review_terminal(TaskReviewVerdict::Approve, "approved"),
            ACTOR,
            None,
            "correlation:terminal:reviewer-replay",
        )
        .await
        .expect("late exact review replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_reviews WHERE reviewer_run_id = ?1",
            &reviewer_fence.run_id,
        )
        .await,
        1
    );
    assert_eq!(
        count_without_id(
            &fixture.store,
            "SELECT COUNT(*) FROM work_notification_outbox WHERE notification_kind = 'task_review_ready'",
        )
        .await,
        1
    );

    let divergent = fixture
        .service
        .record_work_run_terminal(
            review_terminal(TaskReviewVerdict::RequestChanges, "changed verdict"),
            ACTOR,
            None,
            "correlation:terminal:reviewer-divergent",
        )
        .await
        .expect_err("divergent review verdict must fail");
    assert!(matches!(
        divergent,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

trait WithFence {
    fn with_fence(self, fence: WorkRunFence) -> WorkRunTerminal;
}

impl WithFence for WorkRunTerminal {
    fn with_fence(self, fence: WorkRunFence) -> WorkRunTerminal {
        match self {
            WorkRunTerminal::TaskResult(mut result) => {
                result.fence = fence;
                WorkRunTerminal::TaskResult(result)
            }
            other => other,
        }
    }
}
