use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateResponseStatus, GenerateStreamEvent,
    GenerateToolCall, ProviderError, ProviderToolCapabilities, ProviderToolTransport,
};
use noema_store::WorkCommandService;
use noema_tasks::{
    CancelTask, CaptureTask, CommandMeta, QueueTask, ReopenTask, RunStatus, TaskPrecondition,
    TaskProvenance, TaskSourceKind, WorkCommand,
};
use noema_workspaces::WorkspaceId;
use tokio::sync::mpsc;

use super::{
    MAX_CONCURRENT_TASK_RUNS, PERSONAL_WORKSPACE_ID, TaskRuntimeHandle, TaskRuntimeServices,
    notifications::{fail_notification, notification_work_event},
    reconcile_all,
};
use crate::daemon::{RuntimeEventRegistry, RuntimeHandle, WorkRuntimeEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderEvent {
    Started(String),
    Settling(String),
    Settled(String),
}

#[derive(Debug)]
struct SupervisedBlockingProvider {
    events: mpsc::UnboundedSender<ProviderEvent>,
}

#[derive(Debug)]
struct CleanupBlockingProvider {
    events: mpsc::UnboundedSender<ProviderEvent>,
    cleanup_release: Arc<Mutex<Option<std::sync::mpsc::Receiver<()>>>>,
}

#[derive(Debug)]
struct BlockingTerminalProvider;

struct ProviderSettlement {
    run_id: String,
    events: mpsc::UnboundedSender<ProviderEvent>,
    cleanup_release: Option<Arc<Mutex<Option<std::sync::mpsc::Receiver<()>>>>>,
}

impl Drop for ProviderSettlement {
    fn drop(&mut self) {
        if let Some(cleanup_release) = self.cleanup_release.as_ref() {
            let _ = self
                .events
                .send(ProviderEvent::Settling(self.run_id.clone()));
            if let Some(release) = cleanup_release.lock().expect("cleanup release lock").take() {
                let _ = release.recv();
            }
        }
        let _ = self
            .events
            .send(ProviderEvent::Settled(self.run_id.clone()));
    }
}

impl noema_providers::ProviderOperations for SupervisedBlockingProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let run_id = request
                .conversation_id
                .as_deref()
                .and_then(|id| id.strip_prefix("task_run:"))
                .unwrap_or("unknown")
                .to_string();
            let _settlement = ProviderSettlement {
                run_id: run_id.clone(),
                events: self.events.clone(),
                cleanup_release: None,
            };
            let _ = self.events.send(ProviderEvent::Started(run_id));
            std::future::pending().await
        })
    }
}

impl noema_providers::ProviderOperations for CleanupBlockingProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let run_id = request
                .conversation_id
                .as_deref()
                .and_then(|id| id.strip_prefix("task_run:"))
                .unwrap_or("unknown")
                .to_string();
            let _settlement = ProviderSettlement {
                run_id: run_id.clone(),
                events: self.events.clone(),
                cleanup_release: Some(Arc::clone(&self.cleanup_release)),
            };
            let _ = self.events.send(ProviderEvent::Started(run_id));
            std::future::pending().await
        })
    }
}

impl noema_providers::ProviderOperations for BlockingTerminalProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(GenerateResponse {
                responses: Vec::new(),
                tool_calls: vec![GenerateToolCall {
                    id: Some("call:block".to_string()),
                    provider_call_id: None,
                    provider_name: None,
                    name: "task.report_blocked".to_string(),
                    payload: serde_json::json!({
                        "gate_kind": "clarification",
                        "question": "Which region?",
                        "context_markdown": "The contract has no authorized region.",
                    }),
                }],
                reasoning_items: Vec::new(),
                response_status: GenerateResponseStatus::NeedsTools,
                provider: "test".to_string(),
                model: request.model.unwrap_or_else(|| "test-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}

#[tokio::test]
async fn supervisor_enforces_fifo_cap_and_releases_ninth_only_after_cancelled_run_settles() {
    let store = crate::test_support::test_store().await;
    let mut seeded = Vec::new();
    for index in 0..=MAX_CONCURRENT_TASK_RUNS {
        seeded.push(crate::test_support::seed_task(&store, &format!("Capped task {index}")).await);
    }
    let mut fifo = seeded
        .iter()
        .map(|(_, run)| (run.queued_at.clone(), run.run_id.clone()))
        .collect::<Vec<_>>();
    fifo.sort();
    let expected_first = fifo
        .iter()
        .take(MAX_CONCURRENT_TASK_RUNS)
        .map(|(_, run_id)| run_id.clone())
        .collect::<Vec<_>>();
    let expected_ninth = fifo[MAX_CONCURRENT_TASK_RUNS].1.clone();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime = RuntimeHandle::spawn_with_provider(
        Arc::new(SupervisedBlockingProvider { events: event_tx }),
        store.clone(),
    )
    .await
    .expect("runtime");
    let subscriptions = RuntimeEventRegistry::default();
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        crate::test_support::ready_test_provider_registry(),
        crate::test_support::system_error_logger(),
        subscriptions.clone(),
    );

    let mut started = Vec::new();
    while started.len() < MAX_CONCURRENT_TASK_RUNS {
        let event = tokio::time::timeout(Duration::from_secs(3), event_rx.recv())
            .await
            .expect("one of the first eight runs should start")
            .expect("provider event");
        match event {
            ProviderEvent::Started(run_id) => started.push(run_id),
            ProviderEvent::Settling(run_id) => {
                panic!("run {run_id} began settling before cancellation")
            }
            ProviderEvent::Settled(run_id) => panic!("run {run_id} settled before cancellation"),
        }
    }
    assert_eq!(
        started.iter().cloned().collect::<HashSet<_>>(),
        expected_first.iter().cloned().collect::<HashSet<_>>(),
        "the occupied slots must be the exact durable FIFO prefix"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(150), event_rx.recv())
            .await
            .is_err(),
        "the ninth run must not start while all eight supervisor slots are occupied"
    );

    let cancelled_run_id = started[0].clone();
    let (cancelled_task, _) = seeded
        .iter()
        .find(|(_, run)| run.run_id == cancelled_run_id)
        .expect("cancelled task")
        .clone();
    let current = store
        .get_work_task(&cancelled_task.task_id)
        .await
        .expect("load running task")
        .expect("running task")
        .task;
    let command_service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    command_service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: command_meta("cancel-capped-run"),
            precondition: TaskPrecondition {
                task_id: current.task_id.clone(),
                expected_revision: current.revision,
                expected_generation: current.generation,
            },
            reason: Some("test cancellation".to_string()),
        }))
        .await
        .expect("cancel running task");
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: PERSONAL_WORKSPACE_ID.to_string(),
        task_id: Some(current.task_id.to_string()),
    });

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("cancelled provider should settle")
            .expect("settlement event"),
        ProviderEvent::Settled(cancelled_run_id.clone())
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("ninth run should start after settlement")
            .expect("ninth provider event"),
        ProviderEvent::Started(expected_ninth)
    );
    assert_eq!(
        store
            .get_work_run_record(&cancelled_run_id)
            .await
            .expect("load cancelled run")
            .expect("cancelled run")
            .status,
        RunStatus::Cancelled,
        "worker cleanup must not overwrite the durable cancellation fence"
    );

    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_run_stays_excluded_until_its_provider_future_fully_settles() {
    let store = crate::test_support::test_store().await;
    let (task, old_run) = crate::test_support::seed_task(&store, "Same-task settlement").await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (cleanup_tx, cleanup_rx) = std::sync::mpsc::channel();
    let runtime = RuntimeHandle::spawn_with_provider(
        Arc::new(CleanupBlockingProvider {
            events: event_tx,
            cleanup_release: Arc::new(Mutex::new(Some(cleanup_rx))),
        }),
        store.clone(),
    )
    .await
    .expect("runtime");
    let subscriptions = RuntimeEventRegistry::default();
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        crate::test_support::ready_test_provider_registry(),
        crate::test_support::system_error_logger(),
        subscriptions.clone(),
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("old run should start")
            .expect("provider event"),
        ProviderEvent::Started(old_run.run_id.clone())
    );

    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let running = store
        .get_work_task(&task.task_id)
        .await
        .expect("load running task")
        .expect("running task")
        .task;
    service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: command_meta("cancel-overlap"),
            precondition: TaskPrecondition {
                task_id: running.task_id.clone(),
                expected_revision: running.revision,
                expected_generation: running.generation,
            },
            reason: Some("replace execution generation".to_string()),
        }))
        .await
        .expect("cancel old run");
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: PERSONAL_WORKSPACE_ID.to_string(),
        task_id: Some(task.task_id.to_string()),
    });
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("old provider should enter cleanup")
            .expect("provider event"),
        ProviderEvent::Settling(old_run.run_id.clone())
    );

    let cancelled = store
        .get_work_task(&task.task_id)
        .await
        .expect("load cancelled task")
        .expect("cancelled task")
        .task;
    let reopened = service
        .execute(WorkCommand::ReopenTask(ReopenTask {
            meta: command_meta("reopen-overlap"),
            precondition: TaskPrecondition {
                task_id: cancelled.task_id.clone(),
                expected_revision: cancelled.revision,
                expected_generation: cancelled.generation,
            },
        }))
        .await
        .expect("reopen task")
        .task
        .expect("reopened task");
    service
        .execute(WorkCommand::QueueTask(QueueTask {
            meta: command_meta("queue-overlap"),
            precondition: TaskPrecondition {
                task_id: reopened.task_id.clone(),
                expected_revision: reopened.revision,
                expected_generation: reopened.generation,
            },
        }))
        .await
        .expect("queue successor");
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: PERSONAL_WORKSPACE_ID.to_string(),
        task_id: Some(task.task_id.to_string()),
    });
    let successor = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(run) = store
                .get_work_task(&task.task_id)
                .await
                .expect("load reopened task")
                .expect("reopened task")
                .current_run
            {
                break run;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("reconciliation should queue successor");
    assert_eq!(successor.status, RunStatus::Queued);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), event_rx.recv())
            .await
            .is_err(),
        "successor provider must not start during predecessor cleanup"
    );

    cleanup_tx.send(()).expect("release old provider cleanup");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("old provider should settle")
            .expect("provider event"),
        ProviderEvent::Settled(old_run.run_id)
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("successor should start after cleanup")
            .expect("provider event"),
        ProviderEvent::Started(successor.run_id)
    );

    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test]
async fn failed_notification_publishes_exact_work_invalidation() {
    let store = crate::test_support::test_store().await;
    let notification = crate::daemon::tests::seed_waiting_notification(&store).await;
    let subscriptions = RuntimeEventRegistry::default();
    let expected_event = notification_work_event(&notification).expect("notification Work scope");
    let mut work_events = subscriptions.subscribe_work(PERSONAL_WORKSPACE_ID);
    let runtime = RuntimeHandle::spawn_with_provider(
        crate::contract_test_support::fixed_response_provider("unused"),
        store.clone(),
    )
    .await
    .expect("runtime");
    let services = TaskRuntimeServices {
        store,
        runtime: runtime.clone(),
        provider_registry: crate::test_support::ready_test_provider_registry(),
        system_errors: crate::test_support::system_error_logger(),
        subscriptions,
    };

    fail_notification(
        &services,
        notification.notification_id,
        notification.lease_token,
        "delivery_failed",
        "test delivery failure",
        Some(expected_event.clone()),
    )
    .await;

    let received = tokio::time::timeout(Duration::from_secs(1), work_events.recv())
        .await
        .expect("failed notification wakeup")
        .expect("Work event");
    let WorkRuntimeEvent::Committed {
        workspace_id,
        task_id,
    } = received;
    let WorkRuntimeEvent::Committed {
        workspace_id: expected_workspace_id,
        task_id: expected_task_id,
    } = expected_event;
    assert_eq!(workspace_id, expected_workspace_id);
    assert_eq!(task_id, expected_task_id);
    runtime.shutdown().await;
}

#[tokio::test]
async fn reconciliation_recovers_an_active_task_beyond_the_first_hundred_rows() {
    let store = crate::test_support::test_store().await;
    crate::test_support::initialize_codex_provider_selections(&store).await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let tail = service
        .execute(WorkCommand::CaptureTask(capture_command(
            "tail",
            "Tail recovery",
        )))
        .await
        .expect("capture tail task")
        .task
        .expect("tail task");
    service
        .execute(WorkCommand::QueueTask(QueueTask {
            meta: command_meta("queue-tail"),
            precondition: TaskPrecondition {
                task_id: tail.task_id.clone(),
                expected_revision: tail.revision,
                expected_generation: tail.generation,
            },
        }))
        .await
        .expect("queue tail task");
    let old_run_id = store
        .get_work_task(&tail.task_id)
        .await
        .expect("load tail")
        .expect("tail exists")
        .current_run
        .expect("tail planner")
        .run_id;
    noema_store::test_support::force_work_task_missing_current_run(
        &store,
        &tail.task_id,
        &old_run_id,
    )
    .await
    .expect("create missing-run reconciliation state");
    for index in 0..100 {
        service
            .execute(WorkCommand::CaptureTask(capture_command(
                &format!("head-{index}"),
                &format!("Newer task {index}"),
            )))
            .await
            .expect("capture newer task");
    }
    let runtime = RuntimeHandle::spawn_with_provider(
        crate::contract_test_support::fixed_response_provider("unused"),
        store.clone(),
    )
    .await
    .expect("runtime");
    let services = TaskRuntimeServices {
        store: store.clone(),
        runtime: runtime.clone(),
        provider_registry: crate::test_support::ready_test_provider_registry(),
        system_errors: crate::test_support::system_error_logger(),
        subscriptions: RuntimeEventRegistry::default(),
    };
    let first_page = store
        .list_work_tasks(noema_store::WorkTaskQuery {
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).expect("workspace id"),
            project_id: None,
            stage_ids: Vec::new(),
            stage_behaviors: Vec::new(),
            text: None,
            attention_only: false,
            scope: noema_store::WorkTaskScope::Active,
            first: noema_store::WorkPageSize::new(100).expect("page size"),
            after: None,
        })
        .await
        .expect("first active page");
    assert!(first_page.page_info.has_next_page);
    assert!(
        first_page
            .edges
            .iter()
            .all(|edge| edge.node.task.task_id != tail.task_id)
    );
    let second_page = store
        .list_work_tasks(noema_store::WorkTaskQuery {
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).expect("workspace id"),
            project_id: None,
            stage_ids: Vec::new(),
            stage_behaviors: Vec::new(),
            text: None,
            attention_only: false,
            scope: noema_store::WorkTaskScope::Active,
            first: noema_store::WorkPageSize::new(100).expect("page size"),
            after: Some(
                noema_store::WorkTaskCursor::decode(
                    &first_page.page_info.end_cursor.expect("first page cursor"),
                )
                .expect("decode first page cursor"),
            ),
        })
        .await
        .expect("second active page");
    assert!(
        second_page
            .edges
            .iter()
            .any(|edge| edge.node.task.task_id == tail.task_id)
    );
    reconcile_all(&services, &service).await;
    let recovered = store
        .get_work_task(&tail.task_id)
        .await
        .expect("load recovered tail")
        .expect("recovered tail")
        .current_run
        .expect("reconciled planner run");

    assert_ne!(recovered.run_id, old_run_id);
    assert_eq!(recovered.status, RunStatus::Queued);
    runtime.shutdown().await;
}

#[tokio::test]
async fn successful_worker_return_keeps_its_terminal_waiting_status() {
    let store = crate::test_support::test_store().await;
    let (_task, run) = crate::test_support::seed_task(&store, "Settled worker status").await;
    let runtime =
        RuntimeHandle::spawn_with_provider(Arc::new(BlockingTerminalProvider), store.clone())
            .await
            .expect("runtime");
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        crate::test_support::ready_test_provider_registry(),
        crate::test_support::system_error_logger(),
        RuntimeEventRegistry::default(),
    );
    let settled = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let current = store
                .get_work_run_record(&run.run_id)
                .await
                .expect("load worker run")
                .expect("worker run");
            if current.status == RunStatus::WaitingForApproval {
                break current;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker should reach a terminal waiting state");

    assert_eq!(settled.status, RunStatus::WaitingForApproval);
    assert_eq!(settled.error_code, None);
    assert_eq!(settled.error_message, None);
    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

fn capture_command(key: &str, title: &str) -> CaptureTask {
    CaptureTask {
        meta: command_meta(key),
        workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).expect("workspace id"),
        title: title.to_string(),
        description_markdown: title.to_string(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::System,
            created_by_actor_id: "actor:test".to_string(),
            ..TaskProvenance::default()
        },
    }
}

fn command_meta(key: &str) -> CommandMeta {
    CommandMeta {
        actor_id: "actor:test".to_string(),
        causation_id: None,
        correlation_id: format!("correlation:{key}"),
        idempotency_key: Some(format!("idempotency:{key}")),
    }
}
