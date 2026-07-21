//! Supervised Work planner/executor/reviewer runtime.
//!
//! Durable Work commands, leases, fences, and reconciliation remain owned by
//! `noema-store`. This coordinator claims FIFO runs, invokes the existing
//! governed provider loop, and schedules the next durable action after every
//! committed boundary.

use std::{collections::HashMap, sync::Arc, time::Duration};

use noema_home::SystemErrorLogger;
use noema_providers::ProviderRegistryHandle;
use noema_store::{
    ApplyReconciliation, ClaimedWorkRun, NoemaStore, WorkCommandService, WorkRunFence,
};
use noema_tasks::{RunStatus, SafeErrorCode, WorkReconciliationAction};
use noema_workspaces::WorkspaceId;
use serde_json::json;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::daemon::{
    RuntimeEventRegistry, RuntimeHandle, TaskRuntimeEvent, WorkRuntimeEvent, log_system_error,
};

mod execution;
#[path = "task_delivery.rs"]
mod notifications;

const LEASE_SECONDS: i64 = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(300);
const WORK_RUNTIME_ACTOR_ID: &str = "actor:runtime:worker";
/// The product-wide supervised worker cap. A cancelling future stays in the
/// JoinSet until it settles, so it continues to consume this cap naturally.
const MAX_CONCURRENT_TASK_RUNS: usize = 8;
const PERSONAL_WORKSPACE_ID: &str = "workspace:personal";

/// Handle for the supervised Work runtime.
#[derive(Clone)]
pub struct TaskRuntimeHandle {
    inner: Arc<TaskRuntimeInner>,
}

struct TaskRuntimeInner {
    cancellation: CancellationToken,
    join: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    active_runs: std::sync::Mutex<HashMap<String, ActiveRunCancellation>>,
}

struct ActiveRunCancellation {
    task_id: noema_tasks::TaskId,
    task_generation: u64,
    cancellation: CancellationToken,
}

#[derive(Clone)]
struct TaskRuntimeServices {
    store: NoemaStore,
    runtime: RuntimeHandle,
    provider_registry: ProviderRegistryHandle,
    system_errors: SystemErrorLogger,
    subscriptions: RuntimeEventRegistry,
}

impl std::fmt::Debug for TaskRuntimeHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TaskRuntimeHandle")
            .finish_non_exhaustive()
    }
}

impl TaskRuntimeHandle {
    /// Start reconciliation before claiming the first run.
    #[must_use]
    pub fn start(
        store: NoemaStore,
        runtime: RuntimeHandle,
        provider_registry: ProviderRegistryHandle,
        system_errors: SystemErrorLogger,
        subscriptions: RuntimeEventRegistry,
    ) -> Self {
        let inner = Arc::new(TaskRuntimeInner {
            cancellation: CancellationToken::new(),
            join: std::sync::Mutex::new(None),
            active_runs: std::sync::Mutex::new(HashMap::new()),
        });
        let services = TaskRuntimeServices {
            store,
            runtime,
            provider_registry,
            system_errors,
            subscriptions,
        };
        let worker_inner = Arc::clone(&inner);
        let join = tokio::spawn(async move { run_loop(services, worker_inner).await });
        *inner.join.lock().expect("task runtime join lock") = Some(join);
        Self { inner }
    }

    /// Stop the worker loop and wait for all supervised futures to settle.
    pub async fn shutdown(&self) {
        self.inner.cancellation.cancel();
        let join = self
            .inner
            .join
            .lock()
            .expect("task runtime join lock")
            .take();
        if let Some(join) = join {
            let _ = join.await;
        }
    }
}

async fn run_loop(services: TaskRuntimeServices, inner: Arc<TaskRuntimeInner>) {
    let worker_id = format!("work-runtime:{}", std::process::id());
    let service =
        WorkCommandService::new(services.store.clone(), services.provider_registry.clone());
    reconcile_all(&services, &service).await;
    let mut active_runs = JoinSet::new();
    let mut committed_work = services.subscriptions.subscribe_work(PERSONAL_WORKSPACE_ID);

    loop {
        if inner.cancellation.is_cancelled() {
            break;
        }
        let mut queue_available = true;
        while active_runs.len() < MAX_CONCURRENT_TASK_RUNS
            && queue_available
            && !inner.cancellation.is_cancelled()
        {
            let excluded_task_ids = active_task_ids(&inner);
            match service
                .claim_next_work_run(&worker_id, LEASE_SECONDS, &excluded_task_ids)
                .await
            {
                Ok(Some(claimed)) => {
                    let run_services = services.clone();
                    let shutdown = inner.cancellation.clone();
                    let run_inner = Arc::clone(&inner);
                    active_runs.spawn(async move {
                        supervise_claimed_run(run_services, claimed, shutdown, run_inner).await;
                    });
                }
                Ok(None) => queue_available = false,
                Err(error) => {
                    log_system_error(
                        &services.system_errors,
                        "work_runtime_claim_error",
                        "Work run queue could not be claimed",
                        None,
                        error,
                    );
                    queue_available = false;
                }
            }
        }

        notifications::drain_work_notifications(&services).await;
        tokio::select! {
            _ = inner.cancellation.cancelled() => break,
            event = committed_work.recv() => {
                match event {
                    Ok(WorkRuntimeEvent::Committed { task_id: Some(task_id), .. }) => {
                        if let Ok(task_id) = noema_tasks::TaskId::new(task_id) {
                            signal_stale_active_run(&services, &inner, &task_id).await;
                            reconcile_one(&services, &service, &task_id, None).await;
                        }
                    }
                    Ok(WorkRuntimeEvent::Committed { task_id: None, .. })
                    | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        reconcile_all(&services, &service).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
            completed = active_runs.join_next(), if !active_runs.is_empty() => {
                if let Some(Err(error)) = completed {
                    log_task_run_join_error(&services.system_errors, &error);
                }
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }

    while let Some(result) = active_runs.join_next().await {
        if let Err(error) = result {
            log_task_run_join_error(&services.system_errors, &error);
        }
    }
}

async fn supervise_claimed_run(
    services: TaskRuntimeServices,
    claimed: ClaimedWorkRun,
    shutdown: CancellationToken,
    inner: Arc<TaskRuntimeInner>,
) {
    let command_service =
        WorkCommandService::new(services.store.clone(), services.provider_registry.clone());
    let run = claimed.run;
    let fence = WorkRunFence {
        run_id: run.run_id.clone(),
        lease_token: claimed.lease_token.clone(),
        task_generation: run.task_generation,
        contract_id: run.contract_id.clone(),
    };
    publish_task_changed(&services.subscriptions, &run.task_id);
    let run_cancellation = CancellationToken::new();
    inner
        .active_runs
        .lock()
        .expect("active Work runs lock")
        .insert(
            run.run_id.clone(),
            ActiveRunCancellation {
                task_id: run.task_id.clone(),
                task_generation: run.task_generation,
                cancellation: run_cancellation.clone(),
            },
        );
    let execution = execution::execute_run(&services, &run, &fence, &run_cancellation);
    tokio::pin!(execution);
    let mut heartbeat = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(30),
        Duration::from_secs(30),
    );
    let mut interruption = None;
    let result = loop {
        tokio::select! {
            result = &mut execution => break result,
            _ = shutdown.cancelled() => {
                run_cancellation.cancel();
                interruption = Some("runtime shutdown interrupted the Work run".to_string());
            }
            _ = heartbeat.tick() => {
                match command_service.heartbeat_work_run(&fence, LEASE_SECONDS).await {
                    Ok(heartbeat) if heartbeat.cancellation_requested => {
                        run_cancellation.cancel();
                        interruption = Some("Work run cancellation was requested".to_string());
                    }
                    Ok(_) => {}
                    Err(error) => {
                        run_cancellation.cancel();
                        interruption = Some(error.to_string());
                    }
                }
            }
        }
        if interruption.is_some() {
            let unwind = (&mut execution).await;
            break unwind;
        }
    };

    let settled = services.store.get_work_run_record(&run.run_id).await;
    let durably_settled = settled
        .as_ref()
        .ok()
        .and_then(Option::as_ref)
        .is_some_and(|run| run_is_settled(run.status));
    let failure = if durably_settled {
        None
    } else if let Some(interruption) = interruption {
        Some(interruption)
    } else {
        match result {
            Ok(()) => Some(
                "Work worker returned without reaching a durable terminal or waiting status"
                    .to_string(),
            ),
            Err(error) => Some(error),
        }
    };
    if let Some(error) = failure {
        let code = SafeErrorCode::new(execution_error_code(&error))
            .unwrap_or_else(|_| SafeErrorCode::new("work_runtime_failed").expect("safe code"));
        match command_service
            .report_work_run_failure(
                noema_store::ReportRunFailure {
                    fence: fence.clone(),
                    status: if shutdown.is_cancelled() {
                        RunStatus::Interrupted
                    } else {
                        RunStatus::Failed
                    },
                    error_code: code,
                    error_message: Some(redact_runtime_error(&error)),
                    retryable: !shutdown.is_cancelled(),
                },
                WORK_RUNTIME_ACTOR_ID,
                Some(run.run_id.as_str()),
                &format!("correlation:run:{}", run.run_id),
            )
            .await
        {
            Ok(result) => {
                if let Some(task) = result.task.as_ref() {
                    publish_work_changed(&services.subscriptions, task);
                }
            }
            Err(report_error) => {
                log_system_error(
                    &services.system_errors,
                    "work_runtime_failure_report_failed",
                    "Work run failure could not be committed",
                    Some(json!({"run_id": run.run_id.clone(), "task_id": run.task_id.clone()})),
                    report_error,
                );
            }
        }
        log_system_error(
            &services.system_errors,
            "work_runtime_worker_error",
            "Background Work run failed",
            Some(json!({"run_id": run.run_id.clone(), "task_id": run.task_id.clone()})),
            error,
        );
    }
    assert_run_settled(&services, &run.run_id).await;
    publish_task_changed(&services.subscriptions, &run.task_id);
    reconcile_one(
        &services,
        &command_service,
        &run.task_id,
        Some(run.run_id.as_str()),
    )
    .await;
    unregister_active_run(&inner, &run.run_id);
}

fn unregister_active_run(inner: &TaskRuntimeInner, run_id: &str) {
    inner
        .active_runs
        .lock()
        .expect("active Work runs lock")
        .remove(run_id);
}

fn active_task_ids(inner: &TaskRuntimeInner) -> Vec<noema_tasks::TaskId> {
    let active_runs = inner.active_runs.lock().expect("active Work runs lock");
    let mut task_ids = Vec::with_capacity(active_runs.len());
    for active in active_runs.values() {
        if !task_ids.contains(&active.task_id) {
            task_ids.push(active.task_id.clone());
        }
    }
    task_ids
}

async fn signal_stale_active_run(
    services: &TaskRuntimeServices,
    inner: &TaskRuntimeInner,
    task_id: &noema_tasks::TaskId,
) {
    let current_generation = match services.store.get_work_task(task_id).await {
        Ok(Some(detail)) => Some(detail.task.generation),
        Ok(None) => None,
        Err(error) => {
            log_system_error(
                &services.system_errors,
                "work_runtime_cancellation_fence_read_failed",
                "Work cancellation fence could not be read",
                Some(json!({"task_id": task_id})),
                error,
            );
            None
        }
    };
    let active_runs = inner.active_runs.lock().expect("active Work runs lock");
    for active in active_runs
        .values()
        .filter(|active| &active.task_id == task_id)
    {
        if current_generation.is_none_or(|generation| generation != active.task_generation) {
            active.cancellation.cancel();
        }
    }
}

fn log_task_run_join_error(system_errors: &SystemErrorLogger, error: &tokio::task::JoinError) {
    log_system_error(
        system_errors,
        "work_runtime_worker_join_error",
        "Work worker stopped unexpectedly",
        None,
        error,
    );
}

async fn reconcile_all(services: &TaskRuntimeServices, service: &WorkCommandService) {
    let Ok(workspace_id) = WorkspaceId::new(PERSONAL_WORKSPACE_ID) else {
        return;
    };
    let mut after = None;
    loop {
        let query = noema_store::WorkTaskQuery {
            workspace_id: workspace_id.clone(),
            project_id: None,
            stage_ids: Vec::new(),
            stage_behaviors: Vec::new(),
            text: None,
            attention_only: false,
            scope: noema_store::WorkTaskScope::Active,
            first: noema_store::WorkPageSize::new(100).expect("bounded page size"),
            after,
        };
        let connection = match services.store.list_work_tasks(query).await {
            Ok(connection) => connection,
            Err(error) => {
                log_system_error(
                    &services.system_errors,
                    "work_reconciliation_page_failed",
                    "Work reconciliation page could not be loaded",
                    None,
                    error,
                );
                return;
            }
        };
        for edge in &connection.edges {
            reconcile_one(services, service, &edge.node.task.task_id, None).await;
        }
        if !connection.page_info.has_next_page {
            break;
        }
        let Some(end_cursor) = connection.page_info.end_cursor else {
            log_system_error(
                &services.system_errors,
                "work_reconciliation_cursor_missing",
                "Work reconciliation page omitted its continuation cursor",
                None,
                "active Work page reported a next page without an end cursor",
            );
            break;
        };
        after = match noema_store::WorkTaskCursor::decode(&end_cursor) {
            Ok(cursor) => Some(cursor),
            Err(error) => {
                log_system_error(
                    &services.system_errors,
                    "work_reconciliation_cursor_invalid",
                    "Work reconciliation page returned an invalid cursor",
                    None,
                    error,
                );
                break;
            }
        };
    }
}

async fn assert_run_settled(services: &TaskRuntimeServices, run_id: &str) {
    match services.store.get_work_run_record(run_id).await {
        Ok(Some(run)) if run_is_settled(run.status) => {}
        Ok(Some(run)) => log_system_error(
            &services.system_errors,
            "work_runtime_worker_returned_active",
            "Work worker returned without reaching a settled status",
            Some(json!({"run_id": run_id, "status": run.status.as_str()})),
            "worker returned while its durable run remained active",
        ),
        Ok(None) => log_system_error(
            &services.system_errors,
            "work_runtime_worker_run_missing",
            "Work worker returned but its durable run is missing",
            Some(json!({"run_id": run_id})),
            "worker run disappeared after execution",
        ),
        Err(error) => log_system_error(
            &services.system_errors,
            "work_runtime_worker_status_failed",
            "Work worker status could not be verified after execution",
            Some(json!({"run_id": run_id})),
            error,
        ),
    }
}

fn run_is_settled(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Completed
            | RunStatus::WaitingForApproval
            | RunStatus::Interrupted
            | RunStatus::Failed
            | RunStatus::Cancelled
    )
}

async fn reconcile_one(
    services: &TaskRuntimeServices,
    service: &WorkCommandService,
    task_id: &noema_tasks::TaskId,
    causation_id: Option<&str>,
) {
    let envelope = match services
        .store
        .load_work_reconciliation_snapshot(task_id)
        .await
    {
        Ok(Some(envelope)) => envelope,
        Ok(None) => return,
        Err(error) => {
            log_system_error(
                &services.system_errors,
                "work_reconciliation_snapshot_failed",
                "Work reconciliation snapshot could not be loaded",
                Some(json!({"task_id": task_id})),
                error,
            );
            return;
        }
    };
    let action = match noema_store::plan_work_reconciliation(&envelope) {
        Ok(action) => action,
        Err(error) => {
            log_system_error(
                &services.system_errors,
                "work_reconciliation_plan_failed",
                "Work reconciliation could not derive a next action",
                Some(json!({"task_id": task_id})),
                error,
            );
            return;
        }
    };
    if matches!(action, WorkReconciliationAction::Idle) {
        return;
    }
    let request = ApplyReconciliation {
        task_id: task_id.clone(),
        actor_id: "actor:runtime:reconciler".to_string(),
        correlation_id: format!("correlation:task:{}", task_id),
        causation_id: causation_id.map(ToOwned::to_owned),
    };
    match service.apply_work_reconciliation_action(request).await {
        Ok(result) => {
            services
                .subscriptions
                .publish_task(TaskRuntimeEvent::Changed {
                    task_id: task_id.to_string(),
                    run_id: None,
                });
            if let Some(task) = result.task.as_ref() {
                publish_work_changed(&services.subscriptions, task);
            }
        }
        Err(error) => log_system_error(
            &services.system_errors,
            "work_reconciliation_apply_failed",
            "Work reconciliation action could not be committed",
            Some(json!({"task_id": task_id})),
            error,
        ),
    }
}

fn publish_task_changed(subscriptions: &RuntimeEventRegistry, task_id: &noema_tasks::TaskId) {
    subscriptions.publish_task(TaskRuntimeEvent::Changed {
        task_id: task_id.to_string(),
        run_id: None,
    });
}

fn publish_work_changed(subscriptions: &RuntimeEventRegistry, task: &noema_tasks::TaskRecord) {
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: task.workspace_id.to_string(),
        task_id: Some(task.task_id.to_string()),
    });
}

fn redact_runtime_error(error: &str) -> String {
    error
        .lines()
        .next()
        .unwrap_or("Work runtime failure")
        .chars()
        .take(1024)
        .collect()
}

fn execution_error_code(error: &str) -> &'static str {
    if error.contains("terminal") {
        "work_terminal_invalid"
    } else if error.contains("provider") || error.contains("model") {
        "work_provider_failed"
    } else if error.contains("lease") || error.contains("fence") {
        "run_fenced"
    } else {
        "work_runtime_failed"
    }
}

#[cfg(test)]
#[path = "task_runtime/tests.rs"]
mod tests;
