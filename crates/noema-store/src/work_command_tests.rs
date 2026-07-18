//! Focused transactional tests for the semantic Work command writer.

use noema_tasks::{
    CaptureTask, CommandMeta, CreateProject, DelegateTask, QueueTask, TaskComplexity,
    TaskPrecondition, TaskProvenance, TaskSourceKind, UpdateInboxTask, WorkCommand,
    WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    NoemaStore, StoreError, WorkCommandService,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:human:local";

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

    let conflict = service
        .execute(capture("idem:capture", "different title"))
        .await
        .expect_err("same receipt key with a changed request must fail");
    assert!(matches!(
        conflict,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));

    // The value returned by the first command is an immutable snapshot owned
    // by the caller; later writes cannot mutate it through shared store state.
    let update_result = service
        .execute(update("idem:update", &returned_snapshot, "edited title"))
        .await
        .expect("inbox update");
    assert_eq!(returned_snapshot.title, "first title");
    assert_eq!(
        update_result.task.expect("updated task").title,
        "edited title"
    );
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
    assert!(!first_detail.workflow_stages.is_empty());
    assert!(first_detail.accepted_submission.is_none());

    let updated = service
        .execute(update(
            "idem:committed-detail:update",
            &first_detail.task,
            "later title",
        ))
        .await
        .expect("mutate task after original receipt");
    assert_eq!(updated.task.expect("updated task").title, "later title");

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
    let captured = service
        .execute(capture("idem:stale:capture", "captured"))
        .await
        .expect("capture");
    let original = captured.task.expect("captured task");

    let changed = service
        .execute(update("idem:stale:update", &original, "edited"))
        .await
        .expect("first update");
    let current = changed.task.clone().expect("updated task");
    let before_events = event_count(&store).await;

    let stale = service
        .execute(update("idem:stale:second", &original, "must not write"))
        .await
        .expect_err("old revision must be fenced");
    assert!(matches!(
        stale,
        StoreError::Work(WorkDomainError::StaleRevision)
    ));
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

    let edit_after_queue = service
        .execute(update("idem:after-queue", &queued_task, "must fail"))
        .await
        .expect_err("capture fields are Inbox-only");
    assert!(matches!(
        edit_after_queue,
        StoreError::Work(WorkDomainError::InvalidTransition)
    ));
}

#[tokio::test]
async fn stale_generation_is_rejected_even_when_revision_matches() {
    let (_store, service) = fixture().await;
    let captured = service
        .execute(capture("idem:generation:capture", "captured"))
        .await
        .expect("capture")
        .task
        .expect("task");

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

    let stale = service
        .execute(update("idem:generation:stale", &captured, "must fail"))
        .await
        .expect_err("old generation must be fenced");
    assert!(matches!(
        stale,
        StoreError::Work(WorkDomainError::StaleGeneration)
    ));
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
    let captured = service
        .execute(WorkCommand::CaptureTask(CaptureTask {
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
        }))
        .await
        .expect("capture associated task")
        .task
        .expect("task");

    let preserved = service
        .execute(WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:preserve"),
            precondition: precondition(&captured),
            title: Some("Still associated".to_string()),
            description_markdown: None,
            project_id: None,
        }))
        .await
        .expect("omitted project replacement")
        .task
        .expect("updated task");
    assert_eq!(preserved.project_id.as_ref(), Some(&project.project_id));

    let cleared = service
        .execute(WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:clear"),
            precondition: precondition(&preserved),
            title: None,
            description_markdown: None,
            project_id: Some(None),
        }))
        .await
        .expect("explicit project clear")
        .task
        .expect("cleared task");
    assert_eq!(cleared.project_id, None);
}

#[tokio::test]
async fn delegate_source_replay_is_exact_across_idempotency_namespaces() {
    let (store, service) = fixture().await;
    let original = delegated("idem:delegate:first", "same-source", None);
    let first = service.execute(original).await.expect("delegate task");
    let replay = service
        .execute(delegated("idem:delegate:second", "same-source", None))
        .await
        .expect("source replay under another idempotency key");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, first.task);
    assert_eq!(replay.run_id, first.run_id);

    let mut divergent = delegated("idem:delegate:third", "same-source", None);
    let WorkCommand::DelegateTask(command) = &mut divergent else {
        unreachable!()
    };
    command.title = "Changed durable payload".to_string();
    let error = service
        .execute(divergent)
        .await
        .expect_err("source replay with divergent payload must fail");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
    assert_eq!(
        count_without_id(
            &store,
            "SELECT COUNT(*) FROM tasks WHERE source_conversation_id = 'conversation:delegate-source' AND source_tool_call_id = 'tool_call:same-source'",
        )
        .await,
        1
    );
}

#[tokio::test]
async fn capture_source_replay_is_exact_across_idempotency_namespaces() {
    let (store, service) = fixture().await;
    let first = service
        .execute(sourced_capture("idem:capture-source:first", "same-source"))
        .await
        .expect("capture task");
    let replay = service
        .execute(sourced_capture("idem:capture-source:second", "same-source"))
        .await
        .expect("source replay under another idempotency key");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, first.task);

    let mut divergent = sourced_capture("idem:capture-source:third", "same-source");
    let WorkCommand::CaptureTask(command) = &mut divergent else {
        unreachable!()
    };
    command.description_markdown = "divergent".to_string();
    assert!(matches!(
        service.execute(divergent).await,
        Err(StoreError::Work(WorkDomainError::IdempotencyConflict))
    ));
    assert_eq!(
        count_without_id(
            &store,
            "SELECT COUNT(*) FROM tasks WHERE source_conversation_id = 'conversation:capture-source' AND source_tool_call_id = 'tool_call:same-source'",
        )
        .await,
        1
    );
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
    assert_eq!(
        count_without_id(
            &store,
            "SELECT COUNT(*) FROM work_command_receipts WHERE command_name = 'task.capture' AND idempotency_key LIKE 'source-replay:%'",
        )
        .await,
        1
    );

    let WorkCommand::CaptureTask(command) = &mut original else {
        unreachable!()
    };
    command.title = "Divergent source payload".to_string();
    assert!(matches!(
        service.execute(original).await,
        Err(StoreError::Work(WorkDomainError::IdempotencyConflict))
    ));
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
