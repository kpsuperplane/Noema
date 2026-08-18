use noema_providers::ProviderSelectionSnapshot;
use serde::{Deserialize, Serialize};

use crate::{
    TaskExecutionPolicy, TaskExecutorSelection, TaskId, WorkDomainError, error::invalid_input,
};

/// Stable built-in agent identity for planner/executor work.
pub const TASK_EXECUTOR_AGENT_ID: &str = "agent:task-executor";
/// Stable built-in agent identity for independent reviews.
pub const TASK_REVIEWER_AGENT_ID: &str = "agent:task-reviewer";

string_enum! {
/// Role of one bounded background run.
pub enum RunKind, "run.kind" {
    /// Produces a complete plan or opens a blocking gate.
    Planner => "planner",
    /// Performs current Task work.
    Executor => "executor",
    /// Independently evaluates current Task files.
    Reviewer => "reviewer",
}
}

string_enum! {
/// Queue and lease lifecycle of one run; never a task state axis.
pub enum RunStatus, "run.status" {
    /// Waiting for a worker to claim the run.
    Queued => "queued",
    /// Claimed but not started.
    Leased => "leased",
    /// Provider/tool execution is active.
    Running => "running",
    /// Terminal contract was accepted.
    Completed => "completed",
    /// Paused at a safe human-gate boundary.
    WaitingForApproval => "waiting_for_approval",
    /// Lease/process interruption occurred.
    Interrupted => "interrupted",
    /// Run cannot safely continue.
    Failed => "failed",
    /// Explicitly cancelled by a task command.
    Cancelled => "cancelled",
}
}

impl RunStatus {
    /// Whether this status can transition to another run-local status.
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

/// Persisted queue/lease/run projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct AgentRunRecord {
    pub run_id: String,
    pub instance_name: String,
    pub task_id: TaskId,
    pub task_generation: u64,
    pub run_kind: RunKind,
    pub agent_id: String,
    pub attempt_index: u32,
    pub review_round: u32,
    pub parent_run_id: Option<String>,
    pub model: ProviderSelectionSnapshot,
    pub executor: TaskExecutorSelection,
    pub effective_cwd: Option<String>,
    pub acp_session_id: Option<String>,
    pub actual_provider_kind: Option<String>,
    pub actual_model_profile: Option<String>,
    pub execution_policy: TaskExecutionPolicy,
    pub status: RunStatus,
    pub queued_at: String,
    pub lease_owner: Option<String>,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<String>,
    pub heartbeat_at: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub cancellation_requested: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub provider_call_count: u32,
    pub tool_call_count: u32,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub active_milliseconds: u64,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentRunRecord {
    /// Validate the role and round invariant on a persisted run.
    /// # Errors
    /// Returns [`WorkDomainError`] when the role, round, or agent is invalid.
    pub fn validate_lineage(&self) -> Result<(), WorkDomainError> {
        validate_run_lineage(self.run_kind, self.review_round, self.agent_id.as_str())
    }
}

/// Result of one run heartbeat/lease renewal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct AgentRunHeartbeat {
    pub lease_expires_at: String,
    pub cancellation_requested: bool,
}

fn validate_run_lineage(
    kind: RunKind,
    review_round: u32,
    agent_id: &str,
) -> Result<(), WorkDomainError> {
    match kind {
        RunKind::Planner => {
            if review_round != 0 || agent_id.trim().is_empty() {
                return Err(invalid_input(
                    "run.lineage",
                    "Planner runs require round 0 and an agent",
                ));
            }
        }
        RunKind::Executor => {
            if review_round == 0 || agent_id.trim().is_empty() {
                return Err(invalid_input(
                    "run.lineage",
                    "Executor runs require a positive review round and an agent",
                ));
            }
        }
        RunKind::Reviewer => {
            if review_round == 0 || agent_id != TASK_REVIEWER_AGENT_ID {
                return Err(invalid_input(
                    "run.lineage",
                    "Reviewer runs require a positive round and the reviewer agent",
                ));
            }
        }
    }
    Ok(())
}
