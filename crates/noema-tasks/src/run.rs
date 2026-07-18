use noema_providers::ProviderSelectionSnapshot;
use serde::{Deserialize, Serialize};

use crate::{TaskContractId, TaskExecutionPolicy, TaskId, WorkDomainError, error::invalid_input};

/// Stable built-in agent identity for planner/executor work.
pub const TASK_EXECUTOR_AGENT_ID: &str = "agent:task-executor";
/// Stable built-in agent identity for independent reviews.
pub const TASK_REVIEWER_AGENT_ID: &str = "agent:task-reviewer";

/// Role of one bounded background run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// Produces a complete plan or opens a blocking gate.
    Planner,
    /// Produces an immutable task submission.
    Executor,
    /// Independently evaluates one submission.
    Reviewer,
}

impl RunKind {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
        }
    }
}

impl std::fmt::Display for RunKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for RunKind {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "planner" => Ok(Self::Planner),
            "executor" => Ok(Self::Executor),
            "reviewer" => Ok(Self::Reviewer),
            other => Err(invalid_input("run.kind", format!("unknown value {other}"))),
        }
    }
}

/// Queue and lease lifecycle of one run; never a task state axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Waiting for a worker to claim the run.
    Queued,
    /// Claimed but not started.
    Leased,
    /// Provider/tool execution is active.
    Running,
    /// Terminal contract was accepted.
    Completed,
    /// Paused at a safe human-gate boundary.
    WaitingForApproval,
    /// Lease/process interruption occurred.
    Interrupted,
    /// Run cannot safely continue.
    Failed,
    /// Explicitly cancelled by a task command.
    Cancelled,
}

impl RunStatus {
    /// Stable persisted representation.
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

impl std::fmt::Display for RunStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for RunStatus {
    type Err = WorkDomainError;

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
            other => Err(invalid_input(
                "run.status",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Input for one queued run created by a semantic command or reconciler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAgentRun {
    /// Optional caller-supplied run identity.
    pub run_id: Option<String>,
    /// Owning task.
    pub task_id: TaskId,
    /// Generation this run is allowed to advance.
    pub task_generation: u64,
    /// Run role.
    pub run_kind: RunKind,
    /// Acting agent identity.
    pub agent_id: String,
    /// Contract required for Executor/Reviewer and absent for Planner.
    pub contract_id: Option<TaskContractId>,
    /// Attempt index within this run lineage.
    pub attempt_index: u32,
    /// Review round for Executor/Reviewer lineage.
    pub review_round: u32,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Submission being reviewed, when any.
    pub triggering_submission_id: Option<String>,
    /// Review that triggered an executor revision, when any.
    pub triggering_review_id: Option<String>,
    /// Immutable provider selection snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Immutable policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
}

impl NewAgentRun {
    /// Validate role/contract lineage before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when generation, agent identity, role and
    /// trigger lineage, provider selection, or execution policy is invalid.
    pub fn validated(mut self) -> Result<Self, WorkDomainError> {
        if self.task_generation == 0 {
            return Err(invalid_input(
                "run.task_generation",
                "generation must be positive",
            ));
        }
        self.run_id = normalize_optional(self.run_id.take());
        self.agent_id = required(&self.agent_id, "run.agent_id")?;
        self.parent_run_id = normalize_optional(self.parent_run_id.take());
        self.triggering_submission_id = normalize_optional(self.triggering_submission_id.take());
        self.triggering_review_id = normalize_optional(self.triggering_review_id.take());
        validate_run_lineage(
            self.run_kind,
            self.contract_id.as_ref(),
            self.review_round,
            self.triggering_submission_id.as_deref(),
            self.triggering_review_id.as_deref(),
            self.agent_id.as_str(),
        )?;
        self.model = self
            .model
            .normalized_for_persistence()
            .map_err(|error| invalid_input("run.model", error.to_string()))?;
        self.execution_policy.validated()?;
        Ok(self)
    }
}

/// Persisted queue/lease/run projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunRecord {
    /// Stable run identity.
    pub run_id: String,
    /// Owning task.
    pub task_id: TaskId,
    /// Generation fenced by this run.
    pub task_generation: u64,
    /// Contract identity, absent only for Planner runs.
    pub contract_id: Option<TaskContractId>,
    /// Run role.
    pub run_kind: RunKind,
    /// Acting agent identity.
    pub agent_id: String,
    /// Attempt index within this lineage.
    pub attempt_index: u32,
    /// Review round within this contract.
    pub review_round: u32,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Submission under review.
    pub triggering_submission_id: Option<String>,
    /// Review that triggered this run.
    pub triggering_review_id: Option<String>,
    /// Requested provider snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Provider-reported actual family, when known.
    pub actual_provider_kind: Option<String>,
    /// Provider-reported actual profile, when known.
    pub actual_model_profile: Option<String>,
    /// Immutable policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
    /// Current queue/lease status.
    pub status: RunStatus,
    /// Queue timestamp.
    pub queued_at: String,
    /// Lease owner.
    pub lease_owner: Option<String>,
    /// Lease token.
    pub lease_token: Option<String>,
    /// Lease expiry timestamp.
    pub lease_expires_at: Option<String>,
    /// Last heartbeat timestamp.
    pub heartbeat_at: Option<String>,
    /// Start timestamp.
    pub started_at: Option<String>,
    /// End timestamp.
    pub ended_at: Option<String>,
    /// Whether cancellation has been requested.
    pub cancellation_requested: bool,
    /// Safe terminal error code.
    pub error_code: Option<String>,
    /// Safe terminal error message.
    pub error_message: Option<String>,
    /// Provider call count.
    pub provider_call_count: u32,
    /// Tool call count.
    pub tool_call_count: u32,
    /// Input token count.
    pub input_tokens: u64,
    /// Cached input token count.
    pub cached_input_tokens: u64,
    /// Output token count.
    pub output_tokens: u64,
    /// Active execution duration in milliseconds.
    pub active_milliseconds: u64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl AgentRunRecord {
    /// Validate the role/contract invariant on a persisted run.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when the run role, contract, review round,
    /// trigger identities, and built-in agent identity are inconsistent.
    pub fn validate_contract_lineage(&self) -> Result<(), WorkDomainError> {
        validate_run_lineage(
            self.run_kind,
            self.contract_id.as_ref(),
            self.review_round,
            self.triggering_submission_id.as_deref(),
            self.triggering_review_id.as_deref(),
            self.agent_id.as_str(),
        )
    }
}

/// Result of one run heartbeat/lease renewal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunHeartbeat {
    /// New lease expiration timestamp.
    pub lease_expires_at: String,
    /// Whether the task requested cancellation.
    pub cancellation_requested: bool,
}

fn validate_run_contract(
    kind: RunKind,
    contract_id: Option<&TaskContractId>,
) -> Result<(), WorkDomainError> {
    match (kind, contract_id) {
        (RunKind::Planner, None) | (RunKind::Executor | RunKind::Reviewer, Some(_)) => Ok(()),
        (RunKind::Planner, Some(_)) => Err(invalid_input(
            "run.contract_id",
            "Planner runs cannot carry a contract",
        )),
        (RunKind::Executor | RunKind::Reviewer, None) => Err(invalid_input(
            "run.contract_id",
            "Executor and Reviewer runs require a contract",
        )),
    }
}

fn validate_run_lineage(
    kind: RunKind,
    contract_id: Option<&TaskContractId>,
    review_round: u32,
    triggering_submission_id: Option<&str>,
    triggering_review_id: Option<&str>,
    agent_id: &str,
) -> Result<(), WorkDomainError> {
    validate_run_contract(kind, contract_id)?;
    match kind {
        RunKind::Planner => {
            if review_round != 0
                || triggering_submission_id.is_some()
                || triggering_review_id.is_some()
                || agent_id != TASK_EXECUTOR_AGENT_ID
            {
                return Err(invalid_input(
                    "run.lineage",
                    "Planner runs require round 0, no contract/evidence triggers, and the executor agent",
                ));
            }
        }
        RunKind::Executor => {
            if review_round == 0
                || triggering_submission_id.is_some()
                || agent_id != TASK_EXECUTOR_AGENT_ID
            {
                return Err(invalid_input(
                    "run.lineage",
                    "Executor runs require a positive review round, no submission trigger, and the executor agent",
                ));
            }
        }
        RunKind::Reviewer => {
            if review_round == 0
                || triggering_submission_id.is_none()
                || triggering_review_id.is_some()
                || agent_id != TASK_REVIEWER_AGENT_ID
            {
                return Err(invalid_input(
                    "run.lineage",
                    "Reviewer runs require a positive round, a submission trigger, and the reviewer agent",
                ));
            }
        }
    }
    Ok(())
}

fn required(value: &str, field: &'static str) -> Result<String, WorkDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid_input(field, "value cannot be blank"))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
