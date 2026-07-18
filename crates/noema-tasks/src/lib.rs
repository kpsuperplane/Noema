//! Canonical Noema Work domain contracts.
//!
//! This crate owns workflow/stage semantics, task records, immutable execution
//! contracts, gates/messages, run vocabulary, evidence, commands, event
//! vocabulary, and pure transition/reconciliation decisions. SQLite,
//! providers, API projections, and UI remain outside the domain boundary.

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
mod workflow;

pub use command::{
    AcceptTask, AnswerTask, ArchiveProject, CancelTask, CaptureTask, CommandMeta, CreateProject,
    DelegateExecutionIntent, DelegateTask, ProjectPrecondition, QueueTask, ReopenProject,
    ReopenTask, RequestTaskChanges, RetryTask, TaskPrecondition, UpdateInboxTask, UpdateProject,
    WorkCommand, WorkCommandResult,
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
    WorkFailedRunFacts, WorkReconciliationAction, WorkReconciliationSnapshot, WorkTransition,
    WorkTransitionPlan, plan_reconciliation_action, plan_work_transition, reported_failure_facts,
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
    AgentRunHeartbeat, AgentRunRecord, NewAgentRun, RunKind, RunStatus, TASK_EXECUTOR_AGENT_ID,
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
    PERSONAL_CANCELLED_STAGE_ID, PERSONAL_COMPLETED_STAGE_ID, PERSONAL_DOING_STAGE_ID,
    PERSONAL_INBOX_STAGE_ID, PERSONAL_QUEUE_STAGE_ID, PERSONAL_REVIEW_STAGE_ID,
    PERSONAL_WAITING_STAGE_ID, PERSONAL_WORKFLOW_ID, WorkflowDefinition, WorkflowStage,
    WorkflowStageBehavior, personal_stages,
};

#[cfg(test)]
mod wire_tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn closed_work_vocabularies_fail_closed_and_keep_persisted_names() {
        for (kind, wire) in [
            (RunKind::Planner, "planner"),
            (RunKind::Executor, "executor"),
            (RunKind::Reviewer, "reviewer"),
        ] {
            assert_eq!(kind.as_str(), wire);
            assert_eq!(RunKind::from_str(wire).unwrap(), kind);
            assert!(RunKind::from_str("worker").is_err());
        }
        for (behavior, wire) in [
            (WorkflowStageBehavior::Intake, "intake"),
            (WorkflowStageBehavior::Dispatch, "dispatch"),
            (WorkflowStageBehavior::Active, "active"),
            (WorkflowStageBehavior::HumanGate, "human_gate"),
            (WorkflowStageBehavior::Acceptance, "acceptance"),
            (WorkflowStageBehavior::TerminalSuccess, "terminal_success"),
            (
                WorkflowStageBehavior::TerminalCancelled,
                "terminal_cancelled",
            ),
        ] {
            assert_eq!(behavior.as_str(), wire);
            assert_eq!(WorkflowStageBehavior::from_str(wire).unwrap(), behavior);
        }
    }

    #[test]
    fn task_status_and_legacy_event_types_are_not_public_contracts() {
        let task = TaskRecord {
            task_id: TaskId::new("task:1").unwrap(),
            workspace_id: noema_workspaces::WorkspaceId::new("workspace:personal").unwrap(),
            project_id: None,
            workflow_id: WorkflowId::new(PERSONAL_WORKFLOW_ID).unwrap(),
            stage_id: WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).unwrap(),
            title: "Capture".to_string(),
            description_markdown: String::new(),
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::System,
                created_by_actor_id: "actor:system".to_string(),
                ..Default::default()
            },
            generation: 1,
            revision: 1,
            current_contract_id: None,
            active_gate_id: None,
            latest_run_id: None,
            latest_submission_id: None,
            latest_review_id: None,
            accepted_submission_id: None,
            queued_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            completed_at: None,
            cancelled_at: None,
        };
        assert!(task.validate().is_ok());
    }
}
