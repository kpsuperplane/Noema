//! Agent-facing task delegation, inspection, control, and terminal-contract tools.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ModelConfigSnapshot, NewTask, NewTaskValidationCriterion, NoemaStore, TASK_REVIEWER_AGENT_ID,
    TaskComplexity, TaskModelPoolEntry, TaskSource, TaskStatus, provider::NoemaToolSpec,
};

pub(crate) const TASK_DELEGATE_TOOL: &str = "task.delegate";
pub(crate) const TASK_INSPECT_TOOL: &str = "task.inspect";
pub(crate) const TASK_RESUME_TOOL: &str = "task.resume";
pub(crate) const TASK_CANCEL_TOOL: &str = "task.cancel";
pub(crate) const TASK_SUBMIT_RESULT_TOOL: &str = "task.submit_result";
pub(crate) const TASK_SUBMIT_REVIEW_TOOL: &str = "task.submit_review";
pub(crate) const TASK_REPORT_BLOCKED_TOOL: &str = "task.report_blocked";

/// Owner scope applied to task inspection and control calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskAccessRuntimeContext {
    pub owner_human_id: String,
    pub actor_id: String,
}

/// Trusted source context for one primary-agent delegation call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskDelegateRuntimeContext {
    /// Source conversation id.
    pub conversation_id: String,
    /// Source turn id.
    pub turn_id: String,
    /// Source user transcript item id.
    pub user_item_id: String,
    /// Creating agent id.
    pub agent_id: String,
    /// Effective source provider kind.
    pub provider_kind: String,
    /// Effective source provider account id.
    pub provider_account_id: String,
    /// Effective source model profile, when explicit.
    pub model_profile: Option<String>,
    /// Effective source reasoning effort.
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
}

/// Result returned from the task delegation tool.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaskToolResult {
    /// Provider call id.
    pub call_id: Option<String>,
    /// Tool name.
    pub name: String,
    /// Whether creation succeeded.
    pub success: bool,
    /// Safe structured payload.
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegateArguments {
    title: String,
    request: String,
    complexity: TaskComplexity,
    executor_model_pool_entry_id: String,
    validation_criteria: Vec<DelegateCriterion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegateCriterion {
    description: String,
    #[serde(default)]
    evidence_required: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskIdArguments {
    task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeArguments {
    task_id: String,
    #[serde(default)]
    message: Option<String>,
}

/// Return whether a model call is the primary delegation tool.
#[must_use]
pub(crate) fn is_task_delegate_tool(name: &str) -> bool {
    name == TASK_DELEGATE_TOOL
}

#[must_use]
pub(crate) fn is_task_inspect_tool(name: &str) -> bool {
    name == TASK_INSPECT_TOOL
}

#[must_use]
pub(crate) fn is_task_resume_tool(name: &str) -> bool {
    name == TASK_RESUME_TOOL
}

#[must_use]
pub(crate) fn is_task_cancel_tool(name: &str) -> bool {
    name == TASK_CANCEL_TOOL
}

#[must_use]
pub(crate) fn is_task_submit_result_tool(name: &str) -> bool {
    name == TASK_SUBMIT_RESULT_TOOL
}

#[must_use]
pub(crate) fn is_task_submit_review_tool(name: &str) -> bool {
    name == TASK_SUBMIT_REVIEW_TOOL
}

#[must_use]
pub(crate) fn is_task_report_blocked_tool(name: &str) -> bool {
    name == TASK_REPORT_BLOCKED_TOOL
}

pub(crate) fn task_inspect_tool_spec() -> Result<NoemaToolSpec, crate::provider::ToolContractError>
{
    task_id_tool_spec(
        TASK_INSPECT_TOOL,
        "Inspect the current durable status of a delegated task, including its latest run, submission, review, and safe failure details. Use this instead of guessing whether background work is still queued or running.",
    )
}

pub(crate) fn task_resume_tool_spec() -> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
        TASK_RESUME_TOOL,
        "Continue a failed or human-blocked delegated task from its durable transcript and evidence. Supply the human's answer in message when the task is waiting for human input.",
        json!({
            "type": "object",
            "properties": {
                "task_id": {"type": "string", "minLength": 1, "maxLength": 200},
                "message": {"type": "string", "minLength": 1, "maxLength": 20000}
            },
            "required": ["task_id"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

pub(crate) fn task_cancel_tool_spec() -> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    task_id_tool_spec(
        TASK_CANCEL_TOOL,
        "Cancel a queued, running, or human-blocked delegated task. Cancellation is durable and preserves the task transcript for inspection.",
    )
}

pub(crate) fn task_submit_result_tool_spec()
-> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
        TASK_SUBMIT_RESULT_TOOL,
        "Submit the executor's final result and evidence. Use exactly once when the delegated task can be completed from the available evidence.",
        json!({
            "type": "object",
            "properties": {
                "summary": {"type": "string", "minLength": 1, "maxLength": 4000},
                "result_markdown": {"type": "string", "minLength": 1, "maxLength": 100000},
                "criteria": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "criterion_id": {"type": "string", "minLength": 1, "maxLength": 200},
                            "evidence_markdown": {"type": "string", "minLength": 1, "maxLength": 20000}
                        },
                        "required": ["criterion_id", "evidence_markdown"],
                        "additionalProperties": false
                    }
                },
                "artifact_ids": {
                    "type": "array",
                    "items": {"type": "string", "minLength": 1, "maxLength": 200},
                    "maxItems": 100,
                    "uniqueItems": true
                }
            },
            "required": ["summary", "result_markdown", "criteria", "artifact_ids"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

pub(crate) fn task_submit_review_tool_spec()
-> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
        TASK_SUBMIT_REVIEW_TOOL,
        "Submit the reviewer's typed verdict and one assessment for every validation criterion.",
        json!({
            "type": "object",
            "properties": {
                "overall_verdict": {"type": "string", "enum": ["approve", "request_changes", "needs_human"]},
                "overall_feedback": {"type": "string", "minLength": 1, "maxLength": 20000},
                "criteria": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "criterion_id": {"type": "string", "minLength": 1, "maxLength": 200},
                            "outcome": {"type": "string", "enum": ["pass", "fail", "uncertain"]},
                            "evidence_markdown": {"type": "string", "maxLength": 20000},
                            "feedback": {"type": "string", "maxLength": 20000}
                        },
                        "required": ["criterion_id", "outcome"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["overall_verdict", "overall_feedback", "criteria"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

pub(crate) fn task_report_blocked_tool_spec()
-> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
        TASK_REPORT_BLOCKED_TOOL,
        "Stop execution and ask the task owner one blocking question when safe progress requires human input.",
        json!({
            "type": "object",
            "properties": {
                "question": {"type": "string", "minLength": 1, "maxLength": 4000},
                "work_summary": {"type": "string", "minLength": 1, "maxLength": 20000},
                "resume_context": {"type": "string", "maxLength": 20000}
            },
            "required": ["question", "work_summary"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

fn task_id_tool_spec(
    name: &str,
    description: &str,
) -> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
        name,
        description,
        json!({
            "type": "object",
            "properties": {
                "task_id": {"type": "string", "minLength": 1, "maxLength": 200}
            },
            "required": ["task_id"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

/// Build a task delegation schema containing only currently enabled pool ids.
pub(crate) fn task_delegate_tool_spec(
    entries: &[TaskModelPoolEntry],
) -> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    let pool_ids = entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.pool_entry_id.clone())
        .collect::<Vec<_>>();
    let options = entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| {
            format!(
                "{}={} ({}, {})",
                entry.pool_entry_id,
                entry.label.as_deref().unwrap_or(
                    entry
                        .model
                        .model_profile
                        .as_deref()
                        .unwrap_or("provider default")
                ),
                entry.complexity,
                entry.model.provider_kind,
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    NoemaToolSpec::new(
        TASK_DELEGATE_TOOL,
        format!(
            "Delegate a bounded one-off request to a background executor. Use only when the work is independently verifiable and does not need an immediate human exchange. Available executor models: {options}"
        ),
        json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "minLength": 1, "maxLength": 200},
                "request": {"type": "string", "minLength": 1, "maxLength": 20000},
                "complexity": {"type": "string", "enum": ["simple", "medium", "difficult"]},
                "executor_model_pool_entry_id": {"type": "string", "enum": pool_ids},
                "validation_criteria": {
                    "type": "array", "minItems": 1, "maxItems": 12,
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {"type": "string", "minLength": 1, "maxLength": 2000},
                            "evidence_required": {"type": "string", "maxLength": 2000}
                        },
                        "required": ["description"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["title", "request", "complexity", "executor_model_pool_entry_id", "validation_criteria"],
            "additionalProperties": false
        }),
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

/// Execute a delegation request and atomically create its task/run records.
pub(crate) async fn execute_task_delegate(
    store: &NoemaStore,
    context: &TaskDelegateRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = execute_inner(store, context, call_id.clone(), payload).await;
    match result {
        Ok(payload) => TaskToolResult {
            call_id,
            name: TASK_DELEGATE_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => TaskToolResult {
            call_id,
            name: TASK_DELEGATE_TOOL.to_string(),
            success: false,
            payload: json!({"error": error}),
        },
    }
}

pub(crate) async fn execute_task_inspect(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = inspect_inner(store, context, payload).await;
    task_tool_result(TASK_INSPECT_TOOL, call_id, result)
}

pub(crate) async fn execute_task_resume(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = resume_inner(store, context, payload).await;
    task_tool_result(TASK_RESUME_TOOL, call_id, result)
}

pub(crate) async fn execute_task_cancel(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = cancel_inner(store, context, payload).await;
    task_tool_result(TASK_CANCEL_TOOL, call_id, result)
}

fn task_tool_result(
    name: &str,
    call_id: Option<String>,
    result: Result<Value, String>,
) -> TaskToolResult {
    match result {
        Ok(payload) => TaskToolResult {
            call_id,
            name: name.to_string(),
            success: true,
            payload,
        },
        Err(error) => TaskToolResult {
            call_id,
            name: name.to_string(),
            success: false,
            payload: json!({"error": error}),
        },
    }
}

async fn inspect_inner(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = task_id_arguments(payload)?;
    let task = store
        .get_task(arguments.task_id.trim())
        .await
        .map_err(|error| error.to_string())?
        .filter(|task| task.owner_human_id == context.owner_human_id)
        .ok_or_else(|| "task is unavailable".to_string())?;
    let runs = store
        .list_agent_runs_for_task(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let submissions = store
        .list_task_submissions(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let reviews = store
        .list_task_reviews(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let latest_items = if let Some(run) = runs.last() {
        store
            .list_recent_agent_run_items(&run.run_id, 50)
            .await
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let transcript_cursor = latest_items
        .last()
        .map(|item| item.sequence_index.to_string());
    let recent_items = latest_items
        .iter()
        .map(|item| {
            json!({
                "item_id": item.item_id,
                "cursor": item.sequence_index.to_string(),
                "round": item.round_index,
                "kind": item.kind,
                "status": item.status.as_str(),
                "correlation_id": item.correlation_id,
                "parent_item_id": item.parent_item_id,
                "content_text": item.content_text,
                "payload": item.payload,
                "created_at": item.created_at,
                "updated_at": item.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let latest_run = runs.last().map(|run| {
        json!({
            "run_id": run.run_id,
            "kind": run.run_kind.as_str(),
            "status": run.status.as_str(),
            "revision": run.revision_index,
            "attempt": run.attempt_index,
            "model": run.actual_model_profile.as_ref().or(run.model.model_profile.as_ref()),
            "queued_at": run.queued_at,
            "started_at": run.started_at,
            "ended_at": run.ended_at,
            "error_code": run.error_code,
            "error_message": run.error_message,
        })
    });
    let latest_submission = submissions.last().map(|submission| {
        json!({
            "submission_id": submission.submission_id,
            "revision": submission.revision_index,
            "summary": submission.summary,
            "created_at": submission.created_at,
        })
    });
    let latest_review = reviews.last().map(|review| {
        json!({
            "review_id": review.review_id,
            "verdict": review.overall_verdict.as_str(),
            "feedback": review.overall_feedback,
            "created_at": review.created_at,
        })
    });
    let last_activity_at = latest_items
        .last()
        .map(|item| item.updated_at.as_str())
        .or_else(|| runs.last().map(|run| run.updated_at.as_str()))
        .unwrap_or(task.updated_at.as_str());
    let can_resume = matches!(
        task.status,
        TaskStatus::Failed | TaskStatus::WaitingForHuman
    );
    let can_cancel = matches!(
        task.status,
        TaskStatus::Queued
            | TaskStatus::Executing
            | TaskStatus::Reviewing
            | TaskStatus::RevisionRequested
            | TaskStatus::WaitingForHuman
    );
    let current_phase = runs.last().map_or_else(
        || task.status.as_str().to_string(),
        |run| format!("{}.{}", run.run_kind.as_str(), run.status.as_str()),
    );
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "current_phase": current_phase,
        "complexity": task.complexity.as_str(),
        "revision": task.revision_index,
        "max_review_rounds": task.max_review_rounds,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
        "last_activity_at": last_activity_at,
        "completed_at": task.completed_at,
        "can_resume": can_resume,
        "can_cancel": can_cancel,
        "blocking_question": task.blocked_question,
        "terminal_reason": task.terminal_reason,
        "error_code": task.error_code,
        "error_message": task.error_message,
        "policy_consumption": {
            "provider_calls": runs.last().map_or(0, |run| run.provider_call_count),
            "tool_calls": runs.last().map_or(0, |run| run.tool_call_count),
            "active_milliseconds": runs.last().map_or(0, |run| run.active_milliseconds),
            "input_tokens": runs.last().map_or(0, |run| run.input_tokens),
            "cached_input_tokens": runs.last().map_or(0, |run| run.cached_input_tokens),
            "output_tokens": runs.last().map_or(0, |run| run.output_tokens),
            "limits": runs.last().map(|run| json!({
                "provider_continuations": run.execution_policy.max_provider_continuations,
                "tool_calls": run.execution_policy.max_tool_calls,
                "active_minutes": run.execution_policy.max_active_minutes,
                "progress_audit_interval": run.execution_policy.progress_audit_interval,
            })),
        },
        "transcript_cursor": transcript_cursor,
        "recent_items": recent_items,
        "latest_run": latest_run,
        "latest_submission": latest_submission,
        "latest_review": latest_review,
    }))
}

async fn resume_inner(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = resume_arguments(payload)?;
    let (task, run) = store
        .resume_task(
            arguments.task_id.trim(),
            &context.owner_human_id,
            &context.actor_id,
            arguments.message.as_deref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "run_id": run.run_id,
        "run_kind": run.run_kind.as_str(),
        "attempt": run.attempt_index,
        "revision": run.revision_index,
        "model": run.model.model_profile,
    }))
}

async fn cancel_inner(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = task_id_arguments(payload)?;
    let task = store
        .cancel_task(
            arguments.task_id.trim(),
            &context.owner_human_id,
            &context.actor_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "cancelled": task.status == TaskStatus::Cancelled,
    }))
}

fn task_id_arguments(payload: &Value) -> Result<TaskIdArguments, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    serde_json::from_value(arguments).map_err(|error| format!("invalid task arguments: {error}"))
}

fn resume_arguments(payload: &Value) -> Result<ResumeArguments, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    serde_json::from_value(arguments).map_err(|error| format!("invalid task arguments: {error}"))
}

async fn execute_inner(
    store: &NoemaStore,
    context: &TaskDelegateRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    let arguments: DelegateArguments = serde_json::from_value(arguments)
        .map_err(|error| format!("invalid task delegation arguments: {error}"))?;
    let pool = store
        .select_task_model_pool_entry(
            arguments.complexity,
            arguments.executor_model_pool_entry_id.trim(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let reviewer = reviewer_model_snapshot(store, context).await?;
    let criteria = arguments
        .validation_criteria
        .into_iter()
        .enumerate()
        .map(|(index, criterion)| NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
            description: criterion.description,
            expected_evidence: criterion.evidence_required,
        })
        .collect();
    let (task, run) = store
        .create_task_with_executor(NewTask {
            task_id: None,
            title: arguments.title,
            request_markdown: arguments.request,
            complexity: arguments.complexity,
            owner_human_id: "human:local".to_string(),
            source: TaskSource {
                conversation_id: Some(context.conversation_id.clone()),
                turn_id: Some(context.turn_id.clone()),
                item_id: Some(context.user_item_id.clone()),
            },
            created_by_agent_id: context.agent_id.clone(),
            creation_tool_call_id: call_id.or_else(|| call_id_from_payload(payload)),
            pool_entry_id: pool.pool_entry_id.clone(),
            executor_model: pool.model.clone(),
            reviewer_model: reviewer,
            max_review_rounds: None,
            criteria,
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": TaskStatus::Queued.as_str(),
        "complexity": task.complexity.as_str(),
        "executor_model_pool_entry_id": pool.pool_entry_id,
        "executor_model": task.executor_model.model_profile,
        "reviewer_model": task.reviewer_model.model_profile,
        "executor_run_id": run.run_id,
    }))
}

fn call_id_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("call_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

async fn reviewer_model_snapshot(
    store: &NoemaStore,
    context: &TaskDelegateRuntimeContext,
) -> Result<ModelConfigSnapshot, String> {
    if let Some(preference) = store
        .get_agent_runtime_preference(TASK_REVIEWER_AGENT_ID)
        .await
        .map_err(|error| error.to_string())?
    {
        return Ok(ModelConfigSnapshot::explicit(
            preference.provider_kind,
            preference.provider_account_id,
            preference.model_profile,
            preference.reasoning_effort,
            Some("agent:task-reviewer".to_string()),
        ));
    }
    if let Some(model) = context.model_profile.clone() {
        return Ok(ModelConfigSnapshot::explicit(
            context.provider_kind.clone(),
            context.provider_account_id.clone(),
            model,
            context.reasoning_effort,
            Some("primary:effective".to_string()),
        ));
    }
    Ok(ModelConfigSnapshot::provider_default(
        context.provider_kind.clone(),
        context.provider_account_id.clone(),
        context.reasoning_effort,
        Some("primary:provider_default".to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_contract_specs_are_typed_and_role_specific() {
        let result = task_submit_result_tool_spec().expect("result spec");
        assert_eq!(result.name.as_str(), TASK_SUBMIT_RESULT_TOOL);
        assert_eq!(
            result.input_schema.as_value()["required"],
            json!(["summary", "result_markdown", "criteria", "artifact_ids"])
        );

        let review = task_submit_review_tool_spec().expect("review spec");
        assert_eq!(review.name.as_str(), TASK_SUBMIT_REVIEW_TOOL);
        assert_eq!(
            review.input_schema.as_value()["properties"]["overall_verdict"]["enum"],
            json!(["approve", "request_changes", "needs_human"])
        );

        let blocked = task_report_blocked_tool_spec().expect("blocked spec");
        assert_eq!(blocked.name.as_str(), TASK_REPORT_BLOCKED_TOOL);
        assert_eq!(
            blocked.input_schema.as_value()["required"],
            json!(["question", "work_summary"])
        );
    }

    #[tokio::test]
    async fn inspect_and_resume_tools_use_canonical_task_state() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider account");
        let pool = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple task model");
        let (task, run) = store
            .create_task_with_executor(NewTask {
                task_id: None,
                title: "Inspectable task".to_string(),
                request_markdown: "Inspect me".to_string(),
                complexity: TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Completes".to_string(),
                    expected_evidence: None,
                }],
            })
            .await
            .expect("task");
        let context = TaskAccessRuntimeContext {
            owner_human_id: "human:local".to_string(),
            actor_id: "agent:primary".to_string(),
        };
        let arguments = json!({"task_id": task.task_id});

        let queued = execute_task_inspect(&store, &context, None, &arguments).await;
        assert!(queued.success);
        assert_eq!(queued.payload["status"], "queued");
        assert_eq!(queued.payload["latest_run"]["status"], "queued");
        assert_eq!(queued.payload["can_resume"], false);
        assert_eq!(queued.payload["can_cancel"], true);

        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Failed,
                None,
                Some(("provider_error".to_string(), "model missing".to_string())),
            )
            .await
            .expect("failed run");
        let failed = execute_task_inspect(&store, &context, None, &arguments).await;
        assert_eq!(failed.payload["status"], "failed");
        assert_eq!(failed.payload["can_resume"], true);
        assert_eq!(
            failed.payload["latest_run"]["error_message"],
            "model missing"
        );

        let resumed = execute_task_resume(&store, &context, None, &arguments).await;
        assert!(resumed.success);
        assert_eq!(resumed.payload["status"], "queued");
        assert_eq!(resumed.payload["attempt"], 1);

        let cancelled = execute_task_cancel(&store, &context, None, &arguments).await;
        assert!(cancelled.success);
        assert_eq!(cancelled.payload["status"], "cancelled");
    }
}
