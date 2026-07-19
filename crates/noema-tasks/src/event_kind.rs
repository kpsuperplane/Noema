use serde::{Deserialize, Serialize};

use crate::{WorkDomainError, error::invalid_input};

string_enum! {
    /// Closed event vocabulary for the single global Work ledger.
    #[allow(missing_docs, reason = "variant names mirror the closed ledger vocabulary")]
    pub enum WorkEventKind, "work_event.kind" {
        ProjectCreated => "project.created", ProjectUpdated => "project.updated",
        ProjectArchived => "project.archived", ProjectReopened => "project.reopened",
        TaskCaptured => "task.captured", TaskUpdated => "task.updated",
        TaskQueued => "task.queued", TaskStageChanged => "task.stage_changed",
        TaskCancelled => "task.cancelled", TaskReopened => "task.reopened",
        TaskAccepted => "task.accepted", ContractCreated => "contract.created",
        GateOpened => "gate.opened", GateResolved => "gate.resolved",
        GateSuperseded => "gate.superseded", TaskMessageAppended => "task.message_appended",
        TaskMessageConsumed => "task.message_consumed", RunQueued => "run.queued",
        RunClaimed => "run.claimed", RunStarted => "run.started",
        RunHeartbeat => "run.heartbeat", RunCompleted => "run.completed",
        RunWaitingForApproval => "run.waiting_for_approval", RunInterrupted => "run.interrupted",
        RunFailed => "run.failed", RunCancelRequested => "run.cancel_requested",
        RunCancelled => "run.cancelled", SubmissionCreated => "submission.created",
        ReviewCreated => "review.created", NotificationQueued => "notification.queued",
        NotificationDelivered => "notification.delivered", NotificationFailed => "notification.failed",
    }
}

/// Safe, bounded error code retained in the event ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SafeErrorCode(String);

impl SafeErrorCode {
    /// Validate a lower-snake-case error code without prose or secrets.
    /// # Errors
    /// Returns [`WorkDomainError`] when the code is empty, longer than 64
    /// bytes, or is not canonical lower snake case.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkDomainError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || value.starts_with('_')
            || value.ends_with('_')
            || value.contains("__")
            || value.chars().any(|character| {
                !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_')
            })
        {
            return Err(invalid_input(
                "event.error_code",
                "error code must be lower snake case and at most 64 bytes",
            ));
        }
        Ok(Self(value))
    }

    /// Borrow the stable safe code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SafeErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for SafeErrorCode {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for SafeErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}
