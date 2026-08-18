use serde_json::{Value, json};

use crate::{
    ContractOrigin, RunKind, TaskComplexity, TaskContractId, TaskGateId, TaskGateKind,
    TaskMessageId, TaskMessageKind, TaskRecoveryReason, TaskReviewVerdict, TaskSourceKind,
    WorkDomainError, WorkflowStageId, error::invalid_input,
};

#[path = "event_kind.rs"]
mod event_kind;
#[path = "event_validation.rs"]
mod event_validation;
#[path = "event/record.rs"]
mod record;

pub use event_kind::{SafeErrorCode, WorkEventKind};
pub use record::{WorkEventContext, WorkEventRecord};

string_enum! {
    /// Project fields represented by `ProjectUpdated`.
    pub enum ProjectChangedField, "event.project.changed_field" {
        /// Project name.
        Name => "name",
        /// Project description.
        Description => "description",
        /// Server-local execution folder.
        Folder => "folder",
    }

    /// Task fields represented by `TaskUpdated`.
    pub enum TaskChangedField, "event.task.changed_field" {
        /// Task title.
        Title => "title",
        /// Task description.
        Description => "description",
        /// Project assignment.
        Project => "project",
        /// Future execution or recurrence configuration.
        Schedule => "schedule",
        /// Assigned executor identity.
        Executor => "executor",
        /// Explicit working-directory override.
        WorkingDirectory => "working_directory",
    }


    /// Closed reason for a task stage transition.
    pub enum TaskStageChangeReason, "event.task.stage_change_reason" {
        Captured => "captured",
        Updated => "updated",
        Queued => "queued",
        RunStarted => "run_started",
        GateOpened => "gate_opened",
        GateResolved => "gate_resolved",
        Completed => "completed",
        Cancelled => "cancelled",
        Reopened => "reopened",
        Recovery => "recovery",
    }


    /// How a human gate was resolved.
    pub enum GateResolutionKind, "event.gate.resolution_kind" {
        Answer => "answer",
        Retry => "retry",
    }


    /// Why an open gate was superseded.
    pub enum GateSupersessionReason, "event.gate.supersession_reason" {
        Cancelled => "cancelled",
        NewGeneration => "new_generation",
        Replaced => "replaced",
    }


    /// Durable result produced by a completed run.
    pub enum RunTerminalKind, "event.run.terminal_kind" {
        Plan => "plan",
        Submission => "submission",
        Review => "review",
        GateResolved => "gate_resolved",
    }


    /// Closed reason for run cancellation.
    pub enum RunCancellationReason, "event.run.cancellation_reason" {
        Command => "command",
        StaleGeneration => "stale_generation",
        LeaseExpired => "lease_expired",
        Superseded => "superseded",
        WorkerShutdown => "worker_shutdown",
    }


    /// Notification category emitted from the Work ledger.
    pub enum NotificationKind, "event.notification.kind" {
        TaskCreated => "task_created",
        TaskWaiting => "task_waiting",
        TaskRecovery => "task_recovery",
        TaskCompleted => "task_completed",
    }


    /// Closed notification delivery destination.
    pub enum NotificationDestination, "event.notification.destination" {
        HumanPrimaryConversation => "human_primary_conversation",
    }
}

/// Closed, typed payload for one Work event.  Its fields are private so a
/// caller cannot pair an arbitrary event kind with arbitrary JSON metadata.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkEventPayload {
    kind: WorkEventKind,
    value: Value,
}

impl WorkEventPayload {
    /// Return the event kind enforced by this payload.
    #[must_use]
    pub const fn kind(&self) -> WorkEventKind {
        self.kind
    }

    /// Borrow the closed JSON representation used by the ledger.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    /// Reconstruct a persisted payload through the same closed schema checks
    /// used by typed constructors.
    /// # Errors
    /// Returns [`WorkDomainError`] when the JSON exceeds the ledger size
    /// limit, contains unsafe or kind-incompatible fields, or violates the
    /// selected event kind's versioned schema.
    pub fn from_persisted(kind: WorkEventKind, value: Value) -> Result<Self, WorkDomainError> {
        Self::new(kind, value)
    }

    fn new(kind: WorkEventKind, value: Value) -> Result<Self, WorkDomainError> {
        event_validation::validate_payload(kind, &value)?;
        Ok(Self { kind, value })
    }

    fn special(
        kind: WorkEventKind,
        allowed: bool,
        value: Value,
        message: &'static str,
    ) -> Result<Self, WorkDomainError> {
        if allowed {
            Self::new(kind, value)
        } else {
            Err(invalid_input("event.kind", message))
        }
    }

    /// Payload for project archive/reopen events.
    /// # Errors
    /// Returns [`WorkDomainError`] when `kind` is not ProjectArchived or
    /// ProjectReopened, or when `revision` is zero.
    pub fn project_lifecycle(kind: WorkEventKind, revision: u64) -> Result<Self, WorkDomainError> {
        Self::special(
            kind,
            matches!(
                kind,
                WorkEventKind::ProjectArchived | WorkEventKind::ProjectReopened
            ),
            json!({"v": 1, "revision": revision}),
            "project lifecycle payload requires an archive/reopen kind",
        )
    }

    /// Payload for run claimed/started events.
    /// # Errors
    /// Returns [`WorkDomainError`] when `kind` is not RunClaimed or RunStarted,
    /// generation is zero, or review round is incompatible with the run role.
    pub fn run_lifecycle(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        attempt_index: u32,
        review_round: u32,
    ) -> Result<Self, WorkDomainError> {
        Self::special(
            kind,
            matches!(kind, WorkEventKind::RunClaimed | WorkEventKind::RunStarted),
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "attempt_index": attempt_index, "review_round": review_round}),
            "run lifecycle payload requires claimed/started kind",
        )
    }

    /// Payload for run interrupted/failed events.
    /// # Errors
    /// Returns [`WorkDomainError`] when `kind` is not RunInterrupted or
    /// RunFailed, or when generation is zero.
    pub fn run_failure(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        attempt_index: u32,
        error_code: SafeErrorCode,
        retryable: bool,
    ) -> Result<Self, WorkDomainError> {
        Self::special(
            kind,
            matches!(
                kind,
                WorkEventKind::RunInterrupted | WorkEventKind::RunFailed
            ),
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "attempt_index": attempt_index, "error_code": error_code, "retryable": retryable}),
            "run failure payload requires interrupted/failed kind",
        )
    }

    /// Payload for run cancellation events.
    /// # Errors
    /// Returns [`WorkDomainError`] when `kind` is not RunCancelRequested or
    /// RunCancelled, or when generation is zero.
    pub fn run_cancelled(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        reason: RunCancellationReason,
    ) -> Result<Self, WorkDomainError> {
        Self::special(
            kind,
            matches!(
                kind,
                WorkEventKind::RunCancelRequested | WorkEventKind::RunCancelled
            ),
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "reason": reason}),
            "run cancellation payload requires cancel-requested/cancelled kind",
        )
    }
}

macro_rules! event_payload_schema {
    (
        constructors { $($name:ident($($arg:ident: $ty:ty),* $(,)?) => $kind:ident);+ $(;)? }
        special { $($($special_kind:ident)|+ => [$($field:ident),* $(,)?]);+ $(;)? }
    ) => {
        impl WorkEventPayload {
            $(
                #[doc = concat!("Construct and validate a `", stringify!($kind), "` event payload.")]
                /// # Errors
                /// Returns [`WorkDomainError`] when any field violates the closed event schema.
                pub fn $name($($arg: $ty),*) -> Result<Self, WorkDomainError> {
                    let mut object = serde_json::Map::new();
                    object.insert("v".to_owned(), json!(1));
                    $(object.insert(stringify!($arg).to_owned(), json!($arg));)*
                    Self::new(WorkEventKind::$kind, Value::Object(object))
                }
            )+
        }

        /// Required kind-specific fields; every payload additionally carries `v`.
        #[rustfmt::skip]
        pub(super) const fn event_payload_fields(kind: WorkEventKind) -> &'static [&'static str] {
            match kind {
                $(WorkEventKind::$kind => &[$(stringify!($arg)),*],)+
                $($(WorkEventKind::$special_kind)|+ => &[$(stringify!($field)),*],)+
            }
        }
    };
}

event_payload_schema! {
  constructors {
    project_created(revision: u64) => ProjectCreated;
    project_updated(revision: u64, changed_fields: Vec<ProjectChangedField>) => ProjectUpdated;
    task_captured(revision: u64, generation: u64, stage_id: WorkflowStageId, source_kind: TaskSourceKind) => TaskCaptured;
    task_updated(revision: u64, generation: u64, changed_fields: Vec<TaskChangedField>) => TaskUpdated;
    task_queued(revision: u64, generation: u64, contract_id: Option<TaskContractId>, next_run_kind: RunKind) => TaskQueued;
    task_stage_changed(revision: u64, generation: u64, from_stage_id: WorkflowStageId, to_stage_id: WorkflowStageId, reason: TaskStageChangeReason) => TaskStageChanged;
    task_cancelled(revision: u64, generation: u64, reason_present: bool) => TaskCancelled;
    task_reopened(revision: u64, generation: u64, stage_id: WorkflowStageId) => TaskReopened;
    task_completed(revision: u64, generation: u64) => TaskCompleted;
    recurrence_changed(recurrence_id: String, revision: u64, reason: String) => RecurrenceChanged;
    contract_created(contract_id: TaskContractId, version: u32, generation: u64, origin: ContractOrigin, complexity: TaskComplexity, criteria_count: u32, supersedes_contract_id: Option<TaskContractId>) => ContractCreated;
    gate_opened(gate_id: TaskGateId, generation: u64, gate_kind: TaskGateKind, originating_run_id: Option<String>, recovery_reason: Option<TaskRecoveryReason>, retry_run_kind: Option<RunKind>) => GateOpened;
    gate_resolved(gate_id: TaskGateId, generation: u64, gate_kind: TaskGateKind, message_id: TaskMessageId, resolution_kind: GateResolutionKind) => GateResolved;
    gate_superseded(gate_id: TaskGateId, generation: u64, gate_kind: TaskGateKind, reason: GateSupersessionReason) => GateSuperseded;
    task_message_appended(message_id: TaskMessageId, generation: u64, message_kind: TaskMessageKind, gate_id: Option<TaskGateId>, contract_id: Option<TaskContractId>) => TaskMessageAppended;
    task_message_consumed(message_id: TaskMessageId, generation: u64, consumed_by_run_id: String) => TaskMessageConsumed;
    run_queued(run_kind: RunKind, generation: u64, contract_id: Option<TaskContractId>, attempt_index: u32, review_round: u32, parent_run_id: Option<String>) => RunQueued;
    run_heartbeat(run_kind: RunKind, generation: u64, provider_call_count: u32, tool_call_count: u32, active_milliseconds: u64) => RunHeartbeat;
    run_completed(run_kind: RunKind, generation: u64, terminal_kind: RunTerminalKind) => RunCompleted;
    run_waiting_for_approval(run_kind: RunKind, generation: u64, gate_id: TaskGateId, gate_kind: TaskGateKind) => RunWaitingForApproval;
    submission_created(submission_id: String, contract_id: TaskContractId, review_round: u32, criteria_count: u32, artifact_count: u32) => SubmissionCreated;
    review_created(review_id: String, submission_id: String, contract_id: TaskContractId, review_round: u32, review_attempt_index: u32, supersedes_review_id: Option<String>, verdict: TaskReviewVerdict) => ReviewCreated;
    notification_queued(notification_id: String, source_event_sequence: u64, notification_kind: NotificationKind, destination_kind: NotificationDestination) => NotificationQueued;
    notification_delivered(notification_id: String, source_event_sequence: u64, notification_kind: NotificationKind, attempt_count: u32) => NotificationDelivered;
    notification_failed(notification_id: String, source_event_sequence: u64, notification_kind: NotificationKind, attempt_count: u32, error_code: SafeErrorCode, retryable: bool) => NotificationFailed;
  }
  special {
    ProjectArchived | ProjectReopened => [revision];
    RunClaimed | RunStarted => [run_kind, generation, attempt_index, review_round];
    RunInterrupted | RunFailed => [run_kind, generation, attempt_index, error_code, retryable];
    RunCancelRequested | RunCancelled => [run_kind, generation, reason];
  }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurrence_reason_preserves_text_and_rejects_nested_fields() {
        let reason = "provider token feedback description";
        let payload = WorkEventPayload::recurrence_changed(
            "recurrence:daily".to_string(),
            1,
            reason.to_string(),
        )
        .expect("ordinary reason is valid");

        assert_eq!(payload.as_value()["reason"], reason);
        assert!(
            WorkEventPayload::from_persisted(
                WorkEventKind::RecurrenceChanged,
                json!({
                    "v": 1,
                    "recurrence_id": "recurrence:daily",
                    "revision": 1,
                    "reason": {"token": "not a scalar reason"}
                }),
            )
            .is_err()
        );
    }
}
