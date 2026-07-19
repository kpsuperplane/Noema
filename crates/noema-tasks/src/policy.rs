use serde::{Deserialize, Serialize};

use crate::{WorkDomainError, error::invalid_input};

/// Default provider continuation bound.
pub const DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS: u32 = 80;
/// Default tool-call bound.
pub const DEFAULT_TASK_MAX_TOOL_CALLS: u32 = 400;
/// Default active execution bound in minutes.
pub const DEFAULT_TASK_MAX_ACTIVE_MINUTES: u32 = 120;
/// Default progress-audit interval in provider continuations.
pub const DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL: u32 = 20;
/// Default automatic infrastructure retry bound.
pub const DEFAULT_TASK_MAX_AUTOMATIC_RETRIES: u32 = 3;
/// Default reviewed submission/revision bound.
pub const DEFAULT_TASK_MAX_REVIEW_ROUNDS: u32 = 3;
/// Hard provider continuation ceiling.
pub const MAX_TASK_PROVIDER_CONTINUATIONS: u32 = 1_000;
/// Hard tool-call ceiling.
pub const MAX_TASK_TOOL_CALLS: u32 = 10_000;
/// Hard active execution ceiling in minutes.
pub const MAX_TASK_ACTIVE_MINUTES: u32 = 10_080;
/// Hard automatic retry ceiling.
pub const MAX_TASK_AUTOMATIC_RETRIES: u32 = 20;
/// Hard review-round ceiling.
pub const MAX_TASK_REVIEW_ROUNDS: u32 = 20;

/// Immutable provider-independent execution policy snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskExecutionPolicy {
    pub max_provider_continuations: u32,
    pub max_tool_calls: u32,
    pub max_active_minutes: u32,
    pub progress_audit_interval: u32,
    pub max_automatic_retries: u32,
    pub max_review_rounds: u32,
}

impl Default for TaskExecutionPolicy {
    fn default() -> Self {
        Self {
            max_provider_continuations: DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS,
            max_tool_calls: DEFAULT_TASK_MAX_TOOL_CALLS,
            max_active_minutes: DEFAULT_TASK_MAX_ACTIVE_MINUTES,
            progress_audit_interval: DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL,
            max_automatic_retries: DEFAULT_TASK_MAX_AUTOMATIC_RETRIES,
            max_review_rounds: DEFAULT_TASK_MAX_REVIEW_ROUNDS,
        }
    }
}

impl TaskExecutionPolicy {
    /// Validate all persisted safety bounds.
    /// # Errors
    /// Returns [`WorkDomainError`] when a configured limit exceeds its hard
    /// bound, a positive limit is zero, or the audit interval exceeds the
    /// provider continuation limit.
    pub fn validated(self) -> Result<Self, WorkDomainError> {
        if !(1..=MAX_TASK_PROVIDER_CONTINUATIONS).contains(&self.max_provider_continuations) {
            return Err(invalid_input(
                "execution_policy.max_provider_continuations",
                "out of bounds",
            ));
        }
        if !(1..=MAX_TASK_TOOL_CALLS).contains(&self.max_tool_calls) {
            return Err(invalid_input(
                "execution_policy.max_tool_calls",
                "out of bounds",
            ));
        }
        if !(1..=MAX_TASK_ACTIVE_MINUTES).contains(&self.max_active_minutes) {
            return Err(invalid_input(
                "execution_policy.max_active_minutes",
                "out of bounds",
            ));
        }
        if !(1..=self.max_provider_continuations).contains(&self.progress_audit_interval) {
            return Err(invalid_input(
                "execution_policy.progress_audit_interval",
                "must be positive and no larger than provider continuations",
            ));
        }
        if self.max_automatic_retries > MAX_TASK_AUTOMATIC_RETRIES {
            return Err(invalid_input(
                "execution_policy.max_automatic_retries",
                "out of bounds",
            ));
        }
        if !(1..=MAX_TASK_REVIEW_ROUNDS).contains(&self.max_review_rounds) {
            return Err(invalid_input(
                "execution_policy.max_review_rounds",
                "out of bounds",
            ));
        }
        Ok(self)
    }
}
