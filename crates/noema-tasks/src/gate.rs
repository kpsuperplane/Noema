use serde::{Deserialize, Serialize};

use crate::{
    GateResolutionKind, RunKind, TaskContractId, TaskGateId, TaskId, TaskMessageId,
    WorkDomainError, error::invalid_input,
};

/// Structured result of an Approval gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// The governed action may continue.
    Approved,
    /// The governed action must not continue.
    Declined,
}

impl ApprovalDecision {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Declined => "declined",
        }
    }
}

impl std::fmt::Display for ApprovalDecision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ApprovalDecision {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "approved" => Ok(Self::Approved),
            "declined" => Ok(Self::Declined),
            other => Err(invalid_input(
                "approval_decision",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Human intervention category persisted on a task gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGateKind {
    /// Missing facts or an ambiguous choice.
    Clarification,
    /// A governed decision required by policy.
    Approval,
    /// Retry/revision/recovery decision after automated work stopped.
    Recovery,
}

impl TaskGateKind {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clarification => "clarification",
            Self::Approval => "approval",
            Self::Recovery => "recovery",
        }
    }

    /// Whether this gate authorizes one resolution kind.
    ///
    /// The recovery reason and continuation role are part of the authority:
    /// callers must not infer permission from the gate category alone.
    #[must_use]
    pub const fn allows_resolution(
        self,
        recovery_reason: Option<TaskRecoveryReason>,
        retry_run_kind: Option<RunKind>,
        resolution: GateResolutionKind,
    ) -> bool {
        matches!(
            (self, recovery_reason, retry_run_kind, resolution),
            (
                Self::Clarification | Self::Approval,
                None,
                None,
                GateResolutionKind::Answer,
            ) | (
                Self::Recovery,
                Some(TaskRecoveryReason::InfrastructureRetriesExhausted),
                Some(_),
                GateResolutionKind::Answer | GateResolutionKind::Retry,
            ) | (
                Self::Recovery,
                Some(TaskRecoveryReason::ReviewRoundsExhausted),
                Some(RunKind::Executor),
                GateResolutionKind::Answer | GateResolutionKind::Retry,
            ) | (
                Self::Recovery,
                Some(TaskRecoveryReason::UnsafeEffectUncertain),
                Some(_),
                GateResolutionKind::Answer,
            ) | (
                Self::Recovery,
                Some(TaskRecoveryReason::ConfigurationUnavailable),
                Some(_),
                GateResolutionKind::Retry,
            )
        )
    }
}

impl std::fmt::Display for TaskGateKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskGateKind {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "clarification" => Ok(Self::Clarification),
            "approval" => Ok(Self::Approval),
            "recovery" => Ok(Self::Recovery),
            other => Err(invalid_input(
                "task_gate.kind",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Closed reason for a Recovery gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRecoveryReason {
    /// Automatic infrastructure retry bound was exhausted.
    InfrastructureRetriesExhausted,
    /// Automated reviewer round bound was exhausted.
    ReviewRoundsExhausted,
    /// An external effect may have occurred and cannot be replayed safely.
    UnsafeEffectUncertain,
    /// Required provider/model/policy configuration is unavailable.
    ConfigurationUnavailable,
    /// Durable facts are inconsistent and require a human decision.
    InvariantFault,
}

impl TaskRecoveryReason {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InfrastructureRetriesExhausted => "infrastructure_retries_exhausted",
            Self::ReviewRoundsExhausted => "review_rounds_exhausted",
            Self::UnsafeEffectUncertain => "unsafe_effect_uncertain",
            Self::ConfigurationUnavailable => "configuration_unavailable",
            Self::InvariantFault => "invariant_fault",
        }
    }
}

impl std::fmt::Display for TaskRecoveryReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskRecoveryReason {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "infrastructure_retries_exhausted" => Ok(Self::InfrastructureRetriesExhausted),
            "review_rounds_exhausted" => Ok(Self::ReviewRoundsExhausted),
            "unsafe_effect_uncertain" => Ok(Self::UnsafeEffectUncertain),
            "configuration_unavailable" => Ok(Self::ConfigurationUnavailable),
            "invariant_fault" => Ok(Self::InvariantFault),
            other => Err(invalid_input(
                "task_gate.recovery_reason",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Gate lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGateState {
    /// Awaiting a person.
    Open,
    /// Resolved by an answer or retry.
    Resolved,
    /// Superseded by cancellation or a newer generation.
    Superseded,
}

impl TaskGateState {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Superseded => "superseded",
        }
    }
}

impl std::fmt::Display for TaskGateState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskGateState {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "resolved" => Ok(Self::Resolved),
            "superseded" => Ok(Self::Superseded),
            other => Err(invalid_input(
                "task_gate.state",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Structured answer to a gate; approval is never inferred from prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateAnswer {
    /// Durable human explanation or answer.
    pub message_markdown: String,
    /// Required only for Approval gates.
    pub approval_decision: Option<ApprovalDecision>,
}

impl TaskGateAnswer {
    /// Validate answer requirements for one gate kind.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when the answer is blank, an Approval gate
    /// omits a decision, or another gate kind supplies a decision.
    pub fn normalized_for(&self, kind: TaskGateKind) -> Result<Self, WorkDomainError> {
        let message_markdown = self.message_markdown.trim();
        if message_markdown.is_empty() {
            return Err(invalid_input(
                "task_gate_answer.message_markdown",
                "value cannot be blank",
            ));
        }
        if matches!(kind, TaskGateKind::Approval) && self.approval_decision.is_none() {
            return Err(invalid_input(
                "task_gate_answer.approval_decision",
                "approval requires a decision",
            ));
        }
        if !matches!(kind, TaskGateKind::Approval) && self.approval_decision.is_some() {
            return Err(invalid_input(
                "task_gate_answer.approval_decision",
                "decision is only valid for approval",
            ));
        }
        Ok(Self {
            message_markdown: message_markdown.to_string(),
            approval_decision: self.approval_decision,
        })
    }
}

/// Durable human gate projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateRecord {
    /// Stable gate identity.
    pub gate_id: TaskGateId,
    /// Owning task.
    pub task_id: TaskId,
    /// Task generation at gate creation.
    pub task_generation: u64,
    /// Contract being continued, when one exists.
    pub contract_id: Option<TaskContractId>,
    /// Gate category.
    pub kind: TaskGateKind,
    /// Gate lifecycle state.
    pub state: TaskGateState,
    /// Recovery reason for Recovery gates.
    pub recovery_reason: Option<TaskRecoveryReason>,
    /// Explicit continuation role for Recovery gates.
    pub retry_run_kind: Option<RunKind>,
    /// Safe question shown to the person.
    pub prompt_markdown: String,
    /// Bounded context shown with the question.
    pub context_markdown: String,
    /// Actor/run that opened the gate.
    pub opened_by_actor_id: String,
    /// Run whose safe boundary owns the gate.
    pub originating_run_id: Option<String>,
    /// Resolver actor, once resolved/superseded.
    pub resolved_by_actor_id: Option<String>,
    /// Answer message, for resolved gates.
    pub resolution_message_id: Option<TaskMessageId>,
    /// Opening timestamp.
    pub opened_at: String,
    /// Resolution timestamp.
    pub resolved_at: Option<String>,
}

impl TaskGateRecord {
    /// Validate closed gate/recovery combinations and required fields.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when required gate fields are invalid, the
    /// recovery reason and continuation role disagree, or lifecycle state and
    /// resolution fields are inconsistent.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.task_generation == 0 {
            return Err(invalid_input(
                "task_gate.task_generation",
                "generation must be positive",
            ));
        }
        if self.prompt_markdown.trim().is_empty() || self.opened_by_actor_id.trim().is_empty() {
            return Err(invalid_input(
                "task_gate",
                "prompt and opener cannot be blank",
            ));
        }
        match self.kind {
            TaskGateKind::Recovery => {
                if self.recovery_reason.is_none() {
                    return Err(invalid_input(
                        "task_gate.recovery_reason",
                        "recovery gates require a reason",
                    ));
                }
                match (self.recovery_reason, self.retry_run_kind) {
                    (Some(TaskRecoveryReason::InvariantFault), None)
                    | (Some(TaskRecoveryReason::ReviewRoundsExhausted), Some(RunKind::Executor))
                    | (
                        Some(
                            TaskRecoveryReason::InfrastructureRetriesExhausted
                            | TaskRecoveryReason::UnsafeEffectUncertain
                            | TaskRecoveryReason::ConfigurationUnavailable,
                        ),
                        Some(_),
                    ) => {}
                    _ => {
                        return Err(invalid_input(
                            "task_gate.retry_run_kind",
                            "recovery reason and continuation role are inconsistent",
                        ));
                    }
                }
            }
            TaskGateKind::Clarification | TaskGateKind::Approval => {
                if self.recovery_reason.is_some() || self.retry_run_kind.is_some() {
                    return Err(invalid_input(
                        "task_gate",
                        "only recovery gates carry recovery fields",
                    ));
                }
            }
        }
        match self.state {
            TaskGateState::Open
                if self.resolved_by_actor_id.is_some()
                    || self.resolution_message_id.is_some()
                    || self.resolved_at.is_some() =>
            {
                Err(invalid_input(
                    "task_gate.state",
                    "open gates cannot have resolution fields",
                ))
            }
            TaskGateState::Resolved
                if self.resolved_by_actor_id.is_none()
                    || self.resolution_message_id.is_none()
                    || self.resolved_at.is_none() =>
            {
                Err(invalid_input(
                    "task_gate.state",
                    "resolved gates require resolution fields",
                ))
            }
            TaskGateState::Superseded
                if self.resolved_by_actor_id.is_none()
                    || self.resolved_at.is_none()
                    || self.resolution_message_id.is_some() =>
            {
                Err(invalid_input(
                    "task_gate.state",
                    "superseded gates require only supersession fields",
                ))
            }
            _ => Ok(()),
        }
    }
}

/// Kind of durable human task message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMessageKind {
    /// Answer to a clarification/approval/recovery gate.
    HumanAnswer,
    /// Request for a changed result from Review.
    HumanChangeRequest,
    /// Optional human note accompanying Retry.
    RetryNote,
}

impl TaskMessageKind {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HumanAnswer => "human_answer",
            Self::HumanChangeRequest => "human_change_request",
            Self::RetryNote => "retry_note",
        }
    }
}

impl std::fmt::Display for TaskMessageKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskMessageKind {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "human_answer" => Ok(Self::HumanAnswer),
            "human_change_request" => Ok(Self::HumanChangeRequest),
            "retry_note" => Ok(Self::RetryNote),
            other => Err(invalid_input(
                "task_message.kind",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Durable human input retained as task evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMessageRecord {
    /// Stable message identity.
    pub message_id: TaskMessageId,
    /// Owning task.
    pub task_id: TaskId,
    /// Task generation at append time.
    pub task_generation: u64,
    /// Contract linked to the message, when any.
    pub contract_id: Option<TaskContractId>,
    /// Gate answered by the message, when any.
    pub gate_id: Option<TaskGateId>,
    /// Review amended by the message, when any.
    pub review_id: Option<String>,
    /// Message category.
    pub kind: TaskMessageKind,
    /// Human Markdown body.
    pub body_markdown: String,
    /// Structured Approval decision, when relevant.
    pub approval_decision: Option<ApprovalDecision>,
    /// Author actor.
    pub author_actor_id: String,
    /// Continuation run that consumed this message, when checkpointed.
    pub consumed_by_run_id: Option<String>,
    /// Consumption timestamp.
    pub consumed_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
}
