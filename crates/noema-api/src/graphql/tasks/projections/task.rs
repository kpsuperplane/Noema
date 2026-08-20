use async_graphql::Json;

use super::*;

graphql_object! { "Derived human attention, never persisted as task state." => pub struct GraphqlTaskAttention("TaskAttention") {
    "Clarification, approval, recovery, or review readiness." => kind: GraphqlTaskAttentionKind,
    "Stable UI title." => title: String,
    "Safe summary." => summary: String,
    "Complete related gate evidence, when any." => gate: Option<GraphqlTaskGate>,
    "Authoritative task card without recursively embedding attention." => task: GraphqlTaskCard,
    "Server-authorized actions." => valid_actions: Vec<GraphqlValidTaskAction>,
} }

graphql_object! { "Future execution attached directly to an Inbox task." => pub struct GraphqlTaskSchedule("TaskSchedule") {
    "Exact UTC execution instant." => scheduled_for: String,
    "Authoring IANA timezone." => time_zone: String,
    "Missed-instant behavior." => missed_run_policy: GraphqlMissedRunPolicy,
    "Recurring template identity when Repeat is enabled." => recurrence_id: Option<String>,
    "Recurring template revision snapshotted by this occurrence." => recurrence_revision: Option<i64>,
    "Exact recurrence slot represented by this task." => recurrence_scheduled_for: Option<String>,
} }

graphql_object! { "Resolved scheduling preview." => pub struct GraphqlTaskSchedulePreview("TaskSchedulePreview") {
    "Resolved inclusive start instant." => resolved_start: String,
    "Up to five future UTC instants." => occurrences: Vec<String>,
} }

graphql_object! { "Immutable recurring slot history." => pub struct GraphqlRecurrenceOccurrence("RecurrenceOccurrence") {
    "Template revision used for this slot." => recurrence_revision: i64,
    "Exact UTC slot." => scheduled_for: String,
    "Deduplicated local wall-clock minute." => local_slot: String,
    "Whether cron or an explicit request created this occurrence." => trigger: GraphqlRecurrenceOccurrenceTrigger,
    "Materialized, skipped, or coalesced disposition." => resolution: GraphqlRecurrenceOccurrenceResolution,
    "Ordinary child task when materialized." => task_id: Option<String>,
    "Audit timestamp." => created_at: String,
} }

graphql_object! { "Continuing authority and recent history for recurring work." => pub struct GraphqlTaskRecurrence("TaskRecurrence") {
    "Recurring template identity." => recurrence_id: String,
    "Current title for future occurrences." => title: String,
    "Current template TASK.md content." => task_document: String,
    "Transient SHA-256 of the current template." => task_document_digest: String,
    "Inclusive UTC lower bound." => starts_at: String,
    "Five-field cron expression." => cron_expression: String,
    "Authoring IANA timezone." => time_zone: String,
    "Missed-window behavior." => missed_run_policy: GraphqlMissedRunPolicy,
    "Overlap behavior." => overlap_policy: GraphqlOverlapPolicy,
    "Lifecycle." => lifecycle: GraphqlRecurrenceLifecycle,
    "Optimistic template revision." => revision: i64,
    "Next projected UTC slot." => next_run_at: Option<String>,
    "Retained coalesced UTC slot." => pending_coalesced_at: Option<String>,
    "Newest occurrence history." => occurrences: Vec<GraphqlRecurrenceOccurrence>,
} }

graphql_object! { "Current recurring authority for the Tasks list." => pub struct GraphqlTaskRecurrenceSummary("TaskRecurrenceSummary") {
    "Recurring template identity." => recurrence_id: String,
    "Current title for future occurrences." => title: String,
    "Five-field cron expression." => cron_expression: String,
    "Lifecycle." => lifecycle: GraphqlRecurrenceLifecycle,
    "Next projected UTC slot." => next_run_at: Option<String>,
    "Last authority update timestamp." => updated_at: String,
} }

graphql_object_from! { "Safe task provenance." => pub struct GraphqlTaskSource("TaskSource")
    from noema_tasks::TaskProvenance as value {
    "Source conversation." => conversation_id: Option<String> = value.conversation_id,
} }

graphql_object! { "Authoritative task card embedded by a Needs You item. This excludes attention because the surrounding TaskAttention already owns that projection." => pub struct GraphqlTaskCard("TaskCard") {
    "Task identity." => task_id: String,
    "Workspace placement." => workspace: GraphqlWorkspace,
    "Nullable project placement." => project: Option<GraphqlProject>,
    "Human title." => title: String,
    "Bounded current Task document preview." => task_document_preview: String,
    "The only task-level state." => stage: GraphqlWorkflowStage,
    "Optimistic revision." => revision: i64,
    "Execution generation fence." => generation: i64,
    "Assigned executor agent identity." => executor_agent_id: String,
    "Assigned executor backend." => executor_backend: String,
    "Explicit Task directory base override." => cwd_override: Option<String>,
    "Derived effective working directory when already frozen or explicitly configured." => effective_cwd: Option<String>,
    "Effective working-directory source: task, project, or default." => effective_cwd_source: String,
    "Creation timestamp." => created_at: String,
    "Last update timestamp." => updated_at: String,
    "Optional future execution and recurrence provenance." => schedule: Option<GraphqlTaskSchedule>,
    "Completion timestamp, when any." => completed_at: Option<String>,
    "Current run projection." => current_run: Option<GraphqlCurrentRunSummary>,
    "Open gate projection." => active_gate: Option<GraphqlTaskGate>,
    "Server-authorized actions." => valid_actions: Vec<GraphqlValidTaskAction>,
} }

/// Board/list task summary.
#[derive(Clone, Debug, async_graphql::SimpleObject)]
#[graphql(name = "TaskSummary")]
pub struct GraphqlTaskSummary {
    /// Authoritative card fields shared with Needs You.
    #[graphql(flatten)]
    pub task: GraphqlTaskCard,
    /// Derived attention.
    pub attention: Option<GraphqlTaskAttention>,
}

graphql_object! { "Full task detail projection." => pub struct GraphqlTaskDetail("TaskDetail") {
    "Task identity." => task_id: String,
    "Nullable project placement." => project: Option<GraphqlProject>,
    "Full title." => title: String,
    "Current mutable TASK.md content." => task_document: String,
    "Transient SHA-256 of the current Task document." => task_document_digest: String,
    "Current mutable RESULT.md content, when it exists." => result_document: Option<String>,
    "Current mutable REVIEW.md content, when it exists." => review_document: Option<String>,
    "The only task-level state." => stage: GraphqlWorkflowStage,
    "Optimistic revision." => revision: i64,
    "Execution generation fence." => generation: i64,
    "Assigned executor agent identity." => executor_agent_id: String,
    "Assigned executor backend." => executor_backend: String,
    "Explicit Task directory base override." => cwd_override: Option<String>,
    "Derived or frozen effective working directory." => effective_cwd: Option<String>,
    "Effective working-directory source: task, project, or default." => effective_cwd_source: String,
    "Creation timestamp." => created_at: String,
    "Last update timestamp." => updated_at: String,
    "Optional future execution and recurrence provenance." => schedule: Option<GraphqlTaskSchedule>,
    "Completion timestamp, when any." => completed_at: Option<String>,
    "Safe provenance." => source: GraphqlTaskSource,
    "Current run projection." => current_run: Option<GraphqlCurrentRunSummary>,
    "Open gate projection." => active_gate: Option<GraphqlTaskGate>,
    "Bounded recent human messages." => messages: Vec<GraphqlTaskMessage>,
    "Bounded recent task runs." => runs: Vec<GraphqlTaskRun>,
    "Every distinct agent instance that contributed to this task." => contributor_instance_names: Vec<String>,
    "Bounded current artifacts." => artifacts: Vec<crate::graphql::artifacts::GraphqlArtifact>,
    "Derived attention." => attention: Option<GraphqlTaskAttention>,
    "Server-authorized actions." => valid_actions: Vec<GraphqlValidTaskAction>,
} }

graphql_object! { "Saved Tasks event." => pub struct GraphqlTaskEvent("TasksEvent") {
    "Opaque global event cursor." => cursor: String,
    "Event identity." => event_id: String,
    "Closed dotted event kind." => kind: String,
    "Event timestamp." => occurred_at: String,
    "Workspace linkage." => workspace_id: String,
    "Project linkage." => project_id: Option<String>,
    "Task linkage." => task_id: Option<String>,
    "Run linkage." => run_id: Option<String>,
    "Safe audit actor." => actor: String,
    "Causation identity." => causation_id: Option<String>,
    "Correlation identity." => correlation_id: String,
    "Bounded safe JSON payload." => payload: Json<serde_json::Value>,
} }

impl TryFrom<noema_tasks::WorkEventRecord> for GraphqlTaskEvent {
    type Error = noema_store::WorkCursorError;

    fn try_from(value: noema_tasks::WorkEventRecord) -> Result<Self, Self::Error> {
        let cursor = noema_store::WorkEventCursor::new(value.event_sequence())?.encode();
        Ok(Self {
            cursor,
            event_id: value.event_id().to_string(),
            kind: value.kind().as_str().to_string(),
            occurred_at: value.created_at().to_string(),
            workspace_id: value.workspace_id().to_string(),
            project_id: value.project_id().map(ToString::to_string),
            task_id: value.task_id().map(ToString::to_string),
            run_id: value.run_id().map(str::to_string),
            actor: value.actor_id().to_string(),
            causation_id: value.causation_id().map(str::to_string),
            correlation_id: value.correlation_id().to_string(),
            payload: Json(value.safe_payload().clone()),
        })
    }
}
