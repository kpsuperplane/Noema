use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::TaskDomainError;

/// A bounded complexity tier chosen for a delegated task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    /// Small, low-risk work.
    Simple,
    /// Typical multi-step work.
    Medium,
    /// Large or reasoning-intensive work.
    Difficult,
}

impl TaskComplexity {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Difficult => "difficult",
        }
    }
}

impl fmt::Display for TaskComplexity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TaskComplexity {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "simple" => Ok(Self::Simple),
            "medium" => Ok(Self::Medium),
            "difficult" => Ok(Self::Difficult),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "task_complexity",
                value: other.to_string(),
            }),
        }
    }
}

/// User-visible task workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task and its first executor run have been committed but not claimed.
    Queued,
    /// An executor run currently owns the task lease.
    Executing,
    /// A submission is awaiting independent review.
    Reviewing,
    /// Review requested another executor submission.
    RevisionRequested,
    /// Safe progress requires human clarification, approval, or intervention.
    WaitingForHuman,
    /// A reviewer approved every criterion.
    Completed,
    /// No safe continuation is available.
    Failed,
    /// A human or source deletion policy cancelled the task.
    Cancelled,
}

impl TaskStatus {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Executing => "executing",
            Self::Reviewing => "reviewing",
            Self::RevisionRequested => "revision_requested",
            Self::WaitingForHuman => "waiting_for_human",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Return whether this is a delivery-terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Validate an ordinary product state transition.
    ///
    /// Operation-specific planners authorize the exceptional reviewer,
    /// recovery, and failed-task continuation transitions.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Queued,
                Self::Executing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::Executing,
                Self::Reviewing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::Reviewing,
                Self::RevisionRequested
                    | Self::Completed
                    | Self::WaitingForHuman
                    | Self::Failed
                    | Self::Cancelled
            ) | (
                Self::RevisionRequested,
                Self::Executing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::WaitingForHuman,
                Self::Queued | Self::Executing | Self::Failed | Self::Cancelled
            ) | (Self::Failed, Self::Cancelled)
        )
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "executing" => Ok(Self::Executing),
            "reviewing" => Ok(Self::Reviewing),
            "revision_requested" => Ok(Self::RevisionRequested),
            "waiting_for_human" => Ok(Self::WaitingForHuman),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "task_status",
                value: other.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskComplexity, TaskStatus};
    use crate::RunStatus;

    #[test]
    fn task_transition_matrix_rejects_terminal_reopening() {
        assert!(TaskStatus::Reviewing.can_transition_to(TaskStatus::Completed));
        assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::Queued));
        assert!(!RunStatus::Interrupted.can_transition_to(RunStatus::Queued));
        assert!(!RunStatus::Completed.can_transition_to(RunStatus::Running));
    }

    #[test]
    fn failed_tasks_remain_resumable_but_completed_and_cancelled_are_closed() {
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Queued));
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Reviewing));
        for terminal in [TaskStatus::Completed, TaskStatus::Cancelled] {
            for next in [
                TaskStatus::Queued,
                TaskStatus::Executing,
                TaskStatus::Reviewing,
                TaskStatus::RevisionRequested,
                TaskStatus::WaitingForHuman,
                TaskStatus::Completed,
                TaskStatus::Failed,
                TaskStatus::Cancelled,
            ] {
                assert!(!terminal.can_transition_to(next));
            }
        }
    }

    #[test]
    fn task_status_wire_values_are_stable_and_unknown_values_fail_closed() {
        for (status, wire) in [
            (TaskStatus::Queued, "queued"),
            (TaskStatus::Executing, "executing"),
            (TaskStatus::Reviewing, "reviewing"),
            (TaskStatus::RevisionRequested, "revision_requested"),
            (TaskStatus::WaitingForHuman, "waiting_for_human"),
            (TaskStatus::Completed, "completed"),
            (TaskStatus::Failed, "failed"),
            (TaskStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(status.as_str(), wire);
            assert_eq!(wire.parse::<TaskStatus>().expect("known status"), status);
            assert_eq!(
                serde_json::from_str::<TaskStatus>(
                    &serde_json::to_string(&status).expect("serialize status")
                )
                .expect("deserialize status"),
                status
            );
        }
        assert!("future_status".parse::<TaskStatus>().is_err());
        assert!(serde_json::from_str::<TaskStatus>("\"future_status\"").is_err());
    }

    #[test]
    fn task_complexity_wire_values_are_stable_and_fail_closed() {
        for (value, wire) in [
            (TaskComplexity::Simple, "simple"),
            (TaskComplexity::Medium, "medium"),
            (TaskComplexity::Difficult, "difficult"),
        ] {
            assert_eq!(value.as_str(), wire);
            assert_eq!(
                wire.parse::<TaskComplexity>().expect("known complexity"),
                value
            );
            assert_eq!(
                serde_json::from_str::<TaskComplexity>(
                    &serde_json::to_string(&value).expect("serialize complexity")
                )
                .expect("deserialize complexity"),
                value
            );
        }
        assert!("future".parse::<TaskComplexity>().is_err());
        assert!(serde_json::from_str::<TaskComplexity>("\"future\"").is_err());
    }
}
