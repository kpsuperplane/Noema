//! Canonical Noema Work domain contracts.
//!
//! This crate owns workflow and stage semantics, Task records, gates, run
//! vocabulary, commands, events, and pure reconciliation decisions. SQLite,
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
mod error;
mod event;
mod execution;
mod gate;
mod ids;
mod model_pool;
mod planning;
mod policy;
mod review;
mod run;
mod schedule;
mod state;
mod task;
mod transcript;
mod validation;
mod workflow;

pub use command::{
    AnswerTask, ArchiveProject, CancelTask, CaptureTask, ChangeTaskRecurrence, CommandMeta,
    CreateProject, DelegateExecutionIntent, DelegateTask, ProjectPrecondition, QueueTask,
    RecurrencePrecondition, ReopenProject, ReopenTask, RetryTask, RunScheduledTaskNow,
    RunTaskRecurrenceNow, ScheduleTask, TaskPrecondition, UnscheduleTask, UpdateInboxTask,
    UpdateProject, UpdateTaskRecurrence, WorkCommand, WorkCommandResult,
};
pub use error::WorkDomainError;
pub use event::{
    GateResolutionKind, GateSupersessionReason, NotificationDestination, NotificationKind,
    ProjectChangedField, RunCancellationReason, RunTerminalKind, SafeErrorCode, TaskChangedField,
    TaskStageChangeReason, WorkEventContext, WorkEventKind, WorkEventPayload, WorkEventRecord,
};
pub use execution::{
    AcpExecutorLaunch, ProjectRunContext, TaskExecutorBackend, TaskExecutorSelection,
    TaskReopenDirection, WorkspaceRunContext,
};
pub use gate::{
    ApprovalDecision, TaskGateAnswer, TaskGateKind, TaskGateRecord, TaskGateState, TaskMessageKind,
    TaskMessageRecord, TaskRecoveryReason,
};
pub use ids::{TaskGateId, TaskId, TaskMessageId, WorkEventId, WorkflowId, WorkflowStageId};
pub use model_pool::{
    NewTaskModelPoolEntry, TaskModelPoolEntry, is_global_task_model_pool_setting_id, model_use_case,
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
pub use review::TaskReviewVerdict;
pub use run::{
    AgentRunHeartbeat, AgentRunRecord, RunKind, RunStatus, TASK_EXECUTOR_AGENT_ID,
    TASK_REVIEWER_AGENT_ID,
};
pub use schedule::{
    MissedRunPolicy, NewTaskRecurrence, NewTaskSchedule, OverlapPolicy, RecurrenceCommandKind,
    RecurrenceLifecycle, RecurrenceOccurrenceRecord, RecurrenceOccurrenceResolution,
    RecurrenceOccurrenceTrigger, TaskRecurrenceId, TaskRecurrenceRecord,
    next_recurrence_at_or_after, parse_utc_instant, recurrence_local_slot, recurrence_preview,
};
pub use state::TaskComplexity;
pub use task::{
    TASK_AUTHORIZATION_CONTEXT_MAX_MESSAGES, TaskAuthorizationContext, TaskAuthorizationMessage,
    TaskAuthorizationMessageRole, TaskProvenance, TaskRecord, TaskSourceKind,
};
pub use transcript::{AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, NewAgentRunItem};
pub use workflow::{
    PERSONAL_CANCELLED_STAGE_ID, PERSONAL_DOING_STAGE_ID, PERSONAL_DONE_STAGE_ID,
    PERSONAL_INBOX_STAGE_ID, PERSONAL_QUEUE_STAGE_ID, PERSONAL_WAITING_STAGE_ID,
    PERSONAL_WORKFLOW_ID, WorkflowDefinition, WorkflowStage, WorkflowStageBehavior, personal_stage,
    personal_stages, personal_workflow,
};
