//! Focused read-consistency tests for leased Work-run execution contexts.

mod admission;

use noema_tasks::{
    AgentRunItemKind, CommandMeta, CreateProject, DelegateTask, NewTaskValidationCriterion,
    RunKind, RunStatus, SubmissionCriterionEvidence, TaskComplexity, TaskProvenance,
    TaskSourceKind, WorkCommand,
};
use noema_workspaces::{ProjectId, WorkspaceId};

use crate::{
    CompletePlan, NoemaStore, PlanTerminal, StoreError, SubmitPlan, SubmitTaskResult,
    WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN, WorkCommandService, WorkRunFence, WorkRunTerminal,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:agent:context-test";
const WORKSPACE: &str = "workspace:personal";

struct ContextFixture {
    store: NoemaStore,
    service: WorkCommandService,
    task: noema_tasks::TaskRecord,
    contract: noema_tasks::TaskExecutionContract,
    project_id: ProjectId,
    planner_run_id: String,
    executor_fence: WorkRunFence,
}

async fn fixture() -> ContextFixture {
    let store = open_ephemeral_store().await.expect("open store");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);

    let project = service
        .execute(WorkCommand::CreateProject(CreateProject {
            meta: metadata("context:project"),
            workspace_id: WorkspaceId::new(WORKSPACE).expect("workspace id"),
            name: "Context project".to_string(),
            description: "Immutable project description".to_string(),
        }))
        .await
        .expect("create project")
        .project
        .expect("created project");
    let project_id = project.project_id.clone();

    let delegated = service
        .execute(WorkCommand::DelegateTask(DelegateTask {
            meta: metadata("context:delegate"),
            workspace_id: WorkspaceId::new(WORKSPACE).expect("workspace id"),
            title: "Execution context task".to_string(),
            description_markdown: "A bounded context test task".to_string(),
            project_id: Some(project_id.clone()),
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::ChatDelegate,
                created_by_actor_id: "actor:human:local".to_string(),
                ..TaskProvenance::default()
            },
            complexity_hint: None,
            execution_intent: None,
        }))
        .await
        .expect("delegate task");
    let queued_task = delegated.task.expect("delegated task");
    let planner_claim = service
        .claim_next_work_run("worker:context:planner", 60, &[])
        .await
        .expect("claim planner")
        .expect("planner run");
    assert_eq!(planner_claim.run.run_kind, RunKind::Planner);
    let planner_fence = WorkRunFence {
        run_id: planner_claim.run.run_id.clone(),
        lease_token: planner_claim.lease_token.clone(),
        task_generation: queued_task.generation,
        contract_id: None,
    };
    service
        .start_work_run(&planner_fence, ACTOR, None, "correlation:context:planner")
        .await
        .expect("start planner");
    service
        .admit_work_run_execution_context(
            &planner_fence,
            ACTOR,
            None,
            "correlation:context:planner-admission",
        )
        .await
        .expect("admit planner context");
    service
        .record_work_run_terminal(
            WorkRunTerminal::Plan(SubmitPlan {
                fence: planner_fence,
                terminal: PlanTerminal::Complete(CompletePlan {
                    request_markdown: "Execute the context test request.".to_string(),
                    execution_plan_markdown: "Use the immutable context packet.".to_string(),
                    criteria: vec![NewTaskValidationCriterion {
                        criterion_id: None,
                        ordinal: 1,
                        description: "The context is exact and bounded.".to_string(),
                        expected_evidence: Some("A context assertion".to_string()),
                    }],
                    complexity: TaskComplexity::Simple,
                }),
            }),
            ACTOR,
            None,
            "correlation:context:planner",
        )
        .await
        .expect("complete planner");

    let detail = store
        .get_work_task(&queued_task.task_id)
        .await
        .expect("load planned task")
        .expect("planned task");
    let task = detail.task;
    let contract = detail.current_contract.expect("planned contract");
    let executor_claim = service
        .claim_next_work_run("worker:context:executor", 60, &[])
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
        .start_work_run(&executor_fence, ACTOR, None, "correlation:context:executor")
        .await
        .expect("start executor");

    ContextFixture {
        store,
        service,
        task,
        contract,
        project_id,
        planner_run_id: executor_claim
            .run
            .parent_run_id
            .expect("executor keeps planner lineage"),
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

fn executor_submission(fixture: &ContextFixture) -> WorkRunTerminal {
    WorkRunTerminal::TaskResult(SubmitTaskResult {
        fence: fixture.executor_fence.clone(),
        submission: noema_tasks::NewTaskSubmission {
            submission_id: Some("submission:context-test".to_string()),
            task_id: fixture.task.task_id.clone(),
            contract_id: fixture.contract.contract_id.clone(),
            executor_run_id: fixture.executor_fence.run_id.clone(),
            review_round: 1,
            summary: "Context submission summary".to_string(),
            result_markdown: "Context submission result".to_string(),
            criteria: fixture
                .contract
                .criteria
                .iter()
                .map(|criterion| SubmissionCriterionEvidence {
                    criterion_id: criterion.criterion_id.clone(),
                    evidence_markdown: "Context evidence".to_string(),
                })
                .collect(),
            artifact_ids: Vec::new(),
        },
    })
}

async fn insert_run_item(store: &NoemaStore, run_id: &str, sequence_index: i64) {
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO agent_run_items (item_id, run_id, sequence_index, kind, content_text, payload_json) VALUES (?1, ?2, ?3, 'progress_notice', ?4, '{}')",
                    rusqlite::params![
                        format!("run_item:context:{run_id}:{sequence_index}"),
                        run_id,
                        sequence_index,
                        format!("lineage item {sequence_index}"),
                    ],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("insert lineage item");
}

fn assert_invariant(error: StoreError) {
    assert!(
        matches!(error, StoreError::InvariantViolation { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn leased_and_running_context_is_exact_and_lineage_is_bounded() {
    let fixture = fixture().await;
    insert_run_item(&fixture.store, &fixture.planner_run_id, 1).await;
    insert_run_item(&fixture.store, &fixture.planner_run_id, 2).await;
    insert_run_item(&fixture.store, &fixture.executor_fence.run_id, 1).await;

    let running = fixture
        .store
        .get_work_run_execution_context(&fixture.executor_fence.run_id)
        .await
        .expect("running context")
        .expect("running context exists");
    assert_eq!(running.run.status, RunStatus::Running);
    assert_eq!(running.run.run_id, fixture.executor_fence.run_id);
    assert_eq!(running.run.task_generation, fixture.task.generation);
    assert_eq!(running.task, fixture.task);
    assert_eq!(running.contract, Some(fixture.contract.clone()));
    assert_eq!(running.run.model, fixture.contract.executor_model);
    assert_eq!(
        running.run.execution_policy,
        fixture.contract.execution_policy
    );
    assert_eq!(running.workspace, fixture.contract.workspace_context);
    assert_eq!(running.project, fixture.contract.project_context);
    assert_eq!(
        running
            .lineage
            .iter()
            .map(|item| item.run_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            fixture.planner_run_id.as_str(),
            fixture.planner_run_id.as_str(),
            fixture.executor_fence.run_id.as_str(),
        ]
    );
    assert!(running.lineage.len() <= 2 * WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN);

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET status = 'leased' WHERE run_id = ?1",
                    [fixture.executor_fence.run_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("set leased status");
    let leased = fixture
        .store
        .get_work_run_execution_context(&fixture.executor_fence.run_id)
        .await
        .expect("leased context")
        .expect("leased context exists");
    assert_eq!(leased.run.status, RunStatus::Leased);

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE workspaces SET name = 'Changed workspace', description = 'Changed live workspace' WHERE workspace_id = ?1",
                    [WORKSPACE],
                )
                .map_err(StoreError::Sqlite)?;
            connection
                .execute(
                    "UPDATE projects SET name = 'Changed project', description = 'Changed live project' WHERE project_id = ?1",
                    [fixture.project_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("mutate live descriptive rows");
    let after_live_edit = fixture
        .store
        .get_work_run_execution_context(&fixture.executor_fence.run_id)
        .await
        .expect("context after live edit")
        .expect("context after live edit exists");
    assert_eq!(
        after_live_edit.workspace,
        fixture.contract.workspace_context
    );
    assert_eq!(after_live_edit.project, fixture.contract.project_context);
}

#[tokio::test]
async fn phase_transition_children_skip_incompatible_parent_checkpoints() {
    let fixture = fixture().await;
    let executor = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:executor-phase-admission",
        )
        .await
        .expect("executor skips Planner checkpoint");
    assert_eq!(executor.context.run.run_kind, RunKind::Executor);
    assert_eq!(executor.context.contract, Some(fixture.contract.clone()));

    fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture),
            ACTOR,
            None,
            "correlation:context:executor-phase-terminal",
        )
        .await
        .expect("submit executor result");
    let reviewer_claim = fixture
        .service
        .claim_next_work_run("worker:context:reviewer-phase", 60, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let reviewer_fence = WorkRunFence {
        run_id: reviewer_claim.run.run_id,
        lease_token: reviewer_claim.lease_token,
        task_generation: fixture.task.generation,
        contract_id: Some(fixture.contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:context:reviewer-phase",
        )
        .await
        .expect("start reviewer");
    let reviewer = fixture
        .service
        .admit_work_run_execution_context(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:context:reviewer-phase-admission",
        )
        .await
        .expect("reviewer skips Executor checkpoint");
    assert_eq!(reviewer.context.run.run_kind, RunKind::Reviewer);
    assert_eq!(
        reviewer
            .context
            .latest_submission
            .as_ref()
            .map(|submission| submission.submission_id.as_str()),
        Some("submission:context-test")
    );
}

#[tokio::test]
async fn context_scopes_consumed_and_causal_messages_to_current_evidence() {
    let fixture = fixture().await;
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES ('gate:context-causal', ?1, ?2, ?3, 'clarification', 'open', 'Causal question', 'Causal context', ?4, ?5)",
                    rusqlite::params![
                        fixture.task.task_id.as_str(),
                        fixture.task.generation,
                        fixture.contract.contract_id.as_str(),
                        ACTOR,
                        fixture.executor_fence.run_id.as_str(),
                    ],
                )
                .map_err(StoreError::Sqlite)?;
            connection
                .execute(
                    "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:context-causal', ?1, ?2, ?3, 'gate:context-causal', 'human_answer', 'Causal answer', ?4)",
                    rusqlite::params![
                        fixture.task.task_id.as_str(),
                        fixture.task.generation,
                        fixture.contract.contract_id.as_str(),
                        ACTOR,
                    ],
                )
                .map_err(StoreError::Sqlite)?;
            connection
                .execute(
                    "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:context-unrelated', ?1, ?2, ?3, 'retry_note', 'Unrelated message', ?4)",
                    rusqlite::params![
                        fixture.task.task_id.as_str(),
                        fixture.task.generation,
                        fixture.contract.contract_id.as_str(),
                        ACTOR,
                    ],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("insert scoped message evidence");

    fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture),
            ACTOR,
            None,
            "correlation:context:submission",
        )
        .await
        .expect("submit context evidence");
    let reviewer_claim = fixture
        .service
        .claim_next_work_run("worker:context:reviewer", 60, &[])
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
        .start_work_run(&reviewer_fence, ACTOR, None, "correlation:context:reviewer")
        .await
        .expect("start reviewer");
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, message_kind, body_markdown, author_actor_id, consumed_by_run_id, consumed_at) VALUES ('task_message:context-consumed', ?1, ?2, ?3, 'retry_note', 'Consumed by this run', ?4, ?5, '2026-01-01T00:00:00.000Z')",
                    rusqlite::params![
                        fixture.task.task_id.as_str(),
                        fixture.task.generation,
                        fixture.contract.contract_id.as_str(),
                        ACTOR,
                        reviewer_fence.run_id.as_str(),
                    ],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("insert consumed message");
    insert_run_item(&fixture.store, &reviewer_fence.run_id, 1).await;

    let context = fixture
        .store
        .get_work_run_execution_context(&reviewer_fence.run_id)
        .await
        .expect("reviewer context")
        .expect("reviewer context exists");
    assert_eq!(
        context.run.triggering_submission_id.as_deref(),
        Some("submission:context-test")
    );
    assert_eq!(
        context
            .latest_submission
            .as_ref()
            .map(|submission| submission.submission_id.as_str()),
        Some("submission:context-test")
    );
    assert_eq!(context.latest_review, None);
    assert_eq!(context.messages.len(), 2);
    assert!(
        context
            .messages
            .iter()
            .any(|message| message.message_id.as_str() == "task_message:context-consumed")
    );
    assert!(
        context
            .messages
            .iter()
            .any(|message| message.message_id.as_str() == "task_message:context-causal")
    );
    assert!(
        !context
            .messages
            .iter()
            .any(|message| message.message_id.as_str() == "task_message:context-unrelated")
    );
    assert_eq!(context.relevant_gates.len(), 1);
    assert_eq!(
        context.relevant_gates[0].gate_id.as_str(),
        "gate:context-causal"
    );

    for sequence_index in 2..=(WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN as i64 + 1) {
        insert_run_item(&fixture.store, &reviewer_fence.run_id, sequence_index).await;
    }
    let bounded_error = fixture
        .store
        .get_work_run_execution_context(&reviewer_fence.run_id)
        .await
        .expect_err("lineage beyond the bound must fail closed");
    assert_invariant(bounded_error);
}

#[tokio::test]
async fn context_rejects_queued_completed_stale_and_mismatched_current_runs() {
    let fixture = fixture().await;
    let run_id = fixture.executor_fence.run_id.as_str();

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET status = 'queued' WHERE run_id = ?1",
                    [run_id],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("set queued status");
    assert_invariant(
        fixture
            .store
            .get_work_run_execution_context(run_id)
            .await
            .expect_err("queued run must be rejected"),
    );

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET status = 'completed' WHERE run_id = ?1",
                    [run_id],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("set completed status");
    assert_invariant(
        fixture
            .store
            .get_work_run_execution_context(run_id)
            .await
            .expect_err("completed run must be rejected"),
    );

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET status = 'running' WHERE run_id = ?1",
                    [run_id],
                )
                .map_err(StoreError::Sqlite)?;
            connection
                .execute(
                    "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?1",
                    [fixture.task.task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("advance task generation");
    assert_invariant(
        fixture
            .store
            .get_work_run_execution_context(run_id)
            .await
            .expect_err("stale generation must be rejected"),
    );

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET latest_run_id = NULL WHERE task_id = ?1",
                    [fixture.task.task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("clear current run pointer");
    assert_invariant(
        fixture
            .store
            .get_work_run_execution_context(run_id)
            .await
            .expect_err("current run mismatch must be rejected"),
    );

    let missing = fixture
        .store
        .get_work_run_execution_context("run:missing")
        .await
        .expect("missing run read");
    assert_eq!(missing, None);
}
