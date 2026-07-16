use std::{fmt, str::FromStr};

use noema_providers::ProviderSelectionSnapshot;
use serde::{Deserialize, Serialize};

use crate::{TaskDomainError, TaskExecutionPolicy};

/// Stable built-in agent id for background executors.
pub const TASK_EXECUTOR_AGENT_ID: &str = "agent:task-executor";
/// Stable built-in agent id for adversarial reviewers.
pub const TASK_REVIEWER_AGENT_ID: &str = "agent:task-reviewer";

/// Kind of durable background run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// Produces a structured task submission.
    Executor,
    /// Independently checks one executor submission.
    Reviewer,
}

impl RunKind {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
        }
    }
}

impl fmt::Display for RunKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RunKind {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "executor" => Ok(Self::Executor),
            "reviewer" => Ok(Self::Reviewer),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "run_kind",
                value: other.to_string(),
            }),
        }
    }
}

/// Queue/lease state for a durable agent run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Waiting for a worker to claim the run.
    Queued,
    /// Claimed by a worker but not yet started.
    Leased,
    /// Provider/tool execution is active.
    Running,
    /// Run produced its terminal contract.
    Completed,
    /// Run is paused pending an explicit approval or intervention.
    WaitingForApproval,
    /// Worker lost its lease or process shutdown interrupted execution.
    Interrupted,
    /// Run cannot safely continue.
    Failed,
    /// Run was explicitly cancelled.
    Cancelled,
}

impl RunStatus {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Validate a durable run transition.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Leased | Self::Cancelled | Self::Failed)
                | (
                    Self::Leased,
                    Self::Running | Self::Interrupted | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Running,
                    Self::Completed
                        | Self::WaitingForApproval
                        | Self::Interrupted
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::WaitingForApproval,
                    Self::Completed | Self::Failed | Self::Cancelled
                )
                | (Self::Interrupted, Self::Failed | Self::Cancelled)
                | (Self::Failed, Self::Cancelled)
        )
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RunStatus {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "leased" => Ok(Self::Leased),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "waiting_for_approval" => Ok(Self::WaitingForApproval),
            "interrupted" => Ok(Self::Interrupted),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "run_status",
                value: other.to_string(),
            }),
        }
    }
}

/// Input for one queued background run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentRun {
    /// Optional stable run id.
    pub run_id: Option<String>,
    /// Owning task id.
    pub task_id: String,
    /// Executor or reviewer role.
    pub run_kind: RunKind,
    /// Built-in or user-configured agent id.
    pub agent_id: String,
    /// Revision represented by this run.
    pub revision_index: i64,
    /// Continuation or infrastructure-recovery attempt within the revision.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional submission this run reviews.
    pub triggering_submission_id: Option<String>,
    /// Optional review this run follows.
    pub triggering_review_id: Option<String>,
    /// Immutable model request snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Immutable execution-policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
    /// Queue priority; larger values run first.
    pub priority: i64,
}

/// Persisted background run and lease state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunRecord {
    /// Stable run id.
    pub run_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Run role.
    pub run_kind: RunKind,
    /// Acting agent id.
    pub agent_id: String,
    /// Revision index.
    pub revision_index: i64,
    /// Continuation or infrastructure-recovery attempt.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional reviewed submission id.
    pub triggering_submission_id: Option<String>,
    /// Optional preceding review id.
    pub triggering_review_id: Option<String>,
    /// Optional human guidance that resumed this run.
    pub resume_message: Option<String>,
    /// Requested model snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Provider-reported actual model, when available.
    pub actual_provider_kind: Option<String>,
    /// Provider-reported actual profile, when available.
    pub actual_model_profile: Option<String>,
    /// Immutable provider-independent execution-policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
    /// Current queue state.
    pub status: RunStatus,
    /// Queue priority.
    pub priority: i64,
    /// Queue timestamp.
    pub queued_at: String,
    /// Current lease owner.
    pub lease_owner: Option<String>,
    /// Current lease token.
    pub lease_token: Option<String>,
    /// Lease expiry timestamp.
    pub lease_expires_at: Option<String>,
    /// Last heartbeat timestamp.
    pub heartbeat_at: Option<String>,
    /// Start timestamp.
    pub started_at: Option<String>,
    /// End timestamp.
    pub ended_at: Option<String>,
    /// Whether cancellation was requested.
    pub cancellation_requested: bool,
    /// Number of infrastructure retries.
    pub retry_count: i64,
    /// Safe terminal error code.
    pub error_code: Option<String>,
    /// Safe terminal error message.
    pub error_message: Option<String>,
    /// Number of completed provider calls.
    pub provider_call_count: i64,
    /// Number of dispatched tool calls.
    pub tool_call_count: i64,
    /// Cumulative input token count.
    pub input_tokens: i64,
    /// Cumulative cached-input token count.
    pub cached_input_tokens: i64,
    /// Cumulative output token count.
    pub output_tokens: i64,
    /// Cumulative active execution duration.
    pub active_milliseconds: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Result of one lease renewal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunHeartbeat {
    /// New lease expiration as Unix seconds.
    pub lease_expires_at: String,
    /// Whether the owner requested cancellation.
    pub cancellation_requested: bool,
}

#[cfg(test)]
mod tests {
    use super::{RunKind, RunStatus};

    #[test]
    fn run_kind_wire_values_are_stable_and_fail_closed() {
        for (value, wire) in [
            (RunKind::Executor, "executor"),
            (RunKind::Reviewer, "reviewer"),
        ] {
            assert_eq!(value.as_str(), wire);
            assert_eq!(wire.parse::<RunKind>().expect("known run kind"), value);
            assert_eq!(
                serde_json::from_str::<RunKind>(
                    &serde_json::to_string(&value).expect("serialize run kind")
                )
                .expect("deserialize run kind"),
                value
            );
        }
        assert!("future_kind".parse::<RunKind>().is_err());
        assert!(serde_json::from_str::<RunKind>("\"future_kind\"").is_err());
    }

    #[test]
    fn run_status_wire_values_are_stable_and_fail_closed() {
        for (value, wire) in [
            (RunStatus::Queued, "queued"),
            (RunStatus::Leased, "leased"),
            (RunStatus::Running, "running"),
            (RunStatus::Completed, "completed"),
            (RunStatus::WaitingForApproval, "waiting_for_approval"),
            (RunStatus::Interrupted, "interrupted"),
            (RunStatus::Failed, "failed"),
            (RunStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(value.as_str(), wire);
            assert_eq!(wire.parse::<RunStatus>().expect("known run status"), value);
            assert_eq!(
                serde_json::from_str::<RunStatus>(
                    &serde_json::to_string(&value).expect("serialize run status")
                )
                .expect("deserialize run status"),
                value
            );
        }
        assert!("future_status".parse::<RunStatus>().is_err());
        assert!(serde_json::from_str::<RunStatus>("\"future_status\"").is_err());
    }
}
