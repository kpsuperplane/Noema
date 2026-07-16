use serde::{Deserialize, Serialize};

use crate::TaskDomainError;

/// Default maximum number of reviewed executor submissions for one task.
pub const DEFAULT_TASK_MAX_REVIEW_ROUNDS: i64 = 3;
/// Default provider continuation safety ceiling for every task run.
pub const DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS: i64 = 80;
/// Default tool-call safety ceiling for every task run.
pub const DEFAULT_TASK_MAX_TOOL_CALLS: i64 = 400;
/// Default active execution safety ceiling, in minutes, for every task run.
pub const DEFAULT_TASK_MAX_ACTIVE_MINUTES: i64 = 120;
/// Default interval between task progress audits, measured in continuations.
pub const DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL: i64 = 20;
/// Hard upper bound for the configurable provider continuation ceiling.
pub const MAX_TASK_PROVIDER_CONTINUATIONS: i64 = 1_000;
/// Hard upper bound for the configurable tool-call ceiling.
pub const MAX_TASK_TOOL_CALLS: i64 = 10_000;
/// Hard upper bound for active execution time (seven days).
pub const MAX_TASK_ACTIVE_MINUTES: i64 = 10_080;

/// Provider-independent execution safety policy shared by every task model tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionPolicy {
    /// Maximum provider continuations before terminal-only finalization.
    pub max_provider_continuations: i64,
    /// Maximum tool calls before terminal-only finalization.
    pub max_tool_calls: i64,
    /// Maximum active execution time, excluding queue time.
    pub max_active_minutes: i64,
    /// Continuation interval between progress audits.
    pub progress_audit_interval: i64,
}

impl Default for TaskExecutionPolicy {
    fn default() -> Self {
        Self {
            max_provider_continuations: DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS,
            max_tool_calls: DEFAULT_TASK_MAX_TOOL_CALLS,
            max_active_minutes: DEFAULT_TASK_MAX_ACTIVE_MINUTES,
            progress_audit_interval: DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL,
        }
    }
}

impl TaskExecutionPolicy {
    /// Validate values before persisting a policy or run snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError::InvalidExecutionPolicy`] for invalid bounds.
    pub fn validated(self) -> Result<Self, TaskDomainError> {
        if !(1..=MAX_TASK_PROVIDER_CONTINUATIONS).contains(&self.max_provider_continuations) {
            return Err(TaskDomainError::InvalidExecutionPolicy {
                message: format!(
                    "maximum provider continuations must be between 1 and {MAX_TASK_PROVIDER_CONTINUATIONS}"
                ),
            });
        }
        if !(1..=MAX_TASK_TOOL_CALLS).contains(&self.max_tool_calls) {
            return Err(TaskDomainError::InvalidExecutionPolicy {
                message: format!("maximum tool calls must be between 1 and {MAX_TASK_TOOL_CALLS}"),
            });
        }
        if !(1..=MAX_TASK_ACTIVE_MINUTES).contains(&self.max_active_minutes) {
            return Err(TaskDomainError::InvalidExecutionPolicy {
                message: format!(
                    "maximum active minutes must be between 1 and {MAX_TASK_ACTIVE_MINUTES}"
                ),
            });
        }
        if !(1..=self.max_provider_continuations).contains(&self.progress_audit_interval) {
            return Err(TaskDomainError::InvalidExecutionPolicy {
                message: "progress audit interval must be positive and no larger than the continuation limit"
                    .to_string(),
            });
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_policy_enforces_every_bound_and_audit_relationship() {
        for valid in [
            TaskExecutionPolicy {
                max_provider_continuations: 1,
                max_tool_calls: 1,
                max_active_minutes: 1,
                progress_audit_interval: 1,
            },
            TaskExecutionPolicy {
                max_provider_continuations: MAX_TASK_PROVIDER_CONTINUATIONS,
                max_tool_calls: MAX_TASK_TOOL_CALLS,
                max_active_minutes: MAX_TASK_ACTIVE_MINUTES,
                progress_audit_interval: MAX_TASK_PROVIDER_CONTINUATIONS,
            },
            TaskExecutionPolicy::default(),
        ] {
            assert!(valid.validated().is_ok());
        }
        for invalid in [
            TaskExecutionPolicy {
                max_provider_continuations: 0,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                max_provider_continuations: MAX_TASK_PROVIDER_CONTINUATIONS + 1,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                max_tool_calls: 0,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                max_tool_calls: MAX_TASK_TOOL_CALLS + 1,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                max_active_minutes: 0,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                max_active_minutes: MAX_TASK_ACTIVE_MINUTES + 1,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                progress_audit_interval: 0,
                ..TaskExecutionPolicy::default()
            },
            TaskExecutionPolicy {
                progress_audit_interval: DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS + 1,
                ..TaskExecutionPolicy::default()
            },
        ] {
            assert!(invalid.validated().is_err());
        }
    }
}
