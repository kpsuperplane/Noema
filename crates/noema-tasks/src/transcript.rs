use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Closed durable transcript vocabulary for task agent runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunItemKind {
    /// Provider-visible model input.
    ModelInput,
    /// Assistant output text.
    AssistantOutput,
    /// Tool invocation.
    ToolCall,
    /// Tool result.
    ToolResult,
    /// Progress or checkpoint notice.
    ProgressNotice,
    /// Compacted semantic context.
    ContextCheckpoint,
    /// Executor terminal submission.
    TaskSubmission,
    /// Reviewer terminal decision.
    TaskReview,
    /// Governed artifact reference.
    ArtifactReference,
    /// Safe failure notice.
    Failure,
    /// Cancellation notice.
    Cancellation,
}

task_vocabulary!(AgentRunItemKind, "agent_run_item_kind", {
    ModelInput => "model_input",
    AssistantOutput => "assistant_output",
    ToolCall => "tool_call",
    ToolResult => "tool_result",
    ProgressNotice => "progress_notice",
    ContextCheckpoint => "context_checkpoint",
    TaskSubmission => "task_submission",
    TaskReview => "task_review",
    ArtifactReference => "artifact_reference",
    Failure => "failure",
    Cancellation => "cancellation",
});

/// Lifecycle state for one correlated transcript item.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunItemStatus {
    /// Work has been declared but has not started.
    Pending,
    /// Work is currently in progress.
    Running,
    /// Work completed successfully.
    #[default]
    Completed,
    /// Work ended with an error.
    Failed,
    /// Work was cancelled.
    Cancelled,
    /// Work was intentionally not executed.
    Skipped,
}

task_vocabulary!(AgentRunItemStatus, "agent_run_item_status", {
    Pending => "pending",
    Running => "running",
    Completed => "completed",
    Failed => "failed",
    Cancelled => "cancelled",
    Skipped => "skipped",
});

/// Input for appending or upserting one run transcript item.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentRunItem {
    /// Optional stable id used to coalesce streaming updates.
    pub item_id: Option<String>,
    /// Owning run id.
    pub run_id: String,
    /// Zero-based provider round.
    pub round_index: i64,
    /// Transcript item kind.
    pub kind: AgentRunItemKind,
    /// Current lifecycle state.
    pub status: AgentRunItemStatus,
    /// Tool/provider correlation id.
    pub correlation_id: Option<String>,
    /// Optional parent transcript item.
    pub parent_item_id: Option<String>,
    /// Human-readable content.
    pub content_text: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

/// Persisted transcript item for one background run.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunItemRecord {
    /// Stable item id.
    pub item_id: String,
    /// Owning run id.
    pub run_id: String,
    /// Monotonic sequence within the run.
    pub sequence_index: i64,
    /// Zero-based provider round.
    pub round_index: i64,
    /// Transcript item kind.
    pub kind: AgentRunItemKind,
    /// Current lifecycle state.
    pub status: AgentRunItemStatus,
    /// Tool/provider correlation id.
    pub correlation_id: Option<String>,
    /// Optional parent transcript item.
    pub parent_item_id: Option<String>,
    /// Human-readable content.
    pub content_text: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}
