//! Durable task, run, submission, review, and transcript domain contracts.
//!
//! This crate owns pure task semantics. SQLite transactions, leasing, provider
//! execution, delivery, and API presentation remain in their respective
//! infrastructure crates.

macro_rules! task_vocabulary {
    ($type:ident, $kind:literal, {$($variant:ident => $wire:literal),+ $(,)?}) => {
        impl $type {
            /// Return the stable SQLite/API representation.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $type {
            type Err = crate::TaskDomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(crate::TaskDomainError::InvalidEnum {
                        kind: $kind,
                        value: other.to_string(),
                    }),
                }
            }
        }
    };
}

mod criteria;
mod error;
mod event;
mod model_pool;
mod planning;
mod policy;
mod review;
mod run;
mod state;
mod submission;
mod task;
mod transcript;
mod validation;

pub use criteria::{NewTaskValidationCriterion, TaskValidationCriterion};
pub use error::TaskDomainError;
pub use event::{NewRunEvent, NewTaskEvent, TaskEventKind, TaskEventRecord};
pub use model_pool::{
    NewTaskModelPoolEntry, ProviderDefaultTaskModel, TaskModelPoolEntry,
    is_global_task_model_pool_setting_id, provider_default_task_models,
};
pub use planning::{
    AutomaticRecoveryInput, AutomaticRecoveryPlan, ManualContinuationInput, ManualContinuationPlan,
    ReviewPlan, SubmissionPlan, SubmissionState, plan_automatic_recovery, plan_manual_continuation,
    plan_review, plan_submission,
};
pub use policy::{
    DEFAULT_TASK_MAX_ACTIVE_MINUTES, DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS,
    DEFAULT_TASK_MAX_REVIEW_ROUNDS, DEFAULT_TASK_MAX_TOOL_CALLS,
    DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL, MAX_TASK_ACTIVE_MINUTES, MAX_TASK_PROVIDER_CONTINUATIONS,
    MAX_TASK_TOOL_CALLS, TaskExecutionPolicy,
};
pub use review::{
    CriterionOutcome, NewTaskReview, TaskReviewCriterion, TaskReviewRecord, TaskReviewVerdict,
};
pub use run::{
    AgentRunHeartbeat, AgentRunRecord, NewAgentRun, RunKind, RunStatus, TASK_EXECUTOR_AGENT_ID,
    TASK_REVIEWER_AGENT_ID,
};
pub use state::{TaskComplexity, TaskStatus};
pub use submission::{
    NewTaskSubmission, SubmissionCriterionEvidence, TaskSubmissionArtifactRecord,
    TaskSubmissionRecord,
};
pub use task::{NewTask, TaskRecord, TaskSource};
pub use transcript::{AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, NewAgentRunItem};

#[cfg(test)]
mod wire_tests {
    use std::fmt::Debug;

    use serde::{Serialize, de::DeserializeOwned};

    use super::*;

    #[test]
    fn durable_enum_wire_vocabularies_are_stable_and_fail_closed() {
        assert_wire(
            &[
                (TaskComplexity::Simple, "simple"),
                (TaskComplexity::Medium, "medium"),
                (TaskComplexity::Difficult, "difficult"),
            ],
            TaskComplexity::as_str,
        );
        assert_wire(
            &[
                (TaskStatus::Queued, "queued"),
                (TaskStatus::Executing, "executing"),
                (TaskStatus::Reviewing, "reviewing"),
                (TaskStatus::RevisionRequested, "revision_requested"),
                (TaskStatus::WaitingForHuman, "waiting_for_human"),
                (TaskStatus::Completed, "completed"),
                (TaskStatus::Failed, "failed"),
                (TaskStatus::Cancelled, "cancelled"),
            ],
            TaskStatus::as_str,
        );
        assert_wire(
            &[
                (RunKind::Executor, "executor"),
                (RunKind::Reviewer, "reviewer"),
            ],
            RunKind::as_str,
        );
        assert_wire(
            &[
                (RunStatus::Queued, "queued"),
                (RunStatus::Leased, "leased"),
                (RunStatus::Running, "running"),
                (RunStatus::Completed, "completed"),
                (RunStatus::WaitingForApproval, "waiting_for_approval"),
                (RunStatus::Interrupted, "interrupted"),
                (RunStatus::Failed, "failed"),
                (RunStatus::Cancelled, "cancelled"),
            ],
            RunStatus::as_str,
        );
        assert_wire(
            &[
                (AgentRunItemKind::ModelInput, "model_input"),
                (AgentRunItemKind::AssistantOutput, "assistant_output"),
                (AgentRunItemKind::ToolCall, "tool_call"),
                (AgentRunItemKind::ToolResult, "tool_result"),
                (AgentRunItemKind::ProgressNotice, "progress_notice"),
                (AgentRunItemKind::ContextCheckpoint, "context_checkpoint"),
                (AgentRunItemKind::TaskSubmission, "task_submission"),
                (AgentRunItemKind::TaskReview, "task_review"),
                (AgentRunItemKind::ArtifactReference, "artifact_reference"),
                (AgentRunItemKind::Failure, "failure"),
                (AgentRunItemKind::Cancellation, "cancellation"),
            ],
            AgentRunItemKind::as_str,
        );
        assert_wire(
            &[
                (AgentRunItemStatus::Pending, "pending"),
                (AgentRunItemStatus::Running, "running"),
                (AgentRunItemStatus::Completed, "completed"),
                (AgentRunItemStatus::Failed, "failed"),
                (AgentRunItemStatus::Cancelled, "cancelled"),
                (AgentRunItemStatus::Skipped, "skipped"),
            ],
            AgentRunItemStatus::as_str,
        );
        assert_wire(
            &[
                (TaskReviewVerdict::Approve, "approve"),
                (TaskReviewVerdict::RequestChanges, "request_changes"),
                (TaskReviewVerdict::NeedsHuman, "needs_human"),
            ],
            TaskReviewVerdict::as_str,
        );
        assert_wire(
            &[
                (CriterionOutcome::Pass, "pass"),
                (CriterionOutcome::Fail, "fail"),
                (CriterionOutcome::Uncertain, "uncertain"),
            ],
            CriterionOutcome::as_str,
        );
    }

    fn assert_wire<T>(values: &[(T, &'static str)], as_str: impl Fn(T) -> &'static str)
    where
        T: Copy + Debug + PartialEq + Serialize + DeserializeOwned + std::str::FromStr,
        T::Err: Debug,
    {
        for &(value, wire) in values {
            assert_eq!(as_str(value), wire);
            assert_eq!(wire.parse::<T>().expect("known wire value"), value);
            let serialized = serde_json::to_string(&value).expect("serialize wire");
            assert_eq!(serialized, format!("\"{wire}\""));
            assert_eq!(
                serde_json::from_str::<T>(&serialized).expect("deserialize wire"),
                value
            );
        }
        assert!("__unknown__".parse::<T>().is_err());
        assert!(serde_json::from_str::<T>("\"__unknown__\"").is_err());
    }
}
