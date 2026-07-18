use serde::{Deserialize, Serialize};

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

task_vocabulary!(TaskComplexity, "task_complexity", {
    Simple => "simple",
    Medium => "medium",
    Difficult => "difficult",
});

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

task_vocabulary!(TaskStatus, "task_status", {
    Queued => "queued",
    Executing => "executing",
    Reviewing => "reviewing",
    RevisionRequested => "revision_requested",
    WaitingForHuman => "waiting_for_human",
    Completed => "completed",
    Failed => "failed",
    Cancelled => "cancelled",
});

#[cfg(test)]
mod tests {
    use super::TaskStatus;
    use crate::RunStatus;

    #[test]
    fn failed_tasks_remain_resumable_but_completed_and_cancelled_are_closed() {
        assert!(TaskStatus::Reviewing.can_transition_to(TaskStatus::Completed));
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Queued));
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Reviewing));
        assert!(!RunStatus::Interrupted.can_transition_to(RunStatus::Queued));
        assert!(!RunStatus::Completed.can_transition_to(RunStatus::Running));
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
}
