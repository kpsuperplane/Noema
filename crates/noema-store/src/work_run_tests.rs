//! Focused FIFO, lease, heartbeat, and generation-fence tests for Work runs.

mod claim_exclusions;

use noema_tasks::{
    AgentRunItemKind, AgentRunItemStatus, CancelTask, CaptureTask, CommandMeta, NewAgentRunItem,
    TaskPrecondition, TaskProvenance, TaskSourceKind, WorkCommand, WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    NoemaStore, ReportRunFailure, StoreError, WorkCommandService, WorkRunFence, WorkRunProgress,
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

fn capture(key: &str, title: &str) -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: CommandMeta {
            actor_id: ACTOR.to_string(),
            causation_id: None,
            correlation_id: format!("correlation:{key}"),
            idempotency_key: Some(key.to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: title.to_string(),
        description_markdown: String::new(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: ACTOR.to_string(),
            ..TaskProvenance::default()
        },
    })
}

async fn queued_task(
    service: &WorkCommandService,
    capture_key: &str,
    queue_key: &str,
    title: &str,
) -> (noema_tasks::TaskRecord, String) {
    let captured = service
        .execute(capture(capture_key, title))
        .await
        .expect("capture")
        .task
        .expect("captured task");
    let queued = service
        .execute(WorkCommand::QueueTask(noema_tasks::QueueTask {
            meta: CommandMeta {
                actor_id: ACTOR.to_string(),
                causation_id: None,
                correlation_id: format!("correlation:{queue_key}"),
                idempotency_key: Some(queue_key.to_string()),
            },
            precondition: noema_tasks::TaskPrecondition {
                task_id: captured.task_id.clone(),
                expected_revision: captured.revision,
                expected_generation: captured.generation,
            },
        }))
        .await
        .expect("queue")
        .task
        .expect("queued task");
    let run_id = service
        .store()
        .get_work_task(&queued.task_id)
        .await
        .expect("read queued task")
        .expect("task exists")
        .current_run
        .expect("queued run")
        .run_id;
    (queued, run_id)
}

async fn running_planner(
    service: &WorkCommandService,
    key: &str,
) -> (noema_tasks::TaskRecord, WorkRunFence) {
    let (task, _) = queued_task(
        service,
        &format!("{key}:capture"),
        &format!("{key}:queue"),
        key,
    )
    .await;
    let claimed = service
        .claim_next_work_run(&format!("worker:{key}"), 300, &[])
        .await
        .expect("claim")
        .expect("claimed run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: task.generation,
        contract_id: None,
    };
    service
        .start_work_run(&fence, ACTOR, None, &format!("correlation:{key}:start"))
        .await
        .expect("start run");
    (task, fence)
}

#[tokio::test]
async fn exact_run_lookup_keeps_cancelled_historical_runs_observable() {
    let (store, service) = fixture().await;
    let (task, run_id) = queued_task(
        &service,
        "run-lookup:capture",
        "run-lookup:queue",
        "historical run lookup",
    )
    .await;

    service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: CommandMeta {
                actor_id: ACTOR.to_string(),
                causation_id: None,
                correlation_id: "correlation:run-lookup:cancel".to_string(),
                idempotency_key: Some("run-lookup:cancel".to_string()),
            },
            precondition: TaskPrecondition {
                task_id: task.task_id,
                expected_revision: task.revision,
                expected_generation: task.generation,
            },
            reason: Some("verify historical lookup".to_string()),
        }))
        .await
        .expect("cancel queued task");

    let run = store
        .get_work_run_record(&run_id)
        .await
        .expect("load cancelled run")
        .expect("cancelled run remains durable");
    assert_eq!(run.run_id, run_id);
    assert_eq!(run.status, noema_tasks::RunStatus::Cancelled);
    assert!(run.cancellation_requested);
    assert!(run.lease_token.is_none());
    assert!(
        store
            .get_work_run_record("run:missing")
            .await
            .expect("missing lookup")
            .is_none()
    );
}

fn progress(fence: WorkRunFence) -> WorkRunProgress {
    WorkRunProgress {
        fence,
        actual_provider_kind: Some("test-provider".to_string()),
        actual_model_profile: Some("test-model".to_string()),
        provider_call_count_delta: 1,
        tool_call_count_delta: 2,
        input_tokens_delta: 100,
        cached_input_tokens_delta: 25,
        output_tokens_delta: 30,
        active_milliseconds_delta: 40,
    }
}

fn transcript_item(run_id: &str) -> NewAgentRunItem {
    NewAgentRunItem {
        item_id: Some("run_item:fence-test".to_string()),
        run_id: run_id.to_string(),
        round_index: 0,
        kind: AgentRunItemKind::ProgressNotice,
        status: AgentRunItemStatus::Completed,
        correlation_id: None,
        parent_item_id: None,
        content_text: Some("progress".to_string()),
        payload: serde_json::json!({}),
    }
}

#[tokio::test]
async fn transcript_writes_require_the_complete_live_run_fence() {
    let (store, service) = fixture().await;
    let (_task, fence) = running_planner(&service, "transcript-fence").await;
    store
        .append_agent_run_item(transcript_item(&fence.run_id), &fence)
        .await
        .expect("write with exact fence");

    let mut stale = fence.clone();
    stale.task_generation += 1;
    assert!(matches!(
        store
            .upsert_agent_run_item(transcript_item(&fence.run_id), &stale)
            .await,
        Err(StoreError::Work(WorkDomainError::RunFenced))
            | Err(StoreError::Work(WorkDomainError::StaleGeneration))
    ));
    service
        .report_work_run_failure(
            crate::ReportRunFailure {
                fence: fence.clone(),
                status: noema_tasks::RunStatus::Failed,
                error_code: noema_tasks::SafeErrorCode::new("transcript_test").expect("safe code"),
                error_message: None,
                retryable: false,
            },
            ACTOR,
            None,
            "correlation:transcript:fenced",
        )
        .await
        .expect("terminal failure");
    assert!(matches!(
        store
            .upsert_agent_run_item(transcript_item(&fence.run_id), &fence)
            .await,
        Err(StoreError::Work(WorkDomainError::RunFenced))
    ));
}

#[tokio::test]
async fn transcript_writes_reserve_context_checkpoint_kind_and_identity() {
    let (store, service) = fixture().await;
    let (_task, fence) = running_planner(&service, "transcript-checkpoint-reserved").await;
    let mut reserved_kind = transcript_item(&fence.run_id);
    reserved_kind.item_id = Some("run_item:caller-checkpoint".to_string());
    reserved_kind.kind = AgentRunItemKind::ContextCheckpoint;
    let kind_error = store
        .append_agent_run_item(reserved_kind, &fence)
        .await
        .expect_err("public append must reject Store checkpoint kind");
    assert!(matches!(
        kind_error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "run_item.context_checkpoint",
            ..
        })
    ));

    let mut reserved_id = transcript_item(&fence.run_id);
    reserved_id.item_id = Some(format!(
        "run_item:context_checkpoint:{}",
        fence
            .run_id
            .strip_prefix("run:")
            .unwrap_or(fence.run_id.as_str())
    ));
    let id_error = store
        .upsert_agent_run_item(reserved_id, &fence)
        .await
        .expect_err("public upsert must reject Store checkpoint identity");
    assert!(matches!(
        id_error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "run_item.context_checkpoint",
            ..
        })
    ));
    let item_count: i64 = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM agent_run_items WHERE run_id = ?1",
                    [fence.run_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("count reserved writes");
    assert_eq!(item_count, 0);
}

#[tokio::test]
async fn planner_retry_and_read_projection_keep_the_queued_policy_snapshot() {
    let (store, service) = fixture().await;
    let (task, _) = queued_task(
        &service,
        "policy-snapshot:capture",
        "policy-snapshot:queue",
        "policy snapshot",
    )
    .await;
    let queued = service
        .store()
        .get_work_task(&task.task_id)
        .await
        .expect("read queued task")
        .expect("queued task exists")
        .current_run
        .expect("queued planner");
    let original_policy = queued.execution_policy;
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE task_execution_policy SET max_automatic_retries = 0, max_review_rounds = 1 WHERE policy_id = 'default'",
                    [],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("change live default after queue");
    let projected = service
        .store()
        .get_work_task(&task.task_id)
        .await
        .expect("read projected task")
        .expect("projected task exists")
        .current_run
        .expect("projected planner");
    assert_eq!(projected.execution_policy, original_policy);

    let claimed = service
        .claim_next_work_run("worker:policy-snapshot", 60, &[])
        .await
        .expect("claim original planner")
        .expect("original planner");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: task.generation,
        contract_id: None,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:policy-snapshot:start")
        .await
        .expect("start original planner");
    service
        .report_work_run_failure(
            ReportRunFailure {
                fence,
                status: noema_tasks::RunStatus::Failed,
                error_code: noema_tasks::SafeErrorCode::new("policy_snapshot")
                    .expect("safe error code"),
                error_message: None,
                retryable: true,
            },
            ACTOR,
            None,
            "correlation:policy-snapshot:failure",
        )
        .await
        .expect("queue retry from pinned policy");
    let child = service
        .claim_next_work_run("worker:policy-snapshot-child", 60, &[])
        .await
        .expect("claim retry")
        .expect("retry exists despite live retry bound zero");
    assert_eq!(child.run.execution_policy, original_policy);
}

#[tokio::test]
async fn fifo_claim_uses_queued_at_then_run_id_for_equal_ties() {
    let (store, service) = fixture().await;
    let (_first_task, first_id) =
        queued_task(&service, "fifo:capture:a", "fifo:queue:a", "a").await;
    let (_second_task, second_id) =
        queued_task(&service, "fifo:capture:b", "fifo:queue:b", "b").await;

    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET queued_at = '2026-01-01T00:00:00.000Z' WHERE run_id IN (?1, ?2)",
                    rusqlite::params![first_id, second_id],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("tie queue timestamps");

    let expected_first = [first_id.as_str(), second_id.as_str()]
        .into_iter()
        .min()
        .expect("run ids");
    let claimed_first = service
        .claim_next_work_run("worker:fifo", 30, &[])
        .await
        .expect("claim first")
        .expect("first run");
    let claimed_second = service
        .claim_next_work_run("worker:fifo", 30, &[])
        .await
        .expect("claim second")
        .expect("second run");

    assert_eq!(claimed_first.run.run_id, expected_first);
    assert_ne!(claimed_first.run.run_id, claimed_second.run.run_id);
    assert_eq!(claimed_first.run.status, noema_tasks::RunStatus::Leased);
    assert_eq!(claimed_second.run.status, noema_tasks::RunStatus::Leased);
}

#[tokio::test]
async fn run_start_and_heartbeat_require_the_opaque_lease_fence() {
    let (store, service) = fixture().await;
    let (task, _run_id) = queued_task(&service, "fence:capture", "fence:queue", "fenced").await;
    let claimed = service
        .claim_next_work_run("worker:fence", 30, &[])
        .await
        .expect("claim")
        .expect("claimed run");
    let valid = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token.clone(),
        task_generation: task.generation,
        contract_id: None,
    };
    let mut wrong_token = valid.clone();
    wrong_token.lease_token = "lease:wrong".to_string();
    let mut wrong_contract = valid.clone();
    wrong_contract.contract_id =
        Some(noema_tasks::TaskContractId::new("contract:wrong").expect("contract id"));

    let start_error = service
        .start_work_run(&wrong_token, ACTOR, None, "correlation:fence")
        .await
        .expect_err("wrong lease token must fail");
    assert!(matches!(
        start_error,
        StoreError::Work(WorkDomainError::RunFenced)
    ));

    let started = service
        .start_work_run(&valid, ACTOR, None, "correlation:fence")
        .await
        .expect("start with exact fence");
    assert_eq!(started.status, noema_tasks::RunStatus::Running);

    let heartbeat_error = service
        .heartbeat_work_run(&wrong_token, 30)
        .await
        .expect_err("heartbeat must use the same lease");
    assert!(matches!(
        heartbeat_error,
        StoreError::Work(WorkDomainError::RunFenced)
    ));
    assert!(matches!(
        service.heartbeat_work_run(&wrong_contract, 30).await,
        Err(StoreError::Work(WorkDomainError::RunFenced))
    ));
    let heartbeat = service
        .heartbeat_work_run(&valid, 30)
        .await
        .expect("heartbeat");
    assert!(!heartbeat.cancellation_requested);

    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' WHERE run_id = ?1",
                    [valid.run_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("expire lease in test fixture");
    let expired = service
        .heartbeat_work_run(&valid, 30)
        .await
        .expect_err("expired lease must not be renewed");
    assert!(matches!(
        expired,
        StoreError::Work(WorkDomainError::RunFenced)
    ));
}

#[tokio::test]
async fn stale_generation_runs_are_not_claimable() {
    let (store, service) = fixture().await;
    let (task, run_id) =
        queued_task(&service, "generation:capture", "generation:queue", "stale").await;
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?1",
                    [task.task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("advance task generation");

    let claim = service
        .claim_next_work_run("worker:generation", 30, &[])
        .await
        .expect("claim query");
    assert!(claim.is_none(), "a run from an old generation is fenced");
    let status: String = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status FROM agent_runs WHERE run_id = ?1",
                    [run_id],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("read stale run");
    assert_eq!(status, "queued");
}

#[tokio::test]
async fn run_progress_adds_exact_aggregates_and_emits_post_update_heartbeat() {
    let (store, service) = fixture().await;
    let (_task, fence) = running_planner(&service, "progress:exact").await;
    let first = service
        .record_work_run_progress(progress(fence.clone()))
        .await
        .expect("first progress");
    assert_eq!(first.actual_provider_kind.as_deref(), Some("test-provider"));
    assert_eq!(first.actual_model_profile.as_deref(), Some("test-model"));
    assert_eq!(first.provider_call_count, 1);
    assert_eq!(first.tool_call_count, 2);
    assert_eq!(first.input_tokens, 100);
    assert_eq!(first.cached_input_tokens, 25);
    assert_eq!(first.output_tokens, 30);
    assert_eq!(first.active_milliseconds, 40);

    let mut second_progress = progress(fence.clone());
    second_progress.actual_provider_kind = Some("fallback-provider".to_string());
    second_progress.actual_model_profile = Some("fallback-model".to_string());
    second_progress.provider_call_count_delta = 2;
    second_progress.tool_call_count_delta = 1;
    second_progress.input_tokens_delta = 70;
    second_progress.cached_input_tokens_delta = 10;
    second_progress.output_tokens_delta = 20;
    second_progress.active_milliseconds_delta = 60;
    let second = service
        .record_work_run_progress(second_progress)
        .await
        .expect("second progress");
    assert_eq!(
        second.actual_provider_kind.as_deref(),
        Some("fallback-provider")
    );
    assert_eq!(
        second.actual_model_profile.as_deref(),
        Some("fallback-model")
    );
    assert_eq!(second.provider_call_count, 3);
    assert_eq!(second.tool_call_count, 3);
    assert_eq!(second.input_tokens, 170);
    assert_eq!(second.cached_input_tokens, 35);
    assert_eq!(second.output_tokens, 50);
    assert_eq!(second.active_milliseconds, 100);

    let payload: serde_json::Value = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT payload_json FROM work_events
                     WHERE run_id = ?1 AND event_kind = 'run.heartbeat'
                     ORDER BY event_sequence DESC LIMIT 1",
                    [fence.run_id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .and_then(|value| serde_json::from_str(&value).map_err(StoreError::Json))
        .expect("heartbeat payload");
    assert_eq!(payload["provider_call_count"], 3);
    assert_eq!(payload["tool_call_count"], 3);
    assert_eq!(payload["active_milliseconds"], 100);
}

#[tokio::test]
async fn run_progress_rejects_wrong_or_stale_fences_without_partial_writes() {
    let (store, service) = fixture().await;
    let (task, fence) = running_planner(&service, "progress:fence").await;
    let mut wrong_token = progress(fence.clone());
    wrong_token.fence.lease_token = "lease:wrong".to_string();
    let mut wrong_generation = progress(fence.clone());
    wrong_generation.fence.task_generation += 1;
    let mut wrong_contract = progress(fence.clone());
    wrong_contract.fence.contract_id =
        Some(noema_tasks::TaskContractId::new("contract:wrong").expect("contract id"));
    assert!(matches!(
        service.record_work_run_progress(wrong_token).await,
        Err(StoreError::Work(WorkDomainError::RunFenced))
    ));
    assert!(matches!(
        service.record_work_run_progress(wrong_generation).await,
        Err(StoreError::Work(WorkDomainError::StaleGeneration))
    ));
    assert!(matches!(
        service.record_work_run_progress(wrong_contract).await,
        Err(StoreError::Work(WorkDomainError::RunFenced))
    ));
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?1",
                [task.task_id.as_str()],
            )?;
            Ok(())
        })
        .await
        .expect("advance generation");
    assert!(matches!(
        service
            .record_work_run_progress(progress(fence.clone()))
            .await,
        Err(StoreError::Work(WorkDomainError::StaleGeneration))
    ));
    let (provider_calls, heartbeat_events): (i64, i64) = store
        .with_connection(|connection| {
            let calls = connection.query_row(
                "SELECT provider_call_count FROM agent_runs WHERE run_id = ?1",
                [fence.run_id.as_str()],
                |row| row.get(0),
            )?;
            let events = connection.query_row(
                "SELECT COUNT(*) FROM work_events
                 WHERE run_id = ?1 AND event_kind = 'run.heartbeat'",
                [fence.run_id.as_str()],
                |row| row.get(0),
            )?;
            Ok((calls, events))
        })
        .await
        .expect("read atomic outcome");
    assert_eq!((provider_calls, heartbeat_events), (0, 0));
}

#[tokio::test]
async fn run_progress_overflow_fails_closed_without_an_event() {
    let (store, service) = fixture().await;
    let (_task, fence) = running_planner(&service, "progress:overflow").await;
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE agent_runs SET provider_call_count = ?2 WHERE run_id = ?1",
                rusqlite::params![fence.run_id.as_str(), i64::from(u32::MAX)],
            )?;
            Ok(())
        })
        .await
        .expect("seed counter maximum");
    assert!(matches!(
        service
            .record_work_run_progress(progress(fence.clone()))
            .await,
        Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "run_progress.provider_call_count",
            ..
        }))
    ));
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE agent_runs SET provider_call_count = 0, input_tokens = ?2
                 WHERE run_id = ?1",
                rusqlite::params![fence.run_id.as_str(), i64::MAX],
            )?;
            Ok(())
        })
        .await
        .expect("seed token maximum");
    let token_only = WorkRunProgress {
        fence: fence.clone(),
        actual_provider_kind: None,
        actual_model_profile: None,
        provider_call_count_delta: 0,
        tool_call_count_delta: 0,
        input_tokens_delta: 1,
        cached_input_tokens_delta: 0,
        output_tokens_delta: 0,
        active_milliseconds_delta: 0,
    };
    assert!(matches!(
        service.record_work_run_progress(token_only).await,
        Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "run_progress.input_tokens",
            ..
        }))
    ));
    let heartbeat_events: i64 = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM work_events
                     WHERE run_id = ?1 AND event_kind = 'run.heartbeat'",
                    [fence.run_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("event count");
    assert_eq!(heartbeat_events, 0);
}
