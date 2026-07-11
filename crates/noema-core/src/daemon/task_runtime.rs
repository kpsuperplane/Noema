//! Supervised background executor/reviewer coordinator.

use std::{sync::Arc, time::Duration};

use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::graphql::{ConversationLiveEvent, ConversationSubscriptionRegistry};
use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, CriterionOutcome, GenerateResponseItem,
    NewConversationItem, NewTaskReview, NewTaskSubmission, NoemaStore, ReplayMode, RunKind,
    RunStatus, SubmissionCriterionEvidence, SystemErrorLogger, TaskReviewCriterion,
    TaskReviewVerdict, TaskStatus,
    agent_execution::ExecutionRole,
    daemon::{CodexRuntimeHandle, runtime::BackgroundTaskGenerateRequest},
};

const LEASE_SECONDS: i64 = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(300);

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
    loop {
        if inner.cancellation.is_cancelled() {
            break;
        }
        let lease_token = format!("{}:{}", worker_id, uuid_fragment());
        match store
            .claim_next_agent_run(&worker_id, &lease_token, LEASE_SECONDS)
            .await
        {
            Ok(Some(run)) => {
                if let Err(error) =
                    execute_run(&store, &runtime, &subscriptions, &run, &lease_token).await
                {
                    fail_run(&store, &run, &lease_token, &error).await;
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "task_runtime_worker_error",
                            "Background task run failed",
                        )
                        .with_context(json!({"run_id": run.run_id, "task_id": run.task_id}))
                        .with_error_chain([error]),
                    );
                }
            }
            Ok(None) => {
                tokio::select! {
                    _ = inner.cancellation.cancelled() => break,
                    _ = tokio::time::sleep(POLL_INTERVAL) => {}
                }
            }
            Err(error) => {
                system_errors.try_append(
                    crate::SystemErrorEvent::new(
                        "task_runtime_claim_error",
                        "Background task queue claim failed",
                    )
                    .with_error_chain([error.to_string()]),
                );
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn fail_run(store: &NoemaStore, run: &crate::AgentRunRecord, lease_token: &str, error: &str) {
    let _ = store
        .transition_agent_run(
            &run.run_id,
            RunStatus::Failed,
            Some(lease_token),
            Some(("task_runtime_failed".to_string(), error.to_string())),
        )
        .await;
    if run.run_kind != RunKind::CompletionDelivery
        && let Ok(Some(task)) = store.get_task(&run.task_id).await
        && !task.status.is_terminal()
        && task.status.can_transition_to(TaskStatus::Failed)
    {
        let _ = store
            .transition_task(&task.task_id, TaskStatus::Failed, Some(error))
            .await;
    }
}

async fn execute_run(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &crate::AgentRunRecord,
    lease_token: &str,
) -> Result<(), String> {
    store
        .transition_agent_run(&run.run_id, RunStatus::Running, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    if run.run_kind == RunKind::Executor {
        execute_executor(store, runtime, run, lease_token).await
    } else if run.run_kind == RunKind::Reviewer {
        execute_reviewer(store, runtime, run, lease_token).await
    } else {
        execute_completion_delivery(store, subscriptions, run, lease_token).await
    }
}

async fn execute_completion_delivery(
    store: &NoemaStore,
    subscriptions: &ConversationSubscriptionRegistry,
    run: &crate::AgentRunRecord,
    lease_token: &str,
) -> Result<(), String> {
    let task = store
        .get_task(&run.task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before completion delivery".to_string())?;
    let conversation_id = task
        .source
        .conversation_id
        .clone()
        .ok_or_else(|| "task has no source conversation for completion delivery".to_string())?;
    let submission_id = task
        .final_submission_id
        .clone()
        .ok_or_else(|| "completed task has no final submission".to_string())?;
    let submission = store
        .get_task_submission(&submission_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "final task submission disappeared before delivery".to_string())?;

    let already_delivered = store
        .list_conversation_items(&conversation_id, ReplayMode::Audit)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .any(|item| {
            item.kind == ConversationItemKind::AssistantText
                && item
                    .metadata
                    .get("task_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(task.task_id.as_str())
        });
    if !already_delivered {
        let record = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: task.source.turn_id.clone(),
                parent_item_id: task.source.item_id.clone(),
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
                content_text: Some(submission.result_markdown.clone()),
                payload_json: serde_json::json!({
                    "task_id": task.task_id,
                    "submission_id": submission.submission_id,
                    "delivery_run_id": run.run_id,
                    "source": "background_task_completion"
                }),
                metadata: serde_json::json!({
                    "task_id": task.task_id,
                    "submission_id": submission.submission_id,
                    "delivery_run_id": run.run_id,
                    "source": "background_task_completion"
                }),
            })
            .await
            .map_err(|error| error.to_string())?;
        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(crate::daemon::TurnStreamEvent::ConversationItem {
                conversation_id: record.conversation_id.clone(),
                item_id: record.item_id,
                cursor: Some(record.cursor),
                turn_id: record.turn_id,
                metadata: record.metadata,
                item: Box::new(crate::daemon::TurnTranscriptItem::AssistantText {
                    text: submission.result_markdown.clone(),
                }),
            }),
        });
        subscriptions.publish(ConversationLiveEvent::Completed {
            conversation_id: record.conversation_id,
            client_message_id: None,
        });
    }
    store
        .transition_agent_run(&run.run_id, RunStatus::Completed, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn execute_executor(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    run: &crate::AgentRunRecord,
    lease_token: &str,
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
    }
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let prompt = format_executor_prompt(&task, &criteria, run.revision_index);
    let response = generate_once(
        runtime,
        run,
        prompt,
        "You are Noema's background task executor.",
    )
    .await?;
    store
        .record_agent_run_observation(
            &run.run_id,
            lease_token,
            &response.provider,
            &response.model,
            response.usage.as_ref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let result =
        response_text(response).ok_or_else(|| "executor returned no text result".to_string())?;
    let evidence = criteria
        .iter()
        .map(|criterion| SubmissionCriterionEvidence {
            criterion_id: criterion.criterion_id.clone(),
            evidence_markdown: result.clone(),
        })
        .collect();
    store
        .create_task_submission(NewTaskSubmission {
            submission_id: None,
            task_id: task.task_id.clone(),
            executor_run_id: run.run_id.clone(),
            revision_index: run.revision_index,
            summary: result.chars().take(280).collect(),
            result_markdown: result,
            criteria: evidence,
            artifact_ids: Vec::new(),
        })
        .await
        .map_err(|error| error.to_string())?;
    store
        .transition_agent_run(&run.run_id, RunStatus::Completed, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn execute_reviewer(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    run: &crate::AgentRunRecord,
    lease_token: &str,
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
    let prompt = format_reviewer_prompt(&task, &submission, &criteria);
    let response = generate_once(
        runtime,
        run,
        prompt,
        "You are Noema's adversarial task reviewer. Return only the requested JSON.",
    )
    .await?;
    store
        .record_agent_run_observation(
            &run.run_id,
            lease_token,
            &response.provider,
            &response.model,
            response.usage.as_ref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let text =
        response_text(response).ok_or_else(|| "reviewer returned no text result".to_string())?;
    let parsed = parse_reviewer_response(&text)?;
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
        .create_task_review(NewTaskReview {
            review_id: None,
            task_id: task.task_id,
            reviewer_run_id: run.run_id.clone(),
            reviewed_submission_id: submission_id.to_string(),
            overall_verdict: verdict,
            overall_feedback: parsed.overall_feedback,
            criteria: review_criteria,
        })
        .await
        .map_err(|error| error.to_string())?;
    store
        .transition_agent_run(&run.run_id, RunStatus::Completed, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn generate_once(
    runtime: &CodexRuntimeHandle,
    run: &crate::AgentRunRecord,
    input: String,
    instructions: &str,
) -> Result<crate::GenerateResponse, String> {
    runtime
        .generate_background_task(BackgroundTaskGenerateRequest {
            run_id: run.run_id.clone(),
            agent_id: run.agent_id.clone(),
            role: if run.run_kind == RunKind::Reviewer {
                ExecutionRole::TaskReviewer
            } else {
                ExecutionRole::TaskExecutor
            },
            provider_kind: run.model.provider_kind.clone(),
            model: run.model.model_profile.clone(),
            reasoning_effort: run.model.reasoning_effort,
            input,
            instructions: instructions.to_string(),
        })
        .await
        .map_err(|error| error.to_string())
}

fn response_text(response: crate::GenerateResponse) -> Option<String> {
    let text = response
        .responses
        .into_iter()
        .filter_map(|item| match item {
            GenerateResponseItem::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.trim().is_empty()).then_some(text)
}

fn format_executor_prompt(
    task: &crate::TaskRecord,
    criteria: &[crate::TaskValidationCriterion],
    revision: i64,
) -> String {
    let criteria = criteria
        .iter()
        .map(|criterion| {
            format!(
                "{}. {}{}",
                criterion.ordinal,
                criterion.description,
                criterion
                    .expected_evidence
                    .as_deref()
                    .map(|value| format!(" Evidence: {value}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Task: {}\nRevision: {revision}\n\nRequest:\n{}\n\nValidation criteria:\n{criteria}\n\nProduce a complete, useful Markdown result. Address every criterion explicitly.",
        task.title, task.request_markdown
    )
}

fn format_reviewer_prompt(
    task: &crate::TaskRecord,
    submission: &crate::TaskSubmissionRecord,
    criteria: &[crate::TaskValidationCriterion],
) -> String {
    let criteria = criteria
        .iter()
        .map(|criterion| format!("{}: {}", criterion.criterion_id, criterion.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Original request:\n{}\n\nCriteria:\n{criteria}\n\nExecutor result:\n{}\n\nReturn JSON exactly shaped as {{\"overall_verdict\":\"approve|request_changes|needs_human\",\"overall_feedback\":\"...\",\"criteria\":[{{\"criterion_id\":\"...\",\"outcome\":\"pass|fail|uncertain\",\"evidence_markdown\":\"...\",\"feedback\":\"...\"}}]}}. Be adversarial and include every criterion exactly once.",
        task.request_markdown, submission.result_markdown
    )
}

#[derive(Debug, Deserialize)]
struct ReviewerResponse {
    overall_verdict: String,
    overall_feedback: String,
    criteria: Vec<ReviewerCriterionResponse>,
}

#[derive(Debug, Deserialize)]
struct ReviewerCriterionResponse {
    criterion_id: String,
    outcome: String,
    evidence_markdown: Option<String>,
    feedback: Option<String>,
}

fn parse_reviewer_response(text: &str) -> Result<ReviewerResponse, String> {
    let trimmed = text.trim();
    if let Ok(parsed) = serde_json::from_str::<ReviewerResponse>(trimmed) {
        return Ok(parsed);
    }
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if let Ok(parsed) = serde_json::from_str::<ReviewerResponse>(without_fence) {
        return Ok(parsed);
    }
    let start = trimmed.find('{');
    let end = trimmed.rfind('}');
    if let (Some(start), Some(end)) = (start, end)
        && let Ok(parsed) = serde_json::from_str::<ReviewerResponse>(&trimmed[start..=end])
    {
        return Ok(parsed);
    }
    Err("reviewer JSON was invalid".to_string())
}

fn uuid_fragment() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos:x}")
}
