use async_graphql::Json;

use super::*;

graphql_object! { "Derived human attention, never persisted as task state." => pub struct GraphqlTaskAttention("TaskAttention") {
    "Clarification, approval, recovery, or review readiness." => kind: GraphqlTaskAttentionKind,
    "Stable UI title." => title: String,
    "Safe summary." => summary: String,
    "Complete related gate evidence, when any." => gate: Option<GraphqlTaskGate>,
    "Complete related review evidence, when any." => review: Option<GraphqlTaskReview>,
    "Authoritative task card without recursively embedding attention." => task: GraphqlTaskCard,
    "Server-authorized actions." => valid_actions: Vec<GraphqlValidTaskAction>,
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
    "Bounded description preview." => description_preview: String,
    "The only task-level state." => stage: GraphqlWorkflowStage,
    "Optimistic revision." => revision: i64,
    "Execution generation fence." => generation: i64,
    "Creation timestamp." => created_at: String,
    "Last update timestamp." => updated_at: String,
    "Completion timestamp, when any." => completed_at: Option<String>,
    "Current run projection." => current_run: Option<GraphqlCurrentRunSummary>,
    "Open gate projection." => active_gate: Option<GraphqlTaskGate>,
    "Latest review projection." => latest_review: Option<GraphqlTaskReviewSummary>,
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
    "Full description Markdown." => description: String,
    "The only task-level state." => stage: GraphqlWorkflowStage,
    "Optimistic revision." => revision: i64,
    "Execution generation fence." => generation: i64,
    "Creation timestamp." => created_at: String,
    "Last update timestamp." => updated_at: String,
    "Completion timestamp, when any." => completed_at: Option<String>,
    "Safe provenance." => source: GraphqlTaskSource,
    "Current immutable contract." => current_contract: Option<GraphqlTaskExecutionContract>,
    "Current run projection." => current_run: Option<GraphqlCurrentRunSummary>,
    "Open gate projection." => active_gate: Option<GraphqlTaskGate>,
    "Latest immutable submission." => latest_submission: Option<GraphqlTaskSubmission>,
    "Reviewer-approved immutable result that completed the task." => completed_result: Option<GraphqlTaskSubmission>,
    "Latest immutable review." => latest_review: Option<GraphqlTaskReviewSummary>,
    "Bounded recent human messages." => messages: Vec<GraphqlTaskMessage>,
    "Bounded recent task runs." => runs: Vec<GraphqlTaskRun>,
    "Bounded recent submissions." => submissions: Vec<GraphqlTaskSubmission>,
    "Bounded recent reviews." => reviews: Vec<GraphqlTaskReview>,
    "Bounded current artifacts." => artifacts: Vec<crate::graphql::artifacts::GraphqlArtifact>,
    "Derived attention." => attention: Option<GraphqlTaskAttention>,
    "Server-authorized actions." => valid_actions: Vec<GraphqlValidTaskAction>,
} }

graphql_object! { "Durable Work ledger event projection." => pub struct GraphqlWorkEvent("WorkEvent") {
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

impl TryFrom<noema_tasks::WorkEventRecord> for GraphqlWorkEvent {
    type Error = noema_store::WorkCursorError;

    fn try_from(value: noema_tasks::WorkEventRecord) -> Result<Self, Self::Error> {
        let cursor = noema_store::WorkEventCursor::new(value.event_sequence)?.encode();
        Ok(Self {
            cursor,
            event_id: value.event_id.into_string(),
            kind: value.kind.as_str().to_string(),
            occurred_at: value.created_at,
            workspace_id: value.workspace_id.into_string(),
            project_id: value.project_id.map(|id| id.into_string()),
            task_id: value.task_id.map(|id| id.into_string()),
            run_id: value.run_id,
            actor: value.actor_id,
            causation_id: value.causation_id,
            correlation_id: value.correlation_id,
            payload: Json(value.safe_payload),
        })
    }
}
