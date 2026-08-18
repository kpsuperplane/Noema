use serde::{Deserialize, Serialize};

use crate::{
    GateResolutionKind, RunKind, TaskGateId, TaskId, TaskMessageId, WorkDomainError,
    error::invalid_input,
};

string_enum! {
    /// Structured result of an Approval gate.
    pub enum ApprovalDecision, "approval_decision" {
        /// The governed action may continue.
        Approved => "approved",
        /// The governed action must not continue.
        Declined => "declined",
    }


    /// Human intervention category persisted on a task gate.
    pub enum TaskGateKind, "task_gate.kind" {
        /// Missing facts or an ambiguous choice.
        Clarification => "clarification",
        /// A governed decision required by policy.
        Approval => "approval",
        /// Retry/revision/recovery decision after automated work stopped.
        Recovery => "recovery",
    }
}

impl TaskGateKind {
    /// Whether this gate authorizes one resolution kind.
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
                GateResolutionKind::Retry,
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

pub(crate) const fn recovery_fields_are_valid(
    gate: TaskGateKind,
    reason: Option<TaskRecoveryReason>,
    retry: Option<RunKind>,
) -> bool {
    matches!(
        (gate, reason, retry),
        (
            TaskGateKind::Clarification | TaskGateKind::Approval,
            None,
            None
        ) | (
            TaskGateKind::Recovery,
            Some(TaskRecoveryReason::InvariantFault),
            None
        ) | (
            TaskGateKind::Recovery,
            Some(TaskRecoveryReason::ReviewRoundsExhausted),
            Some(RunKind::Executor)
        ) | (
            TaskGateKind::Recovery,
            Some(
                TaskRecoveryReason::InfrastructureRetriesExhausted
                    | TaskRecoveryReason::UnsafeEffectUncertain
                    | TaskRecoveryReason::ConfigurationUnavailable
            ),
            Some(_)
        )
    )
}

string_enum! {
    /// Closed reason for a Recovery gate.
    pub enum TaskRecoveryReason, "task_gate.recovery_reason" {
        /// Automatic infrastructure retry bound was exhausted.
        InfrastructureRetriesExhausted => "infrastructure_retries_exhausted",
        /// Automated reviewer round bound was exhausted.
        ReviewRoundsExhausted => "review_rounds_exhausted",
        /// An external effect may have occurred and cannot be replayed safely.
        UnsafeEffectUncertain => "unsafe_effect_uncertain",
        /// Required provider/model/policy configuration is unavailable.
        ConfigurationUnavailable => "configuration_unavailable",
        /// Durable facts are inconsistent and require a human decision.
        InvariantFault => "invariant_fault",
    }


    /// Gate lifecycle state.
    pub enum TaskGateState, "task_gate.state" {
        /// Awaiting a person.
        Open => "open",
        /// Resolved by an answer or retry.
        Resolved => "resolved",
        /// Superseded by cancellation or a newer generation.
        Superseded => "superseded",
    }
}

/// Structured answer to a gate; approval is never inferred from prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskGateAnswer {
    pub message_markdown: String,
    pub approval_decision: Option<ApprovalDecision>,
}

impl TaskGateAnswer {
    /// Validate answer requirements for one gate kind.
    /// # Errors
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
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskGateRecord {
    pub gate_id: TaskGateId,
    pub task_id: TaskId,
    pub task_generation: u64,
    pub kind: TaskGateKind,
    pub state: TaskGateState,
    pub recovery_reason: Option<TaskRecoveryReason>,
    pub retry_run_kind: Option<RunKind>,
    pub prompt_markdown: String,
    pub context_markdown: String,
    pub suggested_answers: Vec<String>,
    pub opened_by_actor_id: String,
    pub originating_run_id: Option<String>,
    pub resolved_by_actor_id: Option<String>,
    pub resolution_message_id: Option<TaskMessageId>,
    pub opened_at: String,
    pub resolved_at: Option<String>,
}

impl TaskGateRecord {
    /// Validate closed gate/recovery combinations and required fields.
    /// # Errors
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
        if self.suggested_answers.len() > 8
            || self
                .suggested_answers
                .iter()
                .any(|answer| answer.trim().is_empty() || answer.chars().count() > 1_000)
        {
            return Err(invalid_input(
                "task_gate.suggested_answers",
                "at most eight nonblank answers of 1,000 characters are allowed",
            ));
        }
        if self.kind == TaskGateKind::Recovery && !self.suggested_answers.is_empty() {
            return Err(invalid_input(
                "task_gate.suggested_answers",
                "recovery gates cannot suggest answers",
            ));
        }
        if self.kind == TaskGateKind::Recovery && self.recovery_reason.is_none() {
            return Err(invalid_input(
                "task_gate.recovery_reason",
                "recovery gates require a reason",
            ));
        }
        if !recovery_fields_are_valid(self.kind, self.recovery_reason, self.retry_run_kind) {
            let (field, message) = match self.kind {
                TaskGateKind::Recovery => (
                    "task_gate.retry_run_kind",
                    "recovery reason and continuation role are inconsistent",
                ),
                TaskGateKind::Clarification | TaskGateKind::Approval => {
                    ("task_gate", "only recovery gates carry recovery fields")
                }
            };
            return Err(invalid_input(field, message));
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

string_enum! {
    /// Kind of durable human task message.
    pub enum TaskMessageKind, "task_message.kind" {
        /// Answer to a clarification/approval/recovery gate.
        HumanAnswer => "human_answer",
        /// Request for a changed result from Review.
        HumanChangeRequest => "human_change_request",
        /// Optional human note accompanying Retry.
        RetryNote => "retry_note",
    }
}

/// Durable human input retained as task evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskMessageRecord {
    pub message_id: TaskMessageId,
    pub task_id: TaskId,
    pub task_generation: u64,
    pub gate_id: Option<TaskGateId>,
    pub kind: TaskMessageKind,
    pub body_markdown: String,
    pub approval_decision: Option<ApprovalDecision>,
    pub author_actor_id: String,
    pub consumed_by_run_id: Option<String>,
    pub consumed_at: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_review_rounds_accept_retry_guidance_only() {
        let allows = |resolution| {
            TaskGateKind::Recovery.allows_resolution(
                Some(TaskRecoveryReason::ReviewRoundsExhausted),
                Some(RunKind::Executor),
                resolution,
            )
        };

        assert!(allows(GateResolutionKind::Retry));
        assert!(!allows(GateResolutionKind::Answer));
    }
}
