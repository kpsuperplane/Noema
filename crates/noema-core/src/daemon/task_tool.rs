//! Primary-agent task delegation tool.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ModelConfigSnapshot, NewTask, NewTaskValidationCriterion, NoemaStore, TASK_REVIEWER_AGENT_ID,
    TaskComplexity, TaskModelPoolEntry, TaskSource, TaskStatus, provider::NoemaToolSpec,
};

pub(crate) const TASK_DELEGATE_TOOL: &str = "task.delegate";

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

/// Return whether a model call is the primary delegation tool.
#[must_use]
pub(crate) fn is_task_delegate_tool(name: &str) -> bool {
    name == TASK_DELEGATE_TOOL
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
