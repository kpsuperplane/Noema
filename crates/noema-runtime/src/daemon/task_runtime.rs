//! Supervised background executor/reviewer coordinator.

use std::{sync::Arc, time::Duration};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use noema_tasks::{
    CriterionOutcome, NewTaskReview, NewTaskSubmission, RunKind, RunStatus,
    SubmissionCriterionEvidence, TaskReviewCriterion, TaskReviewVerdict, TaskStatus,
};

use noema_store::NoemaStore;

use crate::{
    agent_execution::ExecutionRole,
    daemon::{
        RuntimeEventRegistry, RuntimeHandle, TaskRuntimeEvent, log_system_error,
        runtime::BackgroundTaskGenerateRequest,
        task_run_context::{
            ExecutorBlockedResponse, ExecutorSubmissionResponse, ReviewerResponse,
            format_executor_prompt, format_reviewer_prompt,
            human_continuation_context_for_submission, with_resume_context,
        },
        task_tool::{TASK_REPORT_BLOCKED_TOOL, TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL},
    },
};
use noema_home::SystemErrorLogger;
use noema_providers::ProviderRegistryHandle;

const LEASE_SECONDS: i64 = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(300);
const MAX_CONCURRENT_TASK_RUNS: usize = 8;

/// Runtime handle for supervised task workers.
#[derive(Clone)]
pub struct TaskRuntimeHandle {
    inner: Arc<TaskRuntimeInner>,
}

struct TaskRuntimeInner {
    cancellation: CancellationToken,
    join: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
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
    /// Start the supervised worker loop.
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
        });
        let services = TaskRuntimeServices {
            store,
            runtime,
            provider_registry,
            system_errors,
            subscriptions,
        };
        let worker_inner = Arc::clone(&inner);
        let join = tokio::spawn(async move {
            run_loop(services, worker_inner).await;
        });
        *inner.join.lock().expect("task runtime join lock") = Some(join);
        Self { inner }
    }

    /// Request cancellation and join the worker loop.
    pub async fn shutdown(&self) {
        self.inner.cancellation.cancel();
        let join = {
            self.inner
                .join
                .lock()
                .expect("task runtime join lock")
                .take()
        };
        if let Some(join) = join {
            let _ = join.await;
        }
    }
}

async fn run_loop(services: TaskRuntimeServices, inner: Arc<TaskRuntimeInner>) {
    let worker_id = format!("task-worker:{}", std::process::id());
    let mut active_runs = tokio::task::JoinSet::new();
    loop {
        if inner.cancellation.is_cancelled() {
            break;
        }
        let mut queue_available = true;
        while active_runs.len() < MAX_CONCURRENT_TASK_RUNS
            && queue_available
            && !inner.cancellation.is_cancelled()
        {
            let lease_token = format!("{}:{}", worker_id, uuid_fragment());
            match services
                .store
                .claim_next_agent_run_with_readiness(
                    &worker_id,
                    &lease_token,
                    LEASE_SECONDS,
                    services.provider_registry.as_ref(),
                )
                .await
            {
                Ok(Some(run)) => {
                    let run_services = services.clone();
                    let shutdown = inner.cancellation.clone();
                    active_runs.spawn(async move {
                        supervise_claimed_run(run_services, run, lease_token, shutdown).await;
                    });
                }
                Ok(None) => queue_available = false,
                Err(error) => {
                    log_system_error(
                        &services.system_errors,
                        "task_runtime_claim_error",
                        "Background task queue could not be read",
                        None,
                        error,
                    );
                    queue_available = false;
                }
            }
        }

        drain_task_status_outbox(
            &services.store,
            &services.runtime,
            &services.subscriptions,
            &services.system_errors,
        )
        .await;
        tokio::select! {
            _ = inner.cancellation.cancelled() => break,
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
    run: noema_tasks::AgentRunRecord,
    lease_token: String,
    shutdown: CancellationToken,
) {
    publish_task_changed(&services.subscriptions, &run.task_id);
    let run_cancellation = CancellationToken::new();
    if let Err(error) = supervise_run(
        &services.store,
        &services.runtime,
        &services.provider_registry,
        &services.subscriptions,
        &run,
        &lease_token,
        &run_cancellation,
        &shutdown,
    )
    .await
    {
        fail_run(
            &services.store,
            &services.subscriptions,
            &run,
            &lease_token,
            &error,
        )
        .await;
        publish_task_changed(&services.subscriptions, &run.task_id);
        log_system_error(
            &services.system_errors,
            "task_runtime_worker_error",
            "Background task run failed",
            Some(json!({"run_id": run.run_id, "task_id": run.task_id})),
            error,
        );
    } else {
        publish_task_changed(&services.subscriptions, &run.task_id);
    }
}

fn log_task_run_join_error(system_errors: &SystemErrorLogger, error: &tokio::task::JoinError) {
    log_system_error(
        system_errors,
        "task_runtime_worker_join_error",
        "Background task worker stopped unexpectedly",
        None,
        error,
    );
}

async fn drain_task_status_outbox(
    store: &NoemaStore,
    runtime: &RuntimeHandle,
    subscriptions: &RuntimeEventRegistry,
    system_errors: &SystemErrorLogger,
) {
    let task_ids = match store.list_pending_task_status_deliveries(32).await {
        Ok(task_ids) => task_ids,
        Err(error) => {
            log_system_error(
                system_errors,
                "task_status_outbox_read_failed",
                "Task status delivery queue could not be read",
                None,
                error,
            );
            return;
        }
    };
    for task_id in task_ids {
        if let Err(error) =
            crate::daemon::task_delivery::deliver_task_status_event(store, subscriptions, &task_id)
                .await
        {
            log_system_error(
                system_errors,
                "task_status_delivery_failed",
                "Task status update could not be delivered",
                Some(json!({"task_id": task_id})),
                error,
            );
        }
    }

    crate::daemon::task_delivery::drain_task_completion_outbox(store, runtime, system_errors).await;
}

#[allow(clippy::too_many_arguments)]
async fn supervise_run(
    store: &NoemaStore,
    runtime: &RuntimeHandle,
    provider_registry: &ProviderRegistryHandle,
    subscriptions: &RuntimeEventRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    run_cancellation: &CancellationToken,
    shutdown: &CancellationToken,
) -> Result<(), String> {
    let execution = execute_run(
        store,
        runtime,
        provider_registry,
        subscriptions,
        run,
        lease_token,
        run_cancellation,
    );
    tokio::pin!(execution);
    let mut heartbeat = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(30),
        Duration::from_secs(30),
    );
    loop {
        tokio::select! {
            result = &mut execution => {
                result?;
                let current = store
                    .get_agent_run(&run.run_id)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "task run disappeared after execution".to_string())?;
                if matches!(current.status, RunStatus::Leased | RunStatus::Running) {
                    return Err(
                        "task execution returned without committing a terminal run state"
                            .to_string(),
                    );
                }
                return Ok(());
            },
            _ = shutdown.cancelled() => {
                run_cancellation.cancel();
                persist_terminal_run_notice(store, run, lease_token, noema_tasks::AgentRunItemKind::Cancellation, noema_tasks::AgentRunItemStatus::Cancelled, "Task worker stopped; the run will continue after restart.").await;
                let _ = store
                    .transition_agent_run(&run.run_id, RunStatus::Interrupted, Some(lease_token), None)
                    .await;
                return Ok(());
            }
            _ = heartbeat.tick() => {
                match store.heartbeat_agent_run(&run.run_id, lease_token, LEASE_SECONDS).await {
                    Ok(heartbeat) if heartbeat.cancellation_requested => {
                        run_cancellation.cancel();
                        persist_terminal_run_notice(store, run, lease_token, noema_tasks::AgentRunItemKind::Cancellation, noema_tasks::AgentRunItemStatus::Cancelled, "Task cancelled by its owner.").await;
                        let _ = store
                            .transition_agent_run(&run.run_id, RunStatus::Cancelled, Some(lease_token), None)
                            .await;
                        let _ = crate::daemon::task_delivery::deliver_task_status_event(
                            store,
                            subscriptions,
                            &run.task_id,
                        )
                        .await;
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(_) => {
                        run_cancellation.cancel();
                        return Ok(());
                    }
                }
            }
        }
    }
}

async fn fail_run(
    store: &NoemaStore,
    subscriptions: &RuntimeEventRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    error: &str,
) {
    if store
        .get_agent_run(&run.run_id)
        .await
        .ok()
        .flatten()
        .is_some_and(|current| current.cancellation_requested)
    {
        persist_terminal_run_notice(
            store,
            run,
            lease_token,
            noema_tasks::AgentRunItemKind::Cancellation,
            noema_tasks::AgentRunItemStatus::Cancelled,
            "Task cancelled by its owner.",
        )
        .await;
        let _ = store
            .transition_agent_run(&run.run_id, RunStatus::Cancelled, Some(lease_token), None)
            .await;
        let _ = crate::daemon::task_delivery::deliver_task_status_event(
            store,
            subscriptions,
            &run.task_id,
        )
        .await;
        return;
    }
    let error_code = task_error_code(error);
    persist_terminal_run_notice(
        store,
        run,
        lease_token,
        noema_tasks::AgentRunItemKind::Failure,
        noema_tasks::AgentRunItemStatus::Failed,
        error,
    )
    .await;
    let _ = store
        .transition_agent_run(
            &run.run_id,
            RunStatus::Failed,
            Some(lease_token),
            Some((error_code.to_string(), error.to_string())),
        )
        .await;
    if let Ok(Some(task)) = store.get_task(&run.task_id).await {
        if !task.status.is_terminal() && task.status.can_transition_to(TaskStatus::Failed) {
            let _ = store
                .transition_task(&task.task_id, TaskStatus::Failed, Some(error))
                .await;
        }
        let _ = crate::daemon::task_delivery::deliver_task_status_event(
            store,
            subscriptions,
            &task.task_id,
        )
        .await;
    }
}

async fn persist_terminal_run_notice(
    store: &NoemaStore,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    kind: noema_tasks::AgentRunItemKind,
    status: noema_tasks::AgentRunItemStatus,
    message: &str,
) {
    let round_index = store
        .get_agent_run(&run.run_id)
        .await
        .ok()
        .flatten()
        .map_or(run.provider_call_count, |current| {
            current.provider_call_count
        });
    let _ = store
        .append_agent_run_item(
            noema_tasks::NewAgentRunItem {
                item_id: Some(format!("run_item:{kind}:{}", run.run_id)),
                run_id: run.run_id.clone(),
                round_index,
                kind,
                status,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some(message.to_string()),
                payload: json!({}),
            },
            lease_token,
        )
        .await;
}

fn task_error_code(error: &str) -> &'static str {
    if error.contains("terminal contract missing") {
        "task_terminal_contract_missing"
    } else if error.contains("invalid executor") || error.contains("invalid reviewer") {
        "task_terminal_contract_invalid"
    } else if error.contains("provider") || error.contains("model") {
        "task_provider_failed"
    } else if error.contains("lease") {
        "task_lease_lost"
    } else {
        "task_runtime_failed"
    }
}

include!("task_runtime/execution.rs");

fn publish_task_changed(subscriptions: &RuntimeEventRegistry, task_id: &str) {
    subscriptions.publish_task(TaskRuntimeEvent::Changed {
        task_id: task_id.to_string(),
    });
}

fn uuid_fragment() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_providers::{
        ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
    };

    #[test]
    fn background_generation_request_preserves_complete_provider_selection() {
        let selection = ProviderSelectionSnapshot {
            provider_kind: "openai".to_string(),
            provider_account_id: "provider_account:openai:task-owner".to_string(),
            provider_instance_key: Some(
                ProviderInstanceKey::new("provider_instance:openai:task-owner:generation-7")
                    .expect("instance key"),
            ),
            selection_mode: ProviderSelectionMode::ExplicitProfile,
            model_profile: Some("gpt-5.5-task".to_string()),
            reasoning_effort: Some(ReasoningEffort::High),
            selection_source: Some("task:model_pool:complex".to_string()),
        };
        let run = noema_tasks::AgentRunRecord {
            run_id: "run:test".to_string(),
            task_id: "task:test".to_string(),
            run_kind: RunKind::Reviewer,
            agent_id: "agent:reviewer".to_string(),
            revision_index: 2,
            attempt_index: 1,
            parent_run_id: Some("run:parent".to_string()),
            triggering_submission_id: Some("submission:test".to_string()),
            triggering_review_id: None,
            resume_message: None,
            model: selection.clone(),
            actual_provider_kind: None,
            actual_model_profile: None,
            execution_policy: noema_tasks::TaskExecutionPolicy::default(),
            status: RunStatus::Running,
            priority: 10,
            queued_at: String::new(),
            lease_owner: None,
            lease_token: None,
            lease_expires_at: None,
            heartbeat_at: None,
            started_at: None,
            ended_at: None,
            cancellation_requested: false,
            retry_count: 0,
            error_code: None,
            error_message: None,
            provider_call_count: 0,
            tool_call_count: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            active_milliseconds: 0,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let request = background_task_generate_request(
            &run,
            "lease:test",
            &CancellationToken::new(),
            "Do the work.".to_string(),
            "Follow the task contract.",
            &RuntimeEventRegistry::default(),
        );

        assert_eq!(request.provider_selection, selection);
    }
}
