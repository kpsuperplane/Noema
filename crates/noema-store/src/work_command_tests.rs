//! Focused transactional tests for the semantic Work command writer.

use noema_tasks::{
    CaptureTask, CommandMeta, CreateProject, DelegateTask, QueueTask, RetryTask, SafeErrorCode,
    TaskComplexity, TaskPrecondition, TaskProvenance, TaskSourceKind, UpdateInboxTask, WorkCommand,
    WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    GovernedActionDecision, GovernedActionEffect, GovernedActionState, GovernedAssessmentStatus,
    GovernedRecommendation, NewGovernedAction, NewGovernedActionAssessment, NoemaStore,
    ReportRunFailure, StoreError, WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN, WorkCommandService,
    WorkRunFence,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:human:local";

macro_rules! task {
    ($service:expr, $command:expr, $context:literal) => {
        $service
            .execute($command)
            .await
            .expect($context)
            .task
            .expect("task")
    };
}

macro_rules! work_error {
    ($service:expr, $command:expr, $error:pat, $context:literal) => {
        assert!(
            matches!($service.execute($command).await, Err($error)),
            $context
        )
    };
}

async fn fixture() -> (NoemaStore, WorkCommandService) {
    let store = open_ephemeral_store().await.expect("open store");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);
    (store, service)
}

fn metadata(key: &str) -> CommandMeta {
    CommandMeta {
        actor_id: ACTOR.to_string(),
        causation_id: None,
        correlation_id: format!("correlation:{key}"),
        idempotency_key: Some(key.to_string()),
    }
}

fn capture(key: &str, title: &str) -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: metadata(key),
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: title.to_string(),
        description_markdown: "captured description".to_string(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: ACTOR.to_string(),
            ..TaskProvenance::default()
        },
    })
}

fn sourced_capture(key: &str, source: &str) -> WorkCommand {
    let WorkCommand::CaptureTask(mut command) = capture(key, "Sourced capture") else {
        unreachable!()
    };
    command.provenance.conversation_id = Some("conversation:capture-source".to_string());
    command.provenance.source_tool_call_id = Some(format!("tool_call:{source}"));
    WorkCommand::CaptureTask(command)
}

fn precondition(task: &noema_tasks::TaskRecord) -> TaskPrecondition {
    TaskPrecondition {
        task_id: task.task_id.clone(),
        expected_revision: task.revision,
        expected_generation: task.generation,
    }
}

fn update(key: &str, task: &noema_tasks::TaskRecord, title: &str) -> WorkCommand {
    WorkCommand::UpdateInboxTask(UpdateInboxTask {
        meta: metadata(key),
        precondition: precondition(task),
        title: Some(title.to_string()),
        description_markdown: None,
        project_id: None,
    })
}

fn queue(key: &str, task: &noema_tasks::TaskRecord) -> WorkCommand {
    WorkCommand::QueueTask(QueueTask {
        meta: metadata(key),
        precondition: precondition(task),
    })
}

fn delegated(key: &str, source: &str, complexity_hint: Option<TaskComplexity>) -> WorkCommand {
    WorkCommand::DelegateTask(DelegateTask {
        meta: CommandMeta {
            actor_id: ACTOR.to_string(),
            causation_id: None,
            correlation_id: format!("correlation:delegate:{source}"),
            idempotency_key: Some(key.to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: format!("Delegated {source}"),
        description_markdown: "Durable delegated payload".to_string(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatDelegate,
            conversation_id: Some("conversation:delegate-source".to_string()),
            source_tool_call_id: Some(format!("tool_call:{source}")),
            created_by_actor_id: ACTOR.to_string(),
            ..TaskProvenance::default()
        },
        complexity_hint,
        execution_intent: None,
    })
}

async fn event_count(store: &NoemaStore) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM work_events", [], |row| row.get(0))
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("event count")
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

async fn assert_one_durable_row(store: &NoemaStore, sql: &str) {
    assert_eq!(count_without_id(store, sql).await, 1);
}

#[derive(Clone, Copy)]
enum SourceReplayKind {
    Capture,
    Delegate,
}

async fn assert_source_replay(kind: SourceReplayKind) {
    let (store, service) = fixture().await;
    let command = |key| match kind {
        SourceReplayKind::Capture => sourced_capture(key, "same-source"),
        SourceReplayKind::Delegate => delegated(key, "same-source", None),
    };
    let first = service
        .execute(command("idem:source:first"))
        .await
        .expect("create task");
    let replay = service
        .execute(command("idem:source:second"))
        .await
        .expect("source replay under another idempotency key");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, first.task);
    if matches!(kind, SourceReplayKind::Delegate) {
        assert_eq!(replay.run_id, first.run_id);
    }

    let mut divergent = command("idem:source:third");
    match &mut divergent {
        WorkCommand::CaptureTask(command) => command.description_markdown = "divergent".to_string(),
        WorkCommand::DelegateTask(command) => command.title = "Changed durable payload".to_string(),
        _ => unreachable!(),
    }
    work_error!(
        service,
        divergent,
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "source replay with divergent payload must fail"
    );
    let source_kind = match kind {
        SourceReplayKind::Capture => "capture-source",
        SourceReplayKind::Delegate => "delegate-source",
    };
    assert_one_durable_row(&store, &format!("SELECT COUNT(*) FROM tasks WHERE source_conversation_id = 'conversation:{source_kind}' AND source_tool_call_id = 'tool_call:same-source'")).await;
}

#[tokio::test]
async fn receipt_replay_is_exact_and_divergent_replay_is_rejected() {
    let (_store, service) = fixture().await;
    let command = capture("idem:capture", "first title");

    let first = service.execute(command.clone()).await.expect("capture");
    let returned_snapshot = first.task.clone().expect("captured task");
    let replay = service.execute(command).await.expect("idempotent replay");

    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, Some(returned_snapshot.clone()));

    work_error!(
        service,
        capture("idem:capture", "different title"),
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "same receipt key with a changed request must fail"
    );

    // The value returned by the first command is an immutable snapshot owned
    // by the caller; later writes cannot mutate it through shared store state.
    let updated = task!(
        service,
        update("idem:update", &returned_snapshot, "edited title"),
        "inbox update"
    );
    assert_eq!(returned_snapshot.title, "first title");
    assert_eq!(updated.title, "edited title");
}

#[tokio::test]
async fn committed_detail_replay_does_not_reread_later_task_state() {
    let (_store, service) = fixture().await;
    let command = capture("idem:committed-detail", "receipt title");
    let first = service
        .execute_committed(command.clone())
        .await
        .expect("capture committed detail");
    let first_detail = first.task_detail.clone().expect("task detail snapshot");
    assert_eq!(first_detail.stage.stage_id, first_detail.task.stage_id);
    assert!(first_detail.accepted_submission.is_none());

    let updated = task!(
        service,
        update(
            "idem:committed-detail:update",
            &first_detail.task,
            "later title",
        ),
        "mutate task after original receipt"
    );
    assert_eq!(updated.title, "later title");

    let replay = service
        .execute_committed(command)
        .await
        .expect("replay original receipt");
    assert_eq!(replay.result.event_id, first.result.event_id);
    assert_eq!(replay.result.event_sequence, first.result.event_sequence);
    assert_eq!(replay.task_detail, Some(first_detail));
}

#[tokio::test]
async fn stale_revision_is_atomic_and_inbox_edits_stop_at_queue() {
    let (store, service) = fixture().await;
    let original = task!(
        service,
        capture("idem:stale:capture", "captured"),
        "capture"
    );

    let current = task!(
        service,
        update("idem:stale:update", &original, "edited"),
        "first update"
    );
    let before_events = event_count(&store).await;

    work_error!(
        service,
        update("idem:stale:second", &original, "must not write"),
        StoreError::Work(WorkDomainError::StaleRevision),
        "old revision must be fenced"
    );
    assert_eq!(event_count(&store).await, before_events);
    assert_eq!(
        service
            .store()
            .get_work_task(&current.task_id)
            .await
            .expect("read task")
            .expect("task exists")
            .task
            .title,
        "edited"
    );

    let queued = service
        .execute(queue("idem:queue", &current))
        .await
        .expect("queue task");
    let queued_task = queued.task.expect("queued task");
    assert_eq!(queued_task.stage_id.as_str(), "stage:personal:queue");
    assert!(queued.run_id.is_some(), "queue must create one planner run");

    work_error!(
        service,
        update("idem:after-queue", &queued_task, "must fail"),
        StoreError::Work(WorkDomainError::InvalidTransition),
        "capture fields are Inbox-only"
    );
}

#[tokio::test]
async fn stale_generation_is_rejected_even_when_revision_matches() {
    let (_store, service) = fixture().await;
    let captured = task!(
        service,
        capture("idem:generation:capture", "captured"),
        "capture"
    );

    let task_id = captured.task_id.clone();
    service
        .store()
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?1",
                    [task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("advance generation in test fixture");

    work_error!(
        service,
        update("idem:generation:stale", &captured, "must fail"),
        StoreError::Work(WorkDomainError::StaleGeneration),
        "old generation must be fenced"
    );
}

#[tokio::test]
async fn inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear() {
    let (_store, service) = fixture().await;
    let project = service
        .execute(WorkCommand::CreateProject(CreateProject {
            meta: metadata("idem:project:create"),
            workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
            name: "Project association".to_string(),
            description: String::new(),
        }))
        .await
        .expect("create project")
        .project
        .expect("project");
    let captured = task!(
        service,
        WorkCommand::CaptureTask(CaptureTask {
            meta: metadata("idem:project:capture"),
            workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
            title: "Associated task".to_string(),
            description_markdown: String::new(),
            project_id: Some(project.project_id.clone()),
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::WorkUi,
                created_by_actor_id: ACTOR.to_string(),
                ..TaskProvenance::default()
            },
        }),
        "capture associated task"
    );

    let preserved = task!(
        service,
        WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:preserve"),
            precondition: precondition(&captured),
            title: Some("Still associated".to_string()),
            description_markdown: None,
            project_id: None,
        }),
        "omitted project replacement"
    );
    assert_eq!(preserved.project_id.as_ref(), Some(&project.project_id));

    let cleared = task!(
        service,
        WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:clear"),
            precondition: precondition(&preserved),
            title: None,
            description_markdown: None,
            project_id: Some(None),
        }),
        "explicit project clear"
    );
    assert_eq!(cleared.project_id, None);
}

#[tokio::test]
async fn delegate_source_replay_is_exact_across_idempotency_namespaces() {
    assert_source_replay(SourceReplayKind::Delegate).await;
}

#[tokio::test]
async fn capture_source_replay_is_exact_across_idempotency_namespaces() {
    assert_source_replay(SourceReplayKind::Capture).await;
}

#[tokio::test]
async fn capture_source_replay_is_durable_without_an_idempotency_key() {
    let (store, service) = fixture().await;
    let mut original = sourced_capture("unused:first", "no-key-source");
    let WorkCommand::CaptureTask(command) = &mut original else {
        unreachable!()
    };
    command.meta.idempotency_key = None;
    let first = service
        .execute_committed(original.clone())
        .await
        .expect("source-owned capture without an idempotency key");

    let replay = service
        .execute_committed(original.clone())
        .await
        .expect("exact source replay without an idempotency key");
    assert_eq!(replay, first);
    assert_one_durable_row(&store, "SELECT COUNT(*) FROM work_command_receipts WHERE command_name = 'task.capture' AND idempotency_key LIKE 'source-replay:%'").await;

    let WorkCommand::CaptureTask(command) = &mut original else {
        unreachable!()
    };
    command.title = "Divergent source payload".to_string();
    work_error!(
        service,
        original,
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "source replay with a changed payload must fail"
    );
}

#[tokio::test]
async fn delegate_planner_complexity_hint_selects_the_matching_pool_tier() {
    let (store, service) = fixture().await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                "UPDATE task_model_pool_entries SET reasoning_effort = CASE complexity WHEN 'simple' THEN 'low' WHEN 'medium' THEN 'medium' WHEN 'difficult' THEN 'high' END;",
            ).map_err(StoreError::Sqlite)?;
            Ok(())
        })
        .await
        .expect("differentiate task pool tiers");

    for (source, hint, expected_effort) in [
        ("simple", Some(TaskComplexity::Simple), "low"),
        ("difficult", Some(TaskComplexity::Difficult), "high"),
        ("fallback", None, "medium"),
    ] {
        let result = service
            .execute(delegated(&format!("idem:delegate:{source}"), source, hint))
            .await
            .expect("delegate planner tier");
        let run_id = result.run_id.expect("planner run id");
        let (kind, effort, source_kind): (String, Option<String>, Option<String>) = store
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT run_kind, reasoning_effort, selection_source FROM agent_runs WHERE run_id = ?1",
                        [run_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(StoreError::Sqlite)
            })
            .await
            .expect("load planner snapshot");
        assert_eq!(kind, "planner");
        assert_eq!(effort.as_deref(), Some(expected_effort));
        assert_eq!(source_kind.as_deref(), Some("task_model_pool_setting"));
    }
}

#[tokio::test]
async fn governed_action_approval_releases_and_resumes_a_task_run_once() {
    let (store, service) = fixture().await;
    let captured = task!(
        service,
        capture("idem:governed:capture", "Governed task"),
        "capture task"
    );
    task!(
        service,
        queue("idem:governed:queue", &captured),
        "queue task"
    );
    let claimed = service
        .claim_next_work_run("worker:governed", 60, &[])
        .await
        .expect("claim run")
        .expect("queued run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id.clone(),
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:governed")
        .await
        .expect("start run");
    let failed_run_id = fence.run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE agent_runs SET max_automatic_retries = 0 WHERE run_id = ?1",
                [failed_run_id],
            )?;
            Ok(())
        })
        .await
        .expect("exhaust automatic retries");
    let failed = service
        .report_work_run_failure(
            ReportRunFailure {
                fence,
                status: noema_tasks::RunStatus::Failed,
                error_code: SafeErrorCode::new("work_runtime_failed").expect("safe error code"),
                error_message: Some("retryable test failure".to_string()),
                retryable: true,
            },
            ACTOR,
            None,
            "correlation:governed:failure",
        )
        .await
        .expect("open recovery gate");
    let failed_task = failed.task.expect("failed task");
    let gate_id = failed.gate_id.expect("recovery gate");
    service
        .execute(WorkCommand::RetryTask(RetryTask {
            meta: metadata("idem:governed:retry"),
            precondition: precondition(&failed_task),
            gate_id,
            note: None,
        }))
        .await
        .expect("retry failed run");
    let claimed = service
        .claim_next_work_run("worker:governed:retry", 60, &[])
        .await
        .expect("claim retry")
        .expect("retry run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:governed:retry")
        .await
        .expect("start retry");
    service
        .admit_work_run_execution_context(&fence, ACTOR, None, "correlation:governed")
        .await
        .expect("admit parent checkpoint");
    let parent_item_limit =
        i64::try_from(WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN).expect("lineage limit fits i64");
    let parent_run_id = fence.run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "WITH RECURSIVE item(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM item WHERE n < ?2)
                 INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, content_text)
                 SELECT 'run_item:governed:' || n, ?1, n + 1, 0, 'progress_notice', 'completed', 'parent item ' || n FROM item",
                rusqlite::params![parent_run_id, parent_item_limit],
            )?;
            Ok(())
        })
        .await
        .expect("append oversized parent transcript");
    let action = store
        .create_governed_action(NewGovernedAction {
            owner_human_id: "human:local".to_string(),
            conversation_id: None,
            turn_id: None,
            task_id: Some(captured.task_id.to_string()),
            run_id: Some(fence.run_id.clone()),
            requesting_agent_id: "agent:task-executor".to_string(),
            capability_name: "mcp.example.write".to_string(),
            operation_token: "exact-token".to_string(),
            effect: GovernedActionEffect::Write,
            arguments: serde_json::json!({"record_id": "42"}),
            input_schema: serde_json::json!({"type": "object"}),
            trusted_authority: serde_json::json!({"origin": "task"}),
            safe_summary: "write an external record".to_string(),
        })
        .await
        .expect("create action");
    let waiting = store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                recommendation: GovernedRecommendation::RequireApproval,
                reason_codes: vec!["authorization_ambiguous".to_string()],
                explanation: "human approval required".to_string(),
            },
            Some(&fence),
        )
        .await
        .expect("request approval");
    assert_eq!(waiting.state, GovernedActionState::AwaitingApproval);
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load waiting run")
            .expect("run")
            .status,
        noema_tasks::RunStatus::WaitingForApproval
    );
    let snapshot = store
        .load_work_reconciliation_snapshot(&captured.task_id)
        .await
        .expect("load waiting reconciliation snapshot")
        .expect("task snapshot");
    assert_eq!(
        crate::plan_work_reconciliation(&snapshot).expect("plan waiting reconciliation"),
        noema_tasks::WorkReconciliationAction::Idle
    );

    let declined = store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            GovernedActionDecision::Decline,
        )
        .await
        .expect("decline action");
    assert_eq!(declined.state, GovernedActionState::Declined);
    let child = service
        .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
        .await
        .expect("resume task")
        .expect("child run");
    assert_eq!(
        service
            .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
            .await
            .expect("idempotent resume"),
        Some(child.clone())
    );
    let claimed_child = service
        .claim_next_work_run("worker:governed:child", 60, &[])
        .await
        .expect("claim child")
        .expect("child run");
    assert_eq!(claimed_child.run.run_id, child);
    let child_fence = WorkRunFence {
        run_id: claimed_child.run.run_id,
        lease_token: claimed_child.lease_token,
        task_generation: claimed_child.run.task_generation,
        contract_id: claimed_child.run.contract_id,
    };
    service
        .start_work_run(&child_fence, ACTOR, None, "correlation:governed:child")
        .await
        .expect("start child");
    let admitted_child = service
        .admit_work_run_execution_context(&child_fence, ACTOR, None, "correlation:governed:child")
        .await
        .expect("admit child from bounded parent transcript");
    assert_eq!(
        admitted_child.context.lineage.len(),
        WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN
    );
    assert_eq!(
        admitted_child.context.lineage[0].content_text.as_deref(),
        Some("parent item 2")
    );
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load parent")
            .expect("parent run")
            .status,
        noema_tasks::RunStatus::Completed
    );
}
