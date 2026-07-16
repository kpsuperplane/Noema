use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::TaskDomainError;

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
    /// Return the stable SQLite/API representation.
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

impl fmt::Display for AgentRunItemKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AgentRunItemKind {
    type Err = TaskDomainError;

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
            other => Err(TaskDomainError::InvalidEnum {
                kind: "agent_run_item_kind",
                value: other.to_string(),
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
    /// Return the stable persistence representation.
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

impl fmt::Display for AgentRunItemStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AgentRunItemStatus {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "agent_run_item_status",
                value: other.to_string(),
            }),
        }
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

#[cfg(test)]
mod tests {
    use super::{AgentRunItemKind, AgentRunItemStatus};

    #[test]
    fn run_item_kind_wire_vocabulary_is_closed_and_stable() {
        for (kind, wire) in [
            (AgentRunItemKind::ModelInput, "model_input"),
            (AgentRunItemKind::AssistantOutput, "assistant_output"),
            (AgentRunItemKind::ToolCall, "tool_call"),
            (AgentRunItemKind::ToolResult, "tool_result"),
            (AgentRunItemKind::ProgressNotice, "progress_notice"),
            (AgentRunItemKind::ContextCheckpoint, "context_checkpoint"),
            (AgentRunItemKind::TaskSubmission, "task_submission"),
            (AgentRunItemKind::TaskReview, "task_review"),
            (AgentRunItemKind::ArtifactReference, "artifact_reference"),
            (AgentRunItemKind::Failure, "failure"),
            (AgentRunItemKind::Cancellation, "cancellation"),
        ] {
            assert_eq!(kind.as_str(), wire);
            assert_eq!(
                wire.parse::<AgentRunItemKind>().expect("known item kind"),
                kind
            );
            assert_eq!(
                serde_json::from_str::<AgentRunItemKind>(
                    &serde_json::to_string(&kind).expect("serialize item kind")
                )
                .expect("deserialize item kind"),
                kind
            );
        }
        assert!("extension_item".parse::<AgentRunItemKind>().is_err());
        assert!(serde_json::from_str::<AgentRunItemKind>("\"extension_item\"").is_err());
    }

    #[test]
    fn run_item_status_wire_vocabulary_is_closed_and_stable() {
        for (status, wire) in [
            (AgentRunItemStatus::Pending, "pending"),
            (AgentRunItemStatus::Running, "running"),
            (AgentRunItemStatus::Completed, "completed"),
            (AgentRunItemStatus::Failed, "failed"),
            (AgentRunItemStatus::Cancelled, "cancelled"),
            (AgentRunItemStatus::Skipped, "skipped"),
        ] {
            assert_eq!(status.as_str(), wire);
            assert_eq!(
                wire.parse::<AgentRunItemStatus>()
                    .expect("known item status"),
                status
            );
            assert_eq!(
                serde_json::from_str::<AgentRunItemStatus>(
                    &serde_json::to_string(&status).expect("serialize item status")
                )
                .expect("deserialize item status"),
                status
            );
        }
        assert!("future".parse::<AgentRunItemStatus>().is_err());
        assert!(serde_json::from_str::<AgentRunItemStatus>("\"future\"").is_err());
    }
}
