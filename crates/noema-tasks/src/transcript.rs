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

impl AgentRunItemKind {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelInput => "model_input",
            Self::AssistantOutput => "assistant_output",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::ProgressNotice => "progress_notice",
            Self::ContextCheckpoint => "context_checkpoint",
            Self::TaskSubmission => "task_submission",
            Self::TaskReview => "task_review",
            Self::ArtifactReference => "artifact_reference",
            Self::Failure => "failure",
            Self::Cancellation => "cancellation",
        }
    }
}

impl std::fmt::Display for AgentRunItemKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for AgentRunItemKind {
    type Err = crate::WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "model_input" => Ok(Self::ModelInput),
            "assistant_output" => Ok(Self::AssistantOutput),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "progress_notice" => Ok(Self::ProgressNotice),
            "context_checkpoint" => Ok(Self::ContextCheckpoint),
            "task_submission" => Ok(Self::TaskSubmission),
            "task_review" => Ok(Self::TaskReview),
            "artifact_reference" => Ok(Self::ArtifactReference),
            "failure" => Ok(Self::Failure),
            "cancellation" => Ok(Self::Cancellation),
            other => Err(crate::WorkDomainError::InvalidInput {
                field: "agent_run_item_kind",
                message: format!("unknown value {other}"),
            }),
        }
    }
}

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

impl AgentRunItemStatus {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }
}

impl std::fmt::Display for AgentRunItemStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for AgentRunItemStatus {
    type Err = crate::WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            other => Err(crate::WorkDomainError::InvalidInput {
                field: "agent_run_item_status",
                message: format!("unknown value {other}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{AgentRunItemKind, AgentRunItemStatus};

    #[test]
    fn transcript_vocabularies_are_displayable_and_fail_closed() {
        for kind in [
            AgentRunItemKind::ModelInput,
            AgentRunItemKind::AssistantOutput,
            AgentRunItemKind::ToolCall,
            AgentRunItemKind::ToolResult,
            AgentRunItemKind::ProgressNotice,
            AgentRunItemKind::ContextCheckpoint,
            AgentRunItemKind::TaskSubmission,
            AgentRunItemKind::TaskReview,
            AgentRunItemKind::ArtifactReference,
            AgentRunItemKind::Failure,
            AgentRunItemKind::Cancellation,
        ] {
            assert_eq!(AgentRunItemKind::from_str(kind.as_str()).unwrap(), kind);
            assert_eq!(kind.to_string(), kind.as_str());
        }
        for status in [
            AgentRunItemStatus::Pending,
            AgentRunItemStatus::Running,
            AgentRunItemStatus::Completed,
            AgentRunItemStatus::Failed,
            AgentRunItemStatus::Cancelled,
            AgentRunItemStatus::Skipped,
        ] {
            assert_eq!(
                AgentRunItemStatus::from_str(status.as_str()).unwrap(),
                status
            );
            assert_eq!(status.to_string(), status.as_str());
        }
        assert!(AgentRunItemKind::from_str("unknown").is_err());
        assert!(AgentRunItemStatus::from_str("unknown").is_err());
    }
}

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
