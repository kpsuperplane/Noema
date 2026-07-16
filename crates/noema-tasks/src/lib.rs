//! Durable task, run, submission, review, and transcript domain contracts.
//!
//! This crate owns pure task semantics. SQLite transactions, leasing, provider
//! execution, delivery, and API presentation remain in their respective
//! infrastructure crates.

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
