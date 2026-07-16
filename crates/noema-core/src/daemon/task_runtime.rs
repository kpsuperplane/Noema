//! Supervised background executor/reviewer coordinator.

use std::{sync::Arc, time::Duration};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use noema_tasks::{
    CriterionOutcome, NewTaskReview, NewTaskSubmission, RunKind, RunStatus,
    SubmissionCriterionEvidence, TaskReviewCriterion, TaskReviewVerdict, TaskStatus,
};

use crate::graphql::{ConversationSubscriptionRegistry, TaskLiveEvent};
use crate::{
    NoemaStore,
    agent_execution::ExecutionRole,
    daemon::{
        CodexRuntimeHandle,
        runtime::BackgroundTaskGenerateRequest,
        task_run_context::{
            ExecutorBlockedResponse, ExecutorSubmissionResponse, ReviewerResponse,
            format_executor_prompt, format_reviewer_prompt,
            human_continuation_context_for_submission, with_resume_context,
        },
        task_tool::{TASK_REPORT_BLOCKED_TOOL, TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL},
    },
};
use noema_home::{SystemErrorEvent, SystemErrorLogger};

const LEASE_SECONDS: i64 = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(300);
const MAX_CONCURRENT_TASK_RUNS: usize = 8;

/// Runtime handle for supervised task workers.
#[derive(Clone)]
pub(crate) struct TaskRuntimeHandle {
    inner: Arc<TaskRuntimeInner>,
}

struct TaskRuntimeInner {
    cancellation: CancellationToken,
    join: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
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
    pub(crate) fn start(
        store: NoemaStore,
        runtime: CodexRuntimeHandle,
        system_errors: SystemErrorLogger,
        subscriptions: ConversationSubscriptionRegistry,
    ) -> Self {
        let inner = Arc::new(TaskRuntimeInner {
            cancellation: CancellationToken::new(),
            join: std::sync::Mutex::new(None),
        });
        let worker_inner = Arc::clone(&inner);
        let join = tokio::spawn(async move {
            run_loop(store, runtime, system_errors, subscriptions, worker_inner).await;
        });
        *inner.join.lock().expect("task runtime join lock") = Some(join);
        Self { inner }
    }

    /// Request cancellation and join the worker loop.
    pub(crate) async fn shutdown(&self) {
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

async fn run_loop(
    store: NoemaStore,
    runtime: CodexRuntimeHandle,
    system_errors: SystemErrorLogger,
    subscriptions: ConversationSubscriptionRegistry,
    inner: Arc<TaskRuntimeInner>,
) {
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
            match store
                .claim_next_agent_run(&worker_id, &lease_token, LEASE_SECONDS)
                .await
            {
                Ok(Some(run)) => {
                    let run_store = store.clone();
                    let run_runtime = runtime.clone();
                    let run_subscriptions = subscriptions.clone();
                    let run_system_errors = system_errors.clone();
                    let shutdown = inner.cancellation.clone();
                    active_runs.spawn(async move {
                        supervise_claimed_run(
                            run_store,
                            run_runtime,
                            run_subscriptions,
                            run_system_errors,
                            run,
                            lease_token,
                            shutdown,
                        )
                        .await;
                    });
                }
                Ok(None) => queue_available = false,
                Err(error) => {
                    system_errors.try_append(
                        SystemErrorEvent::new(
                            "task_runtime_claim_error",
                            "Background task queue could not be read",
                        )
                        .with_error_chain([error.to_string()]),
                    );
                    queue_available = false;
                }
            }
        }

        drain_task_status_outbox(&store, &runtime, &subscriptions, &system_errors).await;
        tokio::select! {
            _ = inner.cancellation.cancelled() => break,
            completed = active_runs.join_next(), if !active_runs.is_empty() => {
                if let Some(Err(error)) = completed {
                    log_task_run_join_error(&system_errors, &error);
                }
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }

    while let Some(result) = active_runs.join_next().await {
        if let Err(error) = result {
            log_task_run_join_error(&system_errors, &error);
        }
    }
}

async fn supervise_claimed_run(
    store: NoemaStore,
    runtime: CodexRuntimeHandle,
    subscriptions: ConversationSubscriptionRegistry,
    system_errors: SystemErrorLogger,
    run: noema_tasks::AgentRunRecord,
    lease_token: String,
    shutdown: CancellationToken,
) {
    publish_task_changed(&subscriptions, &run.task_id);
    let run_cancellation = CancellationToken::new();
    if let Err(error) = supervise_run(
        &store,
        &runtime,
        &subscriptions,
        &run,
        &lease_token,
        &run_cancellation,
        &shutdown,
    )
    .await
    {
        fail_run(&store, &subscriptions, &run, &lease_token, &error).await;
        publish_task_changed(&subscriptions, &run.task_id);
        system_errors.try_append(
            SystemErrorEvent::new("task_runtime_worker_error", "Background task run failed")
                .with_context(json!({"run_id": run.run_id, "task_id": run.task_id}))
                .with_error_chain([error]),
        );
    } else {
        publish_task_changed(&subscriptions, &run.task_id);
    }
}

fn log_task_run_join_error(system_errors: &SystemErrorLogger, error: &tokio::task::JoinError) {
    system_errors.try_append(
        SystemErrorEvent::new(
            "task_runtime_worker_join_error",
            "Background task worker stopped unexpectedly",
        )
        .with_error_chain([error.to_string()]),
    );
}

async fn drain_task_status_outbox(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    system_errors: &SystemErrorLogger,
) {
    let task_ids = match store.list_pending_task_status_deliveries(32).await {
        Ok(task_ids) => task_ids,
        Err(error) => {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "task_status_outbox_read_failed",
                    "Task status delivery queue could not be read",
                )
                .with_error_chain([error.to_string()]),
            );
            return;
        }
    };
    for task_id in task_ids {
        if let Err(error) =
            crate::daemon::task_delivery::deliver_task_status_event(store, subscriptions, &task_id)
                .await
        {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "task_status_delivery_failed",
                    "Task status update could not be delivered",
                )
                .with_context(json!({"task_id": task_id}))
                .with_error_chain([error]),
            );
        }
    }

    crate::daemon::task_delivery::drain_task_completion_outbox(store, runtime, system_errors).await;
}

#[allow(clippy::too_many_arguments)]
async fn supervise_run(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    run_cancellation: &CancellationToken,
    shutdown: &CancellationToken,
) -> Result<(), String> {
    let execution = execute_run(
        store,
        runtime,
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
    subscriptions: &ConversationSubscriptionRegistry,
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

async fn execute_run(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    store
        .transition_agent_run(&run.run_id, RunStatus::Running, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &run.task_id);
    match run.run_kind {
        RunKind::Executor => {
            execute_executor(
                store,
                runtime,
                subscriptions,
                run,
                lease_token,
                cancellation,
            )
            .await
        }
        RunKind::Reviewer => {
            execute_reviewer(
                store,
                runtime,
                subscriptions,
                run,
                lease_token,
                cancellation,
            )
            .await
        }
    }
}

async fn execute_executor(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    let task = store
        .get_task(&run.task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before executor run".to_string())?;
    if task.status == TaskStatus::Queued || task.status == TaskStatus::RevisionRequested {
        store
            .transition_task(
                &task.task_id,
                TaskStatus::Executing,
                Some("executor_started"),
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_task_changed(subscriptions, &task.task_id);
    }
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let prompt = with_resume_context(
        store,
        run,
        format_executor_prompt(&task, &criteria, run.revision_index),
    )
    .await?;
    let response = generate_once(
        runtime,
        run,
        lease_token,
        cancellation,
        prompt,
        "You are Noema's background task executor. Work autonomously with the role-approved tools. When finished, call task.submit_result exactly once. If safe progress genuinely requires human input, call task.report_blocked exactly once. Do not return the task result as ordinary assistant text.",
        subscriptions,
    )
    .await?;
    if let Some(call) = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_REPORT_BLOCKED_TOOL)
    {
        let blocked: ExecutorBlockedResponse = serde_json::from_value(call.payload.clone())
            .map_err(|error| format!("invalid executor blocked contract: {error}"))?;
        let blocked_context = match blocked.resume_context {
            Some(resume_context) if !resume_context.trim().is_empty() => format!(
                "{}\n\nResume context:\n{}",
                blocked.work_summary.trim(),
                resume_context.trim()
            ),
            _ => blocked.work_summary.trim().to_string(),
        };
        store
            .report_task_blocked(
                &task.task_id,
                &run.run_id,
                lease_token,
                blocked.question.trim(),
                &blocked_context,
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_task_changed(subscriptions, &task.task_id);
        let _ = crate::daemon::task_delivery::deliver_task_status_event(
            store,
            subscriptions,
            &task.task_id,
        )
        .await;
        return Ok(());
    }
    let call = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_SUBMIT_RESULT_TOOL)
        .ok_or_else(|| "executor terminal contract missing".to_string())?;
    let result: ExecutorSubmissionResponse = serde_json::from_value(call.payload.clone())
        .map_err(|error| format!("invalid executor submission contract: {error}"))?;
    let evidence = result
        .criteria
        .into_iter()
        .map(|criterion| SubmissionCriterionEvidence {
            criterion_id: criterion.criterion_id,
            evidence_markdown: criterion.evidence_markdown,
        })
        .collect();
    store
        .create_task_submission(
            NewTaskSubmission {
                submission_id: None,
                task_id: task.task_id.clone(),
                executor_run_id: run.run_id.clone(),
                revision_index: run.revision_index,
                summary: result.summary,
                result_markdown: result.result_markdown,
                criteria: evidence,
                artifact_ids: result.artifact_ids,
            },
            lease_token,
        )
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &task.task_id);
    let _ = crate::daemon::task_delivery::deliver_task_status_event(
        store,
        subscriptions,
        &task.task_id,
    )
    .await;
    Ok(())
}

async fn execute_reviewer(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    let task = store
        .get_task(&run.task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before reviewer run".to_string())?;
    let submission_id = run
        .triggering_submission_id
        .as_deref()
        .ok_or_else(|| "reviewer run has no submission".to_string())?;
    let submission = store
        .get_task_submission(submission_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "submission disappeared before review".to_string())?;
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let human_context = human_continuation_context_for_submission(store, &submission).await?;
    let prompt = with_resume_context(
        store,
        run,
        format_reviewer_prompt(&task, &submission, &criteria, &human_context),
    )
    .await?;
    let response = generate_once(
        runtime,
        run,
        lease_token,
        cancellation,
        prompt,
        "You are Noema's adversarial task reviewer. Inspect the submission and call task.submit_review exactly once with the typed verdict. Do not return review JSON as ordinary assistant text.",
        subscriptions,
    )
    .await?;
    let call = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_SUBMIT_REVIEW_TOOL)
        .ok_or_else(|| "reviewer terminal contract missing".to_string())?;
    let parsed: ReviewerResponse = serde_json::from_value(call.payload.clone())
        .map_err(|error| format!("invalid reviewer contract: {error}"))?;
    let verdict = parsed
        .overall_verdict
        .parse::<TaskReviewVerdict>()
        .map_err(|_| "reviewer verdict was invalid".to_string())?;
    let review_criteria = parsed
        .criteria
        .into_iter()
        .map(|criterion| {
            Ok(TaskReviewCriterion {
                criterion_id: criterion.criterion_id,
                outcome: criterion
                    .outcome
                    .parse::<CriterionOutcome>()
                    .map_err(|_| "review criterion outcome was invalid")?,
                evidence_markdown: criterion.evidence_markdown,
                feedback: criterion.feedback,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    store
        .create_task_review(
            NewTaskReview {
                review_id: None,
                task_id: task.task_id.clone(),
                reviewer_run_id: run.run_id.clone(),
                reviewed_submission_id: submission_id.to_string(),
                overall_verdict: verdict,
                overall_feedback: parsed.overall_feedback,
                criteria: review_criteria,
            },
            lease_token,
        )
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &task.task_id);
    let _ = crate::daemon::task_delivery::deliver_task_status_event(
        store,
        subscriptions,
        &task.task_id,
    )
    .await;
    Ok(())
}

async fn generate_once(
    runtime: &CodexRuntimeHandle,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
    input: String,
    instructions: &str,
    subscriptions: &ConversationSubscriptionRegistry,
) -> Result<noema_providers::GenerateResponse, String> {
    let request = background_task_generate_request(
        run,
        lease_token,
        cancellation,
        input,
        instructions,
        subscriptions,
    );
    runtime
        .generate_background_task(request)
        .await
        .map_err(|error| error.to_string())
}

fn background_task_generate_request(
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
    input: String,
    instructions: &str,
    subscriptions: &ConversationSubscriptionRegistry,
) -> BackgroundTaskGenerateRequest {
    BackgroundTaskGenerateRequest {
        run_id: run.run_id.clone(),
        task_id: run.task_id.clone(),
        lease_token: lease_token.to_string(),
        cancellation: cancellation.clone(),
        agent_id: run.agent_id.clone(),
        role: if run.run_kind == RunKind::Reviewer {
            ExecutionRole::TaskReviewer
        } else {
            ExecutionRole::TaskExecutor
        },
        provider_selection: run.model.clone(),
        execution_policy: run.execution_policy,
        input,
        instructions: instructions.to_string(),
        task_subscriptions: subscriptions.clone(),
    }
}

fn publish_task_changed(subscriptions: &ConversationSubscriptionRegistry, task_id: &str) {
    subscriptions.publish_task(TaskLiveEvent::Changed {
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
#[path = "task_runtime_tests.rs"]
mod tests;
