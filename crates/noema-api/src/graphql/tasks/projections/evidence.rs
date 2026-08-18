use async_graphql::Json;
use noema_tasks::{AgentRunItemRecord, AgentRunRecord, TaskGateRecord};

use super::*;

graphql_object_from! { "Task gate category." => pub struct GraphqlTaskGate("TaskGate")
    try_from TaskGateRecord as value {
    "Gate identity." => gate_id: String = value.gate_id.into_string(),
    "Task generation." => task_generation: i64 = exact_u64(value.task_generation)?,
    "Gate kind." => kind: GraphqlTaskGateKind = value.kind.into(),
    "Gate state." => state: GraphqlTaskGateState = value.state.into(),
    "Recovery reason, when any." => recovery_reason: Option<GraphqlTaskRecoveryReason> = value.recovery_reason.map(Into::into),
    "Explicit recovery continuation role, when any." => retry_run_kind: Option<GraphqlTaskRunKind> = value.retry_run_kind.map(Into::into),
    "Human prompt." => prompt: String = value.prompt_markdown,
    "Bounded context." => context_markdown: String = value.context_markdown,
    "Optional direct answers." => suggested_answers: Vec<String> = value.suggested_answers,
    "Opener actor." => opened_by: String = value.opened_by_actor_id,
    "Opening run, when any." => originating_run_id: Option<String> = value.originating_run_id,
    "Open timestamp." => opened_at: String = value.opened_at,
    "Resolver actor, when resolved." => resolved_by: Option<String> = value.resolved_by_actor_id,
    "Resolution timestamp, when resolved." => resolved_at: Option<String> = value.resolved_at,
    "Resolution message identity, when resolved." => resolution: Option<String> = value.resolution_message_id.map(|id| id.into_string()),
} }

graphql_object_from! { "Human message in Task history." => pub struct GraphqlTaskMessage("TaskMessage")
    from noema_tasks::TaskMessageRecord as value {
    "Message identity." => message_id: String = value.message_id.into_string(),
    "Safe Markdown body." => body_markdown: String = value.body_markdown,
    "Author actor." => author: String = value.author_actor_id,
    "Creation timestamp." => created_at: String = value.created_at,
} }

graphql_object_from! { "Current run projection, intentionally separate from task stage." => pub struct GraphqlCurrentRunSummary("CurrentRunSummary")
    from AgentRunRecord as value {
    "Run identity." => run_id: String = value.run_id,
    "Human-friendly instance identity." => instance_name: String = value.instance_name,
    "Planner, Executor, or Reviewer." => kind: GraphqlTaskRunKind = value.run_kind.into(),
    "Run-local queue/lease status." => status: GraphqlTaskRunStatus = value.status.into(),
    "Lineage attempt." => attempt_index: i64 = i64::from(value.attempt_index),
    "Queue timestamp." => queued_at: String = value.queued_at,
    "Start timestamp." => started_at: Option<String> = value.started_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
    "Safe activity label." => activity_label: String = format!("{} {}", value.run_kind.as_str(), value.status.as_str()),
} }

graphql_object_from! { "Safe audit projection of one run." => pub struct GraphqlTaskRun("TaskRun")
    try_from AgentRunRecord as value {
    "Run identity." => run_id: String = value.run_id,
    "Human-friendly instance identity." => instance_name: String = value.instance_name,
    "Run role." => kind: GraphqlTaskRunKind = value.run_kind.into(),
    "Run-local status." => status: GraphqlTaskRunStatus = value.status.into(),
    "Agent identity." => agent_id: String = value.agent_id,
    "Task generation." => task_generation: i64 = exact_u64(value.task_generation)?,
    "Attempt index." => attempt_index: i64 = i64::from(value.attempt_index),
    "Review round." => review_round: i64 = i64::from(value.review_round),
    "Parent run, when any." => parent_run_id: Option<String> = value.parent_run_id,
    "Requested model snapshot." => model: GraphqlTaskModelSnapshot = value.model.into(),
    "Executor backend frozen into this run." => executor_backend: String = value.executor.backend.to_string(),
    "Executor agent identity frozen into this run." => executor_agent_id: String = value.executor.agent_id,
    "Effective working directory frozen into this run." => effective_cwd: Option<String> = value.effective_cwd,
    "ACP session identity for diagnostics." => acp_session_id: Option<String> = value.acp_session_id,
    "Safe actual provider family." => actual_provider_kind: Option<String> = value.actual_provider_kind,
    "Safe actual model profile." => actual_model_profile: Option<String> = value.actual_model_profile,
    "Immutable policy snapshot." => execution_policy: GraphqlTaskExecutionPolicy = value.execution_policy.into(),
    "Safe terminal error code." => error_code: Option<String> = value.error_code,
    "Safe terminal error message." => error_message: Option<String> = value.error_message,
    "Completed provider calls." => provider_call_count: i64 = i64::from(value.provider_call_count),
    "Dispatched tool calls." => tool_call_count: i64 = i64::from(value.tool_call_count),
    "Cumulative input tokens." => input_tokens: i64 = exact_u64(value.input_tokens)?,
    "Cumulative cached-input tokens." => cached_input_tokens: i64 = exact_u64(value.cached_input_tokens)?,
    "Cumulative output tokens." => output_tokens: i64 = exact_u64(value.output_tokens)?,
    "Active execution duration in milliseconds." => active_milliseconds: i64 = exact_u64(value.active_milliseconds)?,
    "Queue timestamp." => queued_at: String = value.queued_at,
    "Start timestamp." => started_at: Option<String> = value.started_at,
    "End timestamp." => ended_at: Option<String> = value.ended_at,
    "Creation timestamp." => created_at: String = value.created_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
} }

graphql_object_from! { "Bounded transcript item for a selected run." => pub struct GraphqlTaskRunItem("TaskRunItem")
    from (String, AgentRunItemRecord) as value {
    "Item identity." => item_id: String = value.1.item_id,
    "Run identity." => run_id: String = value.1.run_id,
    "Opaque transcript cursor." => cursor: String = value.0,
    "Sequence index." => sequence_index: i64 = value.1.sequence_index,
    "Provider round." => round_index: i64 = value.1.round_index,
    "Transcript kind." => kind: GraphqlTaskRunItemKind = value.1.kind.into(),
    "Item status." => status: GraphqlTaskRunItemStatus = value.1.status.into(),
    "Correlation identity, when present." => correlation_id: Option<String> = value.1.correlation_id,
    "Parent item, when present." => parent_item_id: Option<String> = value.1.parent_item_id,
    "Safe content text." => content_text: Option<String> = value.1.content_text,
    "Safe structured payload." => payload: Json<serde_json::Value> = Json(value.1.payload),
    "Creation timestamp." => created_at: String = value.1.created_at,
    "Last update timestamp." => updated_at: String = value.1.updated_at,
} }
