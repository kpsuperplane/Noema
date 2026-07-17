//! Agent-facing task delegation, inspection, control, and terminal-contract tools.

use serde::Deserialize;
use serde_json::{Value, json};

use noema_capabilities::ToolSpec;
use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot};
use noema_store::NoemaStore;
use noema_tasks::{
    NewTask, NewTaskValidationCriterion, TASK_REVIEWER_AGENT_ID, TaskComplexity,
    TaskModelPoolEntry, TaskSource, TaskStatus,
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
    pub reasoning_effort: Option<noema_providers::ReasoningEffort>,
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

pub(crate) fn task_inspect_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    task_id_tool_spec(
        TASK_INSPECT_TOOL,
        "Inspect the current durable status of a delegated task, including its latest run, submission, review, and safe failure details. Use this instead of guessing whether background work is still queued or running.",
    )
}

pub(crate) fn task_resume_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
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
    )
}

pub(crate) fn task_cancel_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    task_id_tool_spec(
        TASK_CANCEL_TOOL,
        "Cancel a queued, running, or human-blocked delegated task. Cancellation is durable and preserves the task transcript for inspection.",
    )
}

pub(crate) fn task_submit_result_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
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
                    "description": "Exact artifact IDs returned by artifact.create_local_file during this run. Omit this field or use an empty array when no artifact was created; never invent an artifact ID.",
                    "items": {"type": "string", "minLength": 1, "maxLength": 200},
                    "maxItems": 100,
                    "uniqueItems": true
                }
            },
            "required": ["summary", "result_markdown", "criteria"],
            "additionalProperties": false
        }),
    )
}

pub(crate) fn task_submit_review_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_SUBMIT_REVIEW_TOOL,
        "Submit the reviewer's typed verdict and one assessment for every validation criterion.",
        json!({
            "type": "object",
            "properties": {
                "overall_verdict": {
                    "type": "string",
                    "description": "Use approve when every criterion passes; request_changes when any criterion fails; needs_human only when a criterion cannot be decided without human input and is marked uncertain.",
                    "enum": ["approve", "request_changes", "needs_human"]
                },
                "overall_feedback": {"type": "string", "minLength": 1, "maxLength": 20000},
                "criteria": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "criterion_id": {"type": "string", "minLength": 1, "maxLength": 200},
                            "outcome": {
                                "type": "string",
                                "description": "Use pass for demonstrated satisfaction, fail for demonstrated non-satisfaction, and uncertain only when evidence is unavailable or contradictory.",
                                "enum": ["pass", "fail", "uncertain"]
                            },
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
    )
}

pub(crate) fn task_report_blocked_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
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
    )
}

fn task_id_tool_spec(
    name: &str,
    description: &str,
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
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
    )
}

/// Build a task delegation schema containing only currently enabled pool ids.
pub(crate) fn task_delegate_tool_spec(
    entries: &[TaskModelPoolEntry],
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
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
    ToolSpec::new(
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
    )
}

/// Execute a delegation request and atomically create its task/run records.
pub(crate) async fn execute_task_delegate(
    store: &NoemaStore,
    provider_registry: &ProviderRegistry,
    context: &TaskDelegateRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = execute_inner(store, provider_registry, context, call_id.clone(), payload).await;
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
    provider_registry: &ProviderRegistry,
    context: &TaskAccessRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = resume_inner(store, provider_registry, context, payload).await;
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

include!("task_tool/operations.rs");

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
) -> Result<ProviderSelectionSnapshot, String> {
    if let Some(preference) = store
        .get_agent_runtime_preference(TASK_REVIEWER_AGENT_ID)
        .await
        .map_err(|error| error.to_string())?
    {
        return Ok(ProviderSelectionSnapshot::explicit(
            preference.provider_kind,
            preference.provider_account_id,
            preference.model_profile,
            preference.reasoning_effort,
            Some("agent:task-reviewer".to_string()),
        ));
    }
    if let Some(model) = context.model_profile.clone() {
        return Ok(ProviderSelectionSnapshot::explicit(
            context.provider_kind.clone(),
            context.provider_account_id.clone(),
            model,
            context.reasoning_effort,
            Some("primary:effective".to_string()),
        ));
    }
    Ok(ProviderSelectionSnapshot::provider_default(
        context.provider_kind.clone(),
        context.provider_account_id.clone(),
        context.reasoning_effort,
        Some("primary:provider_default".to_string()),
    ))
}

#[cfg(test)]
#[path = "task_tool/tests.rs"]
mod tests;
