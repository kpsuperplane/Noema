use serde::{Deserialize, Serialize};
use serde_json::Value;

string_enum! {
/// Closed durable transcript vocabulary for task agent runs.
pub enum AgentRunItemKind, "agent_run_item_kind" {
    /// Provider-visible model input.
    ModelInput => "model_input",
    /// Assistant output text.
    AssistantOutput => "assistant_output",
    /// Tool invocation.
    ToolCall => "tool_call",
    /// Tool result.
    ToolResult => "tool_result",
    /// Progress or checkpoint notice.
    ProgressNotice => "progress_notice",
    /// Compacted semantic context.
    ContextCheckpoint => "context_checkpoint",
    /// Executor terminal submission.
    TaskSubmission => "task_submission",
    /// Reviewer terminal decision.
    TaskReview => "task_review",
    /// Governed artifact reference.
    ArtifactReference => "artifact_reference",
    /// Safe failure notice.
    Failure => "failure",
    /// Cancellation notice.
    Cancellation => "cancellation",
}
}

string_enum! {
/// Lifecycle state for one correlated transcript item.
pub enum AgentRunItemStatus, "agent_run_item_status" {
    /// Work has been declared but has not started.
    Pending => "pending",
    /// Work is currently in progress.
    Running => "running",
    /// Work completed successfully.
    Completed => "completed",
    /// Work ended with an error.
    Failed => "failed",
    /// Work was cancelled.
    Cancelled => "cancelled",
    /// Work was intentionally not executed.
    Skipped => "skipped",
}
}

#[allow(
    clippy::derivable_impls,
    reason = "the shared enum macro owns common derives"
)]
impl Default for AgentRunItemStatus {
    fn default() -> Self {
        Self::Completed
    }
}

/// Input for appending or upserting one run transcript item.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct NewAgentRunItem {
    pub item_id: Option<String>,
    pub run_id: String,
    pub round_index: i64,
    pub kind: AgentRunItemKind,
    pub status: AgentRunItemStatus,
    pub correlation_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub content_text: Option<String>,
    pub payload: Value,
}

/// Persisted transcript item for one background run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct AgentRunItemRecord {
    pub item_id: String,
    pub run_id: String,
    pub sequence_index: i64,
    pub round_index: i64,
    pub kind: AgentRunItemKind,
    pub status: AgentRunItemStatus,
    pub correlation_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub content_text: Option<String>,
    pub payload: Value,
    pub created_at: String,
    pub updated_at: String,
}
