//! Focused pure-planning and idempotent-application tests for reconciliation.

use noema_tasks::{
    CaptureTask, CommandMeta, TaskProvenance, TaskSourceKind, WorkCommand,
    WorkReconciliationAction, WorkReconciliationSnapshot, WorkflowStageBehavior,
};
use noema_workspaces::WorkspaceId;

use crate::{
    ApplyReconciliation, NoemaStore, StoreError, WorkCommandService, plan_snapshot,
    plan_work_reconciliation,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:store:reconciler";

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

fn capture() -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: CommandMeta {
            actor_id: "actor:human:local".to_string(),
            causation_id: None,
            correlation_id: "correlation:reconcile:capture".to_string(),
            idempotency_key: Some("reconcile:capture".to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: "reconciliation task".to_string(),
        description_markdown: String::new(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: "actor:human:local".to_string(),
            ..TaskProvenance::default()
        },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DurableCounts {
    events: i64,
    gates: i64,
    contracts: i64,
    runs: i64,
    notifications: i64,
}

async fn durable_counts(store: &NoemaStore) -> DurableCounts {
    store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                       (SELECT COUNT(*) FROM work_events),
                       (SELECT COUNT(*) FROM task_gates),
                       (SELECT COUNT(*) FROM task_execution_contracts),
                       (SELECT COUNT(*) FROM agent_runs),
                       (SELECT COUNT(*) FROM work_notification_outbox)",
                    [],
                    |row| {
                        Ok(DurableCounts {
                            events: row.get(0)?,
                            gates: row.get(1)?,
                            contracts: row.get(2)?,
                            runs: row.get(3)?,
                            notifications: row.get(4)?,
                        })
                    },
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("durable reconciliation counts")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartBoundary {
    QueuedRun,
    ResolvedRecoveryGate,
}

async fn prepare_resolved_recovery_gate(store: &NoemaStore, task_id: &noema_tasks::TaskId) {
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE tasks
                 SET stage_id = 'stage:personal:waiting', active_gate_id = NULL,
                     queued_at = NULL, revision = 2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE task_id = ?1",
                [task_id.as_str()],
            )?;
            connection.execute(
                "INSERT INTO task_gates (
                    gate_id, task_id, task_generation, gate_kind, gate_state,
                    recovery_reason, retry_run_kind, prompt_markdown,
                    context_markdown, opened_by_actor_id
                 ) VALUES (
                    'gate:reconcile-resolved', ?1, 1, 'recovery', 'open',
                    'infrastructure_retries_exhausted', 'planner',
                    'Resume planning?', 'The planner can resume.', 'actor:store:reconciler'
                 )",
                [task_id.as_str()],
            )?;
            connection.execute(
                "INSERT INTO task_messages (
                    message_id, task_id, task_generation, gate_id, message_kind,
                    body_markdown, author_actor_id
                 ) VALUES (
                    'task_message:reconcile-resolved', ?1, 1,
                    'gate:reconcile-resolved', 'retry_note',
                    'Resume planning after restart.', 'actor:human:local'
                 )",
                [task_id.as_str()],
            )?;
            connection.execute(
                "UPDATE task_gates
                 SET gate_state = 'resolved', resolved_by_actor_id = 'actor:human:local',
                     resolution_message_id = 'task_message:reconcile-resolved',
                     resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE gate_id = 'gate:reconcile-resolved'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("prepare resolved recovery gate");
}

#[test]
fn pure_reconciliation_planner_returns_one_closed_action() {
    let queue = plan_snapshot(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Dispatch,
        ..WorkReconciliationSnapshot::default()
    })
    .expect("dispatch action");
    assert_eq!(
        queue,
        WorkReconciliationAction::QueueRun {
            run_kind: noema_tasks::RunKind::Planner
        }
    );

    let recovery = plan_snapshot(WorkReconciliationSnapshot {
        stage_behavior: WorkflowStageBehavior::Active,
        failed_run: Some(noema_tasks::WorkFailedRunFacts {
            run_kind: noema_tasks::RunKind::Planner,
            status: noema_tasks::RunStatus::Failed,
            retryable: true,
            retries_exhausted: true,
            recovery_reason: Some(noema_tasks::TaskRecoveryReason::InfrastructureRetriesExhausted),
        }),
        ..WorkReconciliationSnapshot::default()
    })
    .expect("recovery action");
    assert_eq!(
        recovery,
        WorkReconciliationAction::OpenRecoveryGate {
            reason: noema_tasks::TaskRecoveryReason::InfrastructureRetriesExhausted,
            retry_run_kind: Some(noema_tasks::RunKind::Planner),
        }
    );
}

#[tokio::test]
async fn applying_idle_twice_is_a_durable_noop_with_the_same_cursor() {
    let (store, service) = fixture().await;
    let task = service
        .execute(capture())
        .await
        .expect("capture")
        .task
        .expect("task");
    let envelope = store
        .load_work_reconciliation_snapshot(&task.task_id)
        .await
        .expect("load snapshot")
        .expect("snapshot exists");
    assert_eq!(
        envelope.snapshot.stage_behavior,
        WorkflowStageBehavior::Intake
    );
    assert!(!envelope.snapshot.has_runnable_run);
    assert_eq!(
        plan_work_reconciliation(&envelope).expect("plan"),
        WorkReconciliationAction::Idle
    );

    let before_events = event_count(&store).await;
    let request = || ApplyReconciliation {
        task_id: task.task_id.clone(),
        actor_id: ACTOR.to_string(),
        correlation_id: "correlation:reconcile:idle".to_string(),
        causation_id: None,
    };
    let first = service
        .apply_work_reconciliation_action(request())
        .await
        .expect("apply first idle");
    let second = service
        .apply_work_reconciliation_action(request())
        .await
        .expect("apply second idle");
    assert_eq!(first.event_id, second.event_id);
    assert_eq!(first.event_sequence, second.event_sequence);
    assert_eq!(event_count(&store).await, before_events);
}

#[tokio::test]
async fn reconciliation_application_rederives_current_facts_in_the_write_transaction() {
    let (store, service) = fixture().await;
    let task = service
        .execute(capture())
        .await
        .expect("capture")
        .task
        .expect("task");
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET stage_id = 'stage:personal:queue', queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1",
                    [task.task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("change durable reconciliation facts");

    let result = service
        .apply_work_reconciliation_action(ApplyReconciliation {
            task_id: task.task_id,
            actor_id: ACTOR.to_string(),
            correlation_id: "correlation:reconcile:stale".to_string(),
            causation_id: None,
        })
        .await
        .expect("reconcile current facts");
    assert!(result.run_id.is_some());
    assert_eq!(
        result.task.expect("task").stage_id.as_str(),
        "stage:personal:queue"
    );
}

#[tokio::test]
async fn reconciliation_restart_matrix_is_idempotent_after_durable_boundaries() {
    for boundary in [
        RestartBoundary::QueuedRun,
        RestartBoundary::ResolvedRecoveryGate,
    ] {
        let (store, service) = fixture().await;
        let task = service
            .execute(capture())
            .await
            .expect("capture")
            .task
            .expect("task");
        match boundary {
            RestartBoundary::QueuedRun => {
                service
                    .execute(WorkCommand::QueueTask(noema_tasks::QueueTask {
                        meta: CommandMeta {
                            actor_id: ACTOR.to_string(),
                            causation_id: None,
                            correlation_id: "correlation:reconcile:restart:queue".to_string(),
                            idempotency_key: Some("reconcile:restart:queue".to_string()),
                        },
                        precondition: noema_tasks::TaskPrecondition {
                            task_id: task.task_id.clone(),
                            expected_revision: task.revision,
                            expected_generation: task.generation,
                        },
                    }))
                    .await
                    .expect("queue before restart");
            }
            RestartBoundary::ResolvedRecoveryGate => {
                prepare_resolved_recovery_gate(&store, &task.task_id).await;
            }
        }

        let before = durable_counts(&store).await;
        let request = || ApplyReconciliation {
            task_id: task.task_id.clone(),
            actor_id: ACTOR.to_string(),
            correlation_id: format!("correlation:reconcile:restart:{boundary:?}"),
            causation_id: None,
        };
        let first = service
            .apply_work_reconciliation_action(request())
            .await
            .expect("first reconciliation pass");
        let after_first = durable_counts(&store).await;
        let second = service
            .apply_work_reconciliation_action(request())
            .await
            .expect("second reconciliation pass");
        let after_second = durable_counts(&store).await;

        assert_eq!(
            second.event_sequence, first.event_sequence,
            "restart boundary {boundary:?} changed its event marker"
        );
        assert_eq!(
            after_second, after_first,
            "restart boundary {boundary:?} duplicated a durable event, gate, contract, run, or notice"
        );
        assert!(after_first.events >= before.events);
        assert!(after_first.gates >= before.gates);
        assert!(after_first.contracts >= before.contracts);
        assert!(after_first.runs >= before.runs);
        assert!(after_first.notifications >= before.notifications);
    }
}
