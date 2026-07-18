use serde::{Deserialize, Serialize};

use crate::{WorkDomainError, error::invalid_input};

/// Closed event vocabulary for the single global Work ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkEventKind {
    #[serde(rename = "project.created")]
    ProjectCreated,
    #[serde(rename = "project.updated")]
    ProjectUpdated,
    #[serde(rename = "project.archived")]
    ProjectArchived,
    #[serde(rename = "project.reopened")]
    ProjectReopened,
    #[serde(rename = "task.captured")]
    TaskCaptured,
    #[serde(rename = "task.updated")]
    TaskUpdated,
    #[serde(rename = "task.queued")]
    TaskQueued,
    #[serde(rename = "task.stage_changed")]
    TaskStageChanged,
    #[serde(rename = "task.cancelled")]
    TaskCancelled,
    #[serde(rename = "task.reopened")]
    TaskReopened,
    #[serde(rename = "task.accepted")]
    TaskAccepted,
    #[serde(rename = "contract.created")]
    ContractCreated,
    #[serde(rename = "gate.opened")]
    GateOpened,
    #[serde(rename = "gate.resolved")]
    GateResolved,
    #[serde(rename = "gate.superseded")]
    GateSuperseded,
    #[serde(rename = "task.message_appended")]
    TaskMessageAppended,
    #[serde(rename = "task.message_consumed")]
    TaskMessageConsumed,
    #[serde(rename = "run.queued")]
    RunQueued,
    #[serde(rename = "run.claimed")]
    RunClaimed,
    #[serde(rename = "run.started")]
    RunStarted,
    #[serde(rename = "run.heartbeat")]
    RunHeartbeat,
    #[serde(rename = "run.completed")]
    RunCompleted,
    #[serde(rename = "run.waiting_for_approval")]
    RunWaitingForApproval,
    #[serde(rename = "run.interrupted")]
    RunInterrupted,
    #[serde(rename = "run.failed")]
    RunFailed,
    #[serde(rename = "run.cancel_requested")]
    RunCancelRequested,
    #[serde(rename = "run.cancelled")]
    RunCancelled,
    #[serde(rename = "submission.created")]
    SubmissionCreated,
    #[serde(rename = "review.created")]
    ReviewCreated,
    #[serde(rename = "notification.queued")]
    NotificationQueued,
    #[serde(rename = "notification.delivered")]
    NotificationDelivered,
    #[serde(rename = "notification.failed")]
    NotificationFailed,
}

impl WorkEventKind {
    /// Return the exact persisted event name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectCreated => "project.created",
            Self::ProjectUpdated => "project.updated",
            Self::ProjectArchived => "project.archived",
            Self::ProjectReopened => "project.reopened",
            Self::TaskCaptured => "task.captured",
            Self::TaskUpdated => "task.updated",
            Self::TaskQueued => "task.queued",
            Self::TaskStageChanged => "task.stage_changed",
            Self::TaskCancelled => "task.cancelled",
            Self::TaskReopened => "task.reopened",
            Self::TaskAccepted => "task.accepted",
            Self::ContractCreated => "contract.created",
            Self::GateOpened => "gate.opened",
            Self::GateResolved => "gate.resolved",
            Self::GateSuperseded => "gate.superseded",
            Self::TaskMessageAppended => "task.message_appended",
            Self::TaskMessageConsumed => "task.message_consumed",
            Self::RunQueued => "run.queued",
            Self::RunClaimed => "run.claimed",
            Self::RunStarted => "run.started",
            Self::RunHeartbeat => "run.heartbeat",
            Self::RunCompleted => "run.completed",
            Self::RunWaitingForApproval => "run.waiting_for_approval",
            Self::RunInterrupted => "run.interrupted",
            Self::RunFailed => "run.failed",
            Self::RunCancelRequested => "run.cancel_requested",
            Self::RunCancelled => "run.cancelled",
            Self::SubmissionCreated => "submission.created",
            Self::ReviewCreated => "review.created",
            Self::NotificationQueued => "notification.queued",
            Self::NotificationDelivered => "notification.delivered",
            Self::NotificationFailed => "notification.failed",
        }
    }
}

impl std::fmt::Display for WorkEventKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for WorkEventKind {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ALL_EVENT_KINDS
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| invalid_input("work_event.kind", format!("unknown value {value}")))
    }
}

const ALL_EVENT_KINDS: &[WorkEventKind] = &[
    WorkEventKind::ProjectCreated,
    WorkEventKind::ProjectUpdated,
    WorkEventKind::ProjectArchived,
    WorkEventKind::ProjectReopened,
    WorkEventKind::TaskCaptured,
    WorkEventKind::TaskUpdated,
    WorkEventKind::TaskQueued,
    WorkEventKind::TaskStageChanged,
    WorkEventKind::TaskCancelled,
    WorkEventKind::TaskReopened,
    WorkEventKind::TaskAccepted,
    WorkEventKind::ContractCreated,
    WorkEventKind::GateOpened,
    WorkEventKind::GateResolved,
    WorkEventKind::GateSuperseded,
    WorkEventKind::TaskMessageAppended,
    WorkEventKind::TaskMessageConsumed,
    WorkEventKind::RunQueued,
    WorkEventKind::RunClaimed,
    WorkEventKind::RunStarted,
    WorkEventKind::RunHeartbeat,
    WorkEventKind::RunCompleted,
    WorkEventKind::RunWaitingForApproval,
    WorkEventKind::RunInterrupted,
    WorkEventKind::RunFailed,
    WorkEventKind::RunCancelRequested,
    WorkEventKind::RunCancelled,
    WorkEventKind::SubmissionCreated,
    WorkEventKind::ReviewCreated,
    WorkEventKind::NotificationQueued,
    WorkEventKind::NotificationDelivered,
    WorkEventKind::NotificationFailed,
];

/// Safe, bounded error code retained in the event ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SafeErrorCode(String);

impl SafeErrorCode {
    /// Validate a lower-snake-case error code without prose or secrets.
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
