use async_graphql::{Json, SimpleObject};

use super::*;

/// Derived human attention, never persisted as task state.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskAttention")]
pub struct GraphqlTaskAttention {
    /// Clarification, approval, recovery, or review readiness.
    pub kind: GraphqlTaskAttentionKind,
    /// Stable UI title.
    pub title: String,
    /// Safe summary.
    pub summary: String,
    /// Complete related gate evidence, when any.
    pub gate: Option<GraphqlTaskGate>,
    /// Complete related review evidence, when any.
    pub review: Option<GraphqlTaskReview>,
    /// Authoritative task card without recursively embedding attention.
    pub task: GraphqlTaskCard,
    /// Server-authorized actions.
    pub valid_actions: Vec<GraphqlValidTaskAction>,
}

/// Closed server-authorized task action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, async_graphql::Enum)]
#[graphql(name = "ValidTaskAction")]
pub enum GraphqlValidTaskAction {
    /// Edit Inbox capture fields.
    Edit,
    /// Authorize dispatch.
    Queue,
    /// Answer a gate.
    Answer,
    /// Retry a recovery gate.
    Retry,
    /// Accept an approved result.
    Accept,
    /// Request changed work.
    RequestChanges,
    /// Cancel work.
    Cancel,
    /// Reopen terminal history.
    Reopen,
}

impl From<noema_store::WorkTaskValidAction> for GraphqlValidTaskAction {
    fn from(value: noema_store::WorkTaskValidAction) -> Self {
        match value {
            noema_store::WorkTaskValidAction::Edit => Self::Edit,
            noema_store::WorkTaskValidAction::Queue => Self::Queue,
            noema_store::WorkTaskValidAction::Answer => Self::Answer,
            noema_store::WorkTaskValidAction::Retry => Self::Retry,
            noema_store::WorkTaskValidAction::Accept => Self::Accept,
            noema_store::WorkTaskValidAction::RequestChanges => Self::RequestChanges,
            noema_store::WorkTaskValidAction::Cancel => Self::Cancel,
            noema_store::WorkTaskValidAction::Reopen => Self::Reopen,
        }
    }
}

/// Safe task provenance.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSource")]
pub struct GraphqlTaskSource {
    /// Source classification.
    pub source_kind: GraphqlTaskSourceKind,
    /// Source conversation.
    pub conversation_id: Option<String>,
    /// Source turn.
    pub turn_id: Option<String>,
    /// Source transcript item.
    pub item_id: Option<String>,
}

/// Authoritative task card embedded by a Needs You item.
///
/// This deliberately excludes `attention`: the surrounding `TaskAttention`
/// already is that projection, so including it again would make the response
/// graph recursive while adding no task authority.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskCard")]
pub struct GraphqlTaskCard {
    pub task_id: String,
    pub workspace: GraphqlWorkspace,
    pub project: Option<GraphqlProject>,
    pub title: String,
    pub description_preview: String,
    pub stage: GraphqlWorkflowStage,
    pub revision: i64,
    pub generation: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub current_run: Option<GraphqlCurrentRunSummary>,
    pub active_gate: Option<GraphqlTaskGate>,
    pub latest_review: Option<GraphqlTaskReviewSummary>,
    pub valid_actions: Vec<GraphqlValidTaskAction>,
}

/// Board/list task summary.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSummary")]
pub struct GraphqlTaskSummary {
    /// Task identity.
    pub task_id: String,
    /// Workspace placement.
    pub workspace: GraphqlWorkspace,
    /// Nullable project placement.
    pub project: Option<GraphqlProject>,
    /// Human title.
    pub title: String,
    /// Bounded description preview.
    pub description_preview: String,
    /// The only task-level state.
    pub stage: GraphqlWorkflowStage,
    /// Optimistic revision.
    pub revision: i64,
    /// Execution generation fence.
    pub generation: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Acceptance timestamp, when any.
    pub completed_at: Option<String>,
    /// Current run projection.
    pub current_run: Option<GraphqlCurrentRunSummary>,
    /// Open gate projection.
    pub active_gate: Option<GraphqlTaskGate>,
    /// Latest review projection.
    pub latest_review: Option<GraphqlTaskReviewSummary>,
    /// Derived attention.
    pub attention: Option<GraphqlTaskAttention>,
    /// Server-authorized actions.
    pub valid_actions: Vec<GraphqlValidTaskAction>,
}

/// Full task detail projection.
#[derive(Clone, Debug)]
pub struct GraphqlTaskDetail {
    /// Task identity.
    pub task_id: String,
    /// Workspace placement.
    pub workspace: GraphqlWorkspace,
    /// Nullable project placement.
    pub project: Option<GraphqlProject>,
    /// Full title.
    pub title: String,
    /// Full description Markdown.
    pub description: String,
    /// The same bounded preview exposed by `TaskSummary`.
    pub description_preview: String,
    /// The only task-level state.
    pub stage: GraphqlWorkflowStage,
    /// Optimistic revision.
    pub revision: i64,
    /// Execution generation fence.
    pub generation: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Acceptance timestamp, when any.
    pub completed_at: Option<String>,
    /// Safe provenance.
    pub source: GraphqlTaskSource,
    /// Workflow metadata.
    pub workflow: GraphqlWorkflow,
    /// Current immutable contract.
    pub current_contract: Option<GraphqlTaskExecutionContract>,
    /// Current run projection.
    pub current_run: Option<GraphqlCurrentRunSummary>,
    /// Open gate projection.
    pub active_gate: Option<GraphqlTaskGate>,
    /// Latest immutable submission.
    pub latest_submission: Option<GraphqlTaskSubmission>,
    /// Human-accepted immutable result, keyed by the accepted submission.
    pub accepted_result: Option<GraphqlTaskSubmission>,
    /// Latest immutable review.
    pub latest_review: Option<GraphqlTaskReviewSummary>,
    /// Derived attention.
    pub attention: Option<GraphqlTaskAttention>,
    /// Server-authorized actions.
    pub valid_actions: Vec<GraphqlValidTaskAction>,
}

/// Durable Work ledger event projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkEvent")]
pub struct GraphqlWorkEvent {
    /// Opaque global event cursor.
    pub cursor: String,
    /// Event identity.
    pub event_id: String,
    /// Closed dotted event kind.
    pub kind: String,
    /// Event timestamp.
    pub occurred_at: String,
    /// Workspace linkage.
    pub workspace_id: String,
    /// Project linkage.
    pub project_id: Option<String>,
    /// Task linkage.
    pub task_id: Option<String>,
    /// Run linkage.
    pub run_id: Option<String>,
    /// Safe audit actor.
    pub actor: String,
    /// Causation identity.
    pub causation_id: Option<String>,
    /// Correlation identity.
    pub correlation_id: String,
    /// Bounded safe JSON payload.
    pub payload: Json<serde_json::Value>,
}

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
