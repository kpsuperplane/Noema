//! Canonical Noema Work domain contracts.
//!
//! This crate owns workflow/stage semantics, task records, immutable execution
//! contracts, gates/messages, run vocabulary, evidence, commands, event
//! vocabulary, and pure transition/reconciliation decisions. SQLite,
//! providers, API projections, and UI remain outside the domain boundary.

macro_rules! string_enum {
    (
        $(
            $(#[$enum_meta:meta])*
            pub enum $name:ident, $field:literal {
                $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)?
            }
        )+
    ) => {
        $(
            $(#[$enum_meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
            #[allow(missing_docs, reason = "variant names are stable domain vocabulary")]
            pub enum $name {
                $($(#[$variant_meta])* #[serde(rename = $wire)] $variant),+
            }

            impl $name {
                /// Return the stable persisted representation.
                #[must_use]
                pub const fn as_str(self) -> &'static str {
                    match self { $(Self::$variant => $wire),+ }
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str(self.as_str())
                }
            }

            impl std::str::FromStr for $name {
                type Err = crate::WorkDomainError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    match value {
                        $($wire => Ok(Self::$variant)),+,
                        other => Err(crate::error::invalid_input($field, format!("unknown value {other}"))),
                    }
                }
            }
        )+
    };
}

mod command;
mod contract;
mod criteria;
mod error;
mod event;
mod gate;
mod ids;
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
mod workflow;

pub use command::{
    AnswerTask, ArchiveProject, CancelTask, CaptureTask, CommandMeta, CreateProject,
    DelegateExecutionIntent, DelegateTask, ProjectPrecondition, QueueTask, ReopenProject,
    ReopenTask, RetryTask, TaskPrecondition, UpdateInboxTask, UpdateProject, WorkCommand,
    WorkCommandResult,
};
pub use contract::{
    ContractOrigin, ProjectContextSnapshot, TaskContractAmendment, TaskExecutionContract,
    WorkspaceContextSnapshot,
};
pub use criteria::{NewTaskValidationCriterion, TaskValidationCriterion};
pub use error::WorkDomainError;
pub use event::{
    GateResolutionKind, GateSupersessionReason, NotificationDestination, NotificationKind,
    ProjectChangedField, RunCancellationReason, RunTerminalKind, SafeErrorCode, TaskChangedField,
    TaskStageChangeReason, WorkEventContext, WorkEventKind, WorkEventPayload, WorkEventRecord,
};
pub use gate::{
    ApprovalDecision, TaskGateAnswer, TaskGateKind, TaskGateRecord, TaskGateState, TaskMessageKind,
    TaskMessageRecord, TaskRecoveryReason,
};
pub use ids::{
    TaskContractId, TaskGateId, TaskId, TaskMessageId, WorkEventId, WorkflowId, WorkflowStageId,
};
pub use model_pool::{
    NewTaskModelPoolEntry, ProviderDefaultTaskModel, TaskModelPoolEntry,
    is_global_task_model_pool_setting_id, provider_default_task_models,
};
pub use planning::{
    WorkFailedRunFacts, WorkReconciliationAction, WorkReconciliationSnapshot,
    plan_reconciliation_action, reported_failure_facts,
};
pub use policy::{
    DEFAULT_TASK_MAX_ACTIVE_MINUTES, DEFAULT_TASK_MAX_AUTOMATIC_RETRIES,
    DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS, DEFAULT_TASK_MAX_REVIEW_ROUNDS,
    DEFAULT_TASK_MAX_TOOL_CALLS, DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL, MAX_TASK_ACTIVE_MINUTES,
    MAX_TASK_AUTOMATIC_RETRIES, MAX_TASK_PROVIDER_CONTINUATIONS, MAX_TASK_REVIEW_ROUNDS,
    MAX_TASK_TOOL_CALLS, TaskExecutionPolicy,
};
pub use review::{
    CriterionOutcome, NewTaskReview, TaskReviewCriterion, TaskReviewRecord, TaskReviewVerdict,
    validate_review_verdict,
};
pub use run::{
    AgentRunHeartbeat, AgentRunRecord, RunKind, RunStatus, TASK_EXECUTOR_AGENT_ID,
    TASK_REVIEWER_AGENT_ID,
};
pub use state::TaskComplexity;
pub use submission::{
    NewTaskSubmission, SubmissionCriterionEvidence, TaskSubmissionArtifactRecord,
    TaskSubmissionRecord,
};
pub use task::{TaskProvenance, TaskRecord, TaskSourceKind};
pub use transcript::{AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, NewAgentRunItem};
pub use workflow::{
    PERSONAL_CANCELLED_STAGE_ID, PERSONAL_DOING_STAGE_ID, PERSONAL_DONE_STAGE_ID,
    PERSONAL_INBOX_STAGE_ID, PERSONAL_QUEUE_STAGE_ID, PERSONAL_WAITING_STAGE_ID,
    PERSONAL_WORKFLOW_ID, WorkflowDefinition, WorkflowStage, WorkflowStageBehavior,
    personal_stages,
};
