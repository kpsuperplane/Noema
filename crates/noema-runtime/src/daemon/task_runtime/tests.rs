use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, GenerateToolCall, ProviderError,
    ProviderHandle, ProviderToolCapabilities, ProviderToolTransport,
};
use noema_store::WorkCommandService;
use noema_tasks::{
    CancelTask, CaptureTask, CommandMeta, MissedRunPolicy, NewTaskSchedule, QueueTask, ReopenTask,
    RunKind, RunStatus, TaskPrecondition, TaskProvenance, TaskReopenDirection, TaskSourceKind,
    WorkCommand,
};
use noema_workspaces::WorkspaceId;
use tokio::sync::mpsc;

use super::{
    ClaimRenewalEvidence, MAX_CONCURRENT_TASK_RUNS, PERSONAL_WORKSPACE_ID, TaskRuntimeHandle,
    TaskRuntimeServices, claim_renewal_event,
    notifications::{fail_notification, notification_work_event},
    reconcile_all, task_generation_retains_browser_session,
};
use crate::daemon::{RuntimeError, RuntimeEventRegistry, RuntimeHandle, WorkRuntimeEvent};

#[test]
fn delayed_claim_renewal_records_the_required_timing_and_phase() {
    let event = claim_renewal_event(
        &ClaimRenewalEvidence {
            run_id: "run:test",
            planned_at_unix_ms: 1_000,
            started_at_unix_ms: 7_000,
            start_delay: Duration::from_secs(6),
            sqlite_duration: Duration::from_millis(19),
            time_before_expiry: Duration::from_secs(84),
            shutdown_requested: false,
            run_cancellation_requested: false,
            active_phase: Some("initial".to_string()),
        },
        None,
    );

    assert_eq!(event.category, "task_run_claim_renewal_delayed");
    assert_eq!(event.context["planned_at_unix_ms"], 1_000);
    assert_eq!(event.context["started_at_unix_ms"], 7_000);
    assert_eq!(event.context["start_delay_ms"], 6_000);
    assert_eq!(event.context["sqlite_duration_ms"], 19);
    assert_eq!(event.context["time_before_expiry_ms"], 84_000);
    assert_eq!(event.context["shutdown_requested"], false);
    assert_eq!(event.context["run_cancellation_requested"], false);
    assert_eq!(event.context["active_phase"], "initial");
}

#[tokio::test]
async fn browser_cleanup_follows_current_task_generation_and_terminal_state() {
    let store = crate::test_support::test_store().await;
    let (task, _) = crate::test_support::seed_task(&store, "Browser cleanup policy").await;
    let active = store
        .get_work_task(&task.task_id)
        .await
        .expect("load task")
        .expect("task detail");
    let generation = active.task.generation;
    let mut completed = active.clone();
    completed.stage.system_behavior = noema_tasks::WorkflowStageBehavior::TerminalSuccess;
    let mut cancelled = active.clone();
    cancelled.stage.system_behavior = noema_tasks::WorkflowStageBehavior::TerminalCancelled;
    let mut changed = active.clone();
    changed.task.generation += 1;

    assert!(task_generation_retains_browser_session(
        Some(&active),
        generation
    ));
    assert!(!task_generation_retains_browser_session(
        Some(&completed),
        generation
    ));
    assert!(!task_generation_retains_browser_session(
        Some(&cancelled),
        generation
    ));
    assert!(!task_generation_retains_browser_session(
        Some(&changed),
        generation
    ));
    assert!(!task_generation_retains_browser_session(None, generation));
}

#[tokio::test]
async fn approval_continuation_retains_browser_session_for_same_generation() {
    let store = crate::test_support::test_store().await;
    let (task, _) = crate::test_support::seed_task(&store, "Browser approval").await;
    let mut parent = store
        .get_work_task(&task.task_id)
        .await
        .expect("load task")
        .expect("task detail");
    parent.task.active_gate_id =
        Some(noema_tasks::TaskGateId::new("gate:browser-approval".to_string()).expect("gate id"));
    parent.stage.system_behavior = noema_tasks::WorkflowStageBehavior::HumanGate;
    let generation = parent.task.generation;
    let mut child = parent.clone();
    child.task.active_gate_id = None;
    child.task.latest_run_id = Some("run:approval-child".to_string());
    child.stage.system_behavior = noema_tasks::WorkflowStageBehavior::Active;

    assert!(task_generation_retains_browser_session(
        Some(&parent),
        generation
    ));
    assert!(task_generation_retains_browser_session(
        Some(&child),
        generation
    ));
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderEvent {
    Started(String),
    Settling(String),
    Settled(String),
}

#[derive(Debug)]
struct BlockingProvider {
    events: Option<mpsc::UnboundedSender<ProviderEvent>>,
    cleanup_release: Option<Arc<Mutex<Option<std::sync::mpsc::Receiver<()>>>>>,
    terminal: bool,
}

#[derive(Debug)]
struct FailingProvider;

#[derive(Debug)]
struct TerminalRepairProvider {
    always_invalid: bool,
    executor_calls: AtomicUsize,
}

impl TerminalRepairProvider {
    fn new(always_invalid: bool) -> Self {
        Self {
            always_invalid,
            executor_calls: AtomicUsize::new(0),
        }
    }
}

impl noema_providers::ProviderOperations for TerminalRepairProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            allowed_tools: true,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let Some(result_tool) = request
                .tools
                .iter()
                .find(|tool| tool.name.as_str() == "task.finish_execution")
            else {
                return std::future::pending().await;
            };
            let _ = result_tool;
            let call_index = self.executor_calls.fetch_add(1, Ordering::SeqCst);
            let payload = if call_index == 0 || self.always_invalid {
                serde_json::json!({"unexpected": true})
            } else {
                serde_json::json!({})
            };
            Ok(GenerateResponse {
                responses: Vec::new(),
                tool_calls: vec![GenerateToolCall {
                    id: Some(format!("call:terminal:{call_index}")),
                    provider_call_id: None,
                    provider_name: None,
                    name: "task.finish_execution".to_string(),
                    payload,
                }],
                reasoning_items: Vec::new(),
                hosted_web_searches: Vec::new(),
                provider: "test".to_string(),
                model: request.model.unwrap_or_else(|| "test-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}

impl noema_providers::ProviderOperations for FailingProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async {
            Err(ProviderError::ProviderUnavailable {
                provider: "test".to_string(),
                message: "simulated task failure".to_string(),
            })
        })
    }
}

impl BlockingProvider {
    fn supervised(events: mpsc::UnboundedSender<ProviderEvent>) -> Self {
        Self {
            events: Some(events),
            cleanup_release: None,
            terminal: false,
        }
    }

    fn cleaning_up(
        events: mpsc::UnboundedSender<ProviderEvent>,
        release: std::sync::mpsc::Receiver<()>,
    ) -> Self {
        Self {
            events: Some(events),
            cleanup_release: Some(Arc::new(Mutex::new(Some(release)))),
            terminal: false,
        }
    }

    fn terminal() -> Self {
        Self {
            events: None,
            cleanup_release: None,
            terminal: true,
        }
    }
}

#[test]
fn execution_failure_uses_typed_semantics() {
    let uncertain = RuntimeError::OutcomeUncertain;
    let wording = RuntimeError::Protocol("provider terminal lease fence".to_string());
    let terminal = RuntimeError::TaskTerminalInvalid("invalid payload".to_string());
    let invariant = RuntimeError::from(noema_store::StoreError::InvariantViolation {
        message: "deterministic context fault".to_string(),
    });
    let invalid_context = RuntimeError::from(noema_store::StoreError::Work(
        noema_tasks::WorkDomainError::InvalidInput {
            field: "run.context_checkpoint",
            message: "deterministic context fault".to_string(),
        },
    ));
    let other_invalid_input = RuntimeError::from(noema_store::StoreError::Work(
        noema_tasks::WorkDomainError::InvalidInput {
            field: "other.input",
            message: "retry through the existing runtime policy".to_string(),
        },
    ));
    let classify = super::execution_failure;
    assert_eq!(classify(&uncertain), ("unsafe_effect_uncertain", false));
    assert_eq!(classify(&wording), ("work_runtime_failed", true));
    assert_eq!(classify(&terminal), ("work_terminal_invalid", false));
    assert_eq!(classify(&invariant), ("invariant_fault", false));
    assert_eq!(classify(&invalid_context), ("invariant_fault", false));
    assert_eq!(
        classify(&other_invalid_input),
        ("work_runtime_failed", true)
    );
}

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

impl noema_providers::ProviderOperations for BlockingProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            if self.terminal {
                return Ok(GenerateResponse {
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
                    hosted_web_searches: Vec::new(),
                    provider: "test".to_string(),
                    model: request.model.unwrap_or_else(|| "test-model".to_string()),
                    response_id: None,
                    usage: None,
                });
            }
            let run_id = request
                .conversation_id
                .as_deref()
                .and_then(|id| id.strip_prefix("task_run:"))
                .unwrap_or("unknown")
                .to_string();
            let events = self.events.as_ref().expect("event provider");
            let _settlement = ProviderSettlement {
                run_id: run_id.clone(),
                events: events.clone(),
                cleanup_release: self.cleanup_release.clone(),
            };
            let _ = events.send(ProviderEvent::Started(run_id));
            std::future::pending().await
        })
    }
}

async fn start_task_runtime(
    provider: ProviderHandle,
    store: &noema_store::NoemaStore,
    subscriptions: RuntimeEventRegistry,
) -> (RuntimeHandle, TaskRuntimeHandle) {
    let runtime = RuntimeHandle::spawn_with_provider(provider, store.clone())
        .await
        .expect("runtime");
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        crate::test_support::ready_test_provider_registry(),
        crate::test_support::system_error_logger(),
        subscriptions,
    );
    (runtime, task_runtime)
}

async fn next_event(
    events: &mut mpsc::UnboundedReceiver<ProviderEvent>,
    context: &str,
) -> ProviderEvent {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect(context)
        .expect("provider event")
}

async fn assert_no_event(events: &mut mpsc::UnboundedReceiver<ProviderEvent>, context: &str) {
    assert!(
        tokio::time::timeout(Duration::from_millis(150), events.recv())
            .await
            .is_err(),
        "{context}"
    );
}

async fn current_task(
    store: &noema_store::NoemaStore,
    task_id: &noema_tasks::TaskId,
) -> noema_tasks::TaskRecord {
    store
        .get_work_task(task_id)
        .await
        .expect("load task")
        .expect("task exists")
        .task
}

async fn wait_for_run_status(
    store: &noema_store::NoemaStore,
    run_id: &str,
    expected: RunStatus,
) -> noema_tasks::AgentRunRecord {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let run = store
                .get_work_run_record(run_id)
                .await
                .expect("load run")
                .expect("run exists");
            if run.status == expected {
                break run;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("run should reach expected status")
}

fn publish_task(subscriptions: &RuntimeEventRegistry, task_id: &noema_tasks::TaskId) {
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: PERSONAL_WORKSPACE_ID.to_string(),
        task_id: Some(task_id.to_string()),
    });
}

fn precondition(task: &noema_tasks::TaskRecord) -> TaskPrecondition {
    TaskPrecondition {
        task_id: task.task_id.clone(),
        expected_revision: task.revision,
        expected_generation: task.generation,
    }
}

fn cancel_command(key: &str, task: &noema_tasks::TaskRecord, reason: &str) -> WorkCommand {
    WorkCommand::CancelTask(CancelTask {
        meta: command_meta(key),
        precondition: precondition(task),
        reason: Some(reason.to_string()),
    })
}

fn queue_command(key: &str, task: &noema_tasks::TaskRecord) -> WorkCommand {
    WorkCommand::QueueTask(QueueTask {
        meta: command_meta(key),
        precondition: precondition(task),
    })
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
    let subscriptions = RuntimeEventRegistry::default();
    let (runtime, task_runtime) = start_task_runtime(
        Arc::new(BlockingProvider::supervised(event_tx)),
        &store,
        subscriptions.clone(),
    )
    .await;

    let mut started = Vec::new();
    while started.len() < MAX_CONCURRENT_TASK_RUNS {
        let event = next_event(&mut event_rx, "one of the first eight runs should start").await;
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
    assert_no_event(
        &mut event_rx,
        "the ninth run must not start while all eight supervisor slots are occupied",
    )
    .await;

    let cancelled_run_id = started[0].clone();
    let (cancelled_task, _) = seeded
        .iter()
        .find(|(_, run)| run.run_id == cancelled_run_id)
        .expect("cancelled task")
        .clone();
    let current = current_task(&store, &cancelled_task.task_id).await;
    let command_service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    command_service
        .execute(cancel_command(
            "cancel-capped-run",
            &current,
            "test cancellation",
        ))
        .await
        .expect("cancel running task");
    publish_task(&subscriptions, &current.task_id);

    assert_eq!(
        next_event(&mut event_rx, "cancelled provider should settle").await,
        ProviderEvent::Settled(cancelled_run_id.clone())
    );
    assert_eq!(
        next_event(&mut event_rx, "ninth run should start after settlement").await,
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
    let subscriptions = RuntimeEventRegistry::default();
    let (runtime, task_runtime) = start_task_runtime(
        Arc::new(BlockingProvider::cleaning_up(event_tx, cleanup_rx)),
        &store,
        subscriptions.clone(),
    )
    .await;
    assert_eq!(
        next_event(&mut event_rx, "old run should start").await,
        ProviderEvent::Started(old_run.run_id.clone())
    );

    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let running = current_task(&store, &task.task_id).await;
    service
        .execute(cancel_command(
            "cancel-overlap",
            &running,
            "replace execution generation",
        ))
        .await
        .expect("cancel old run");
    publish_task(&subscriptions, &task.task_id);
    assert_eq!(
        next_event(&mut event_rx, "old provider should enter cleanup").await,
        ProviderEvent::Settling(old_run.run_id.clone())
    );

    let cancelled = current_task(&store, &task.task_id).await;
    let reopened = service
        .execute(WorkCommand::ReopenTask(ReopenTask {
            meta: command_meta("reopen-overlap"),
            precondition: precondition(&cancelled),
            direction: TaskReopenDirection {
                feedback_markdown: "Try again after the prior run settles.".to_string(),
                request_markdown: None,
                complexity: None,
            },
        }))
        .await
        .expect("reopen task")
        .task
        .expect("reopened task");
    assert_eq!(
        reopened.stage_id.as_str(),
        noema_tasks::PERSONAL_QUEUE_STAGE_ID
    );
    publish_task(&subscriptions, &task.task_id);
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
    assert_no_event(
        &mut event_rx,
        "successor provider must not start during predecessor cleanup",
    )
    .await;

    cleanup_tx.send(()).expect("release old provider cleanup");
    assert_eq!(
        next_event(&mut event_rx, "old provider should settle").await,
        ProviderEvent::Settled(old_run.run_id)
    );
    assert_eq!(
        next_event(&mut event_rx, "successor should start after cleanup").await,
        ProviderEvent::Started(successor.run_id)
    );

    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test]
async fn failed_run_publishes_work_invalidation_for_automatic_replacement() {
    let store = crate::test_support::test_store().await;
    let (task, failed_run) = crate::test_support::seed_task(&store, "Automatic retry event").await;
    let subscriptions = RuntimeEventRegistry::default();
    let mut work_events = subscriptions.subscribe_work(PERSONAL_WORKSPACE_ID);
    let (runtime, task_runtime) =
        start_task_runtime(Arc::new(FailingProvider), &store, subscriptions).await;

    let event = tokio::time::timeout(Duration::from_secs(2), work_events.recv())
        .await
        .expect("failed run should publish a Work invalidation")
        .expect("Work event stream should remain open");
    let WorkRuntimeEvent::Committed {
        workspace_id,
        task_id,
    } = event;
    assert_eq!(workspace_id, PERSONAL_WORKSPACE_ID);
    assert_eq!(task_id.as_deref(), Some(task.task_id.as_str()));

    let detail = store
        .get_work_task(&task.task_id)
        .await
        .expect("load task after failure")
        .expect("task should remain available");
    assert!(
        detail
            .runs
            .iter()
            .any(|run| run.run_id != failed_run.run_id)
    );

    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test]
async fn runtime_shutdown_queues_an_interrupted_run_for_retry() {
    let store = crate::test_support::test_store().await;
    let (task, interrupted_run) = crate::test_support::seed_task(&store, "Shutdown retry").await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (runtime, task_runtime) = start_task_runtime(
        Arc::new(BlockingProvider::supervised(event_tx)),
        &store,
        RuntimeEventRegistry::default(),
    )
    .await;
    assert_eq!(
        next_event(&mut event_rx, "run should start before shutdown").await,
        ProviderEvent::Started(interrupted_run.run_id.clone())
    );

    task_runtime.shutdown().await;

    let detail = store
        .get_work_task(&task.task_id)
        .await
        .expect("load task after shutdown")
        .expect("task should remain available");
    assert_eq!(
        detail
            .runs
            .iter()
            .find(|run| run.run_id == interrupted_run.run_id)
            .expect("interrupted run")
            .status,
        RunStatus::Interrupted
    );
    assert_eq!(detail.active_gate, None);
    let replacement = detail.current_run.expect("queued replacement run");
    assert_ne!(replacement.run_id, interrupted_run.run_id);
    assert_eq!(replacement.status, RunStatus::Queued);

    runtime.shutdown().await;
}

#[tokio::test]
async fn malformed_terminal_is_repaired_in_the_same_executor_run() {
    let store = crate::test_support::test_store().await;
    let (task, run) = crate::test_support::seed_task(&store, "Terminal repair").await;
    store
        .write_task_file(&task.task_id, noema_store::TASK_RESULT, "Repaired result.")
        .await
        .expect("write result");
    let provider = Arc::new(TerminalRepairProvider::new(false));
    let (runtime, task_runtime) =
        start_task_runtime(provider.clone(), &store, RuntimeEventRegistry::default()).await;

    let settled = wait_for_run_status(&store, &run.run_id, RunStatus::Completed).await;
    assert_eq!(settled.attempt_index, 0);
    assert_eq!(provider.executor_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        settled.run_id, run.run_id,
        "repair must stay in the same run"
    );

    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test]
async fn second_malformed_terminal_fails_nonretryably_into_recovery() {
    let store = crate::test_support::test_store().await;
    let (task, run) = crate::test_support::seed_task(&store, "Terminal recovery").await;
    let provider = Arc::new(TerminalRepairProvider::new(true));
    let (runtime, task_runtime) =
        start_task_runtime(provider.clone(), &store, RuntimeEventRegistry::default()).await;

    let settled = wait_for_run_status(&store, &run.run_id, RunStatus::Failed).await;
    let detail = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let detail = store
                .get_work_task(&task.task_id)
                .await
                .expect("load recovery task")
                .expect("recovery task");
            if detail.active_gate.is_some() {
                break detail;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("invalid terminal should open recovery");

    assert_eq!(settled.error_code.as_deref(), Some("work_terminal_invalid"));
    assert_eq!(provider.executor_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        detail
            .runs
            .iter()
            .filter(|candidate| candidate.run_kind == RunKind::Executor)
            .count(),
        1,
        "a second invalid terminal must not queue an automatic retry"
    );
    assert_eq!(
        detail.active_gate.expect("recovery gate").kind,
        noema_tasks::TaskGateKind::Recovery
    );

    drop(task_runtime);
    drop(runtime);
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
        .execute(queue_command("queue-tail", &tail))
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
async fn newly_earlier_schedule_replaces_the_runtime_deadline_without_duplicate_execution() {
    let store = crate::test_support::test_store().await;
    crate::test_support::initialize_codex_provider_selections(&store).await;
    let subscriptions = RuntimeEventRegistry::default();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (runtime, task_runtime) = start_task_runtime(
        Arc::new(BlockingProvider::supervised(event_tx)),
        &store,
        subscriptions.clone(),
    )
    .await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let now = super::unix_now();
    let scheduled = |key: &str, title: &str, at: i64| {
        let mut command = capture_command(key, title);
        command.schedule = Some(NewTaskSchedule {
            scheduled_for: at,
            time_zone: "UTC".to_string(),
            missed_run_policy: MissedRunPolicy::RunOnce,
            recurrence: None,
        });
        WorkCommand::CaptureTask(command)
    };
    let late = service
        .execute(scheduled("late-schedule", "Late", now + 30))
        .await
        .unwrap()
        .task
        .unwrap();
    publish_task(&subscriptions, &late.task_id);
    let early = service
        .execute(scheduled("early-schedule", "Early", now + 1))
        .await
        .unwrap()
        .task
        .unwrap();
    publish_task(&subscriptions, &early.task_id);

    let started = next_event(&mut event_rx, "earlier scheduled task should start").await;
    assert!(matches!(started, ProviderEvent::Started(_)));
    assert_ne!(
        current_task(&store, &early.task_id).await.stage_id.as_str(),
        noema_tasks::PERSONAL_INBOX_STAGE_ID
    );
    assert_eq!(
        current_task(&store, &late.task_id).await.stage_id.as_str(),
        noema_tasks::PERSONAL_INBOX_STAGE_ID
    );
    assert_no_event(&mut event_rx, "scheduled occurrence executes only once").await;
    task_runtime.shutdown().await;
    runtime.shutdown().await;
}

#[tokio::test]
async fn successful_worker_return_keeps_its_terminal_waiting_status() {
    let store = crate::test_support::test_store().await;
    let (_task, run) = crate::test_support::seed_task(&store, "Settled worker status").await;
    let (runtime, task_runtime) = start_task_runtime(
        Arc::new(BlockingProvider::terminal()),
        &store,
        RuntimeEventRegistry::default(),
    )
    .await;
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
        task_document_markdown: title.to_string(),
        project_id: None,
        executor_agent_id: None,
        cwd_override: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::System,
            created_by_actor_id: "actor:test".to_string(),
            ..TaskProvenance::default()
        },
        schedule: None,
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
