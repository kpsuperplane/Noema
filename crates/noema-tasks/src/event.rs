use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ContractOrigin, RunKind, TaskComplexity, TaskContractId, TaskGateId, TaskGateKind, TaskId,
    TaskMessageId, TaskMessageKind, TaskRecoveryReason, TaskReviewVerdict, TaskSourceKind,
    WorkDomainError, WorkEventId, WorkflowStageId, error::invalid_input,
};

mod event_kind;
mod event_schema;
mod event_validation;

pub use event_kind::{SafeErrorCode, WorkEventKind};

macro_rules! closed_event_enum {
    ($name:ident, $field:literal, { $( $variant:ident => $wire:literal ),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant,)+
        }

        impl $name {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire,)+ }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = WorkDomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(invalid_input($field, format!("unknown value {other}"))),
                }
            }
        }
    };
}

closed_event_enum!(
    ProjectChangedField,
    "event.project.changed_field",
    { Name => "name", Description => "description" }
);
closed_event_enum!(
    TaskChangedField,
    "event.task.changed_field",
    { Title => "title", Description => "description", Project => "project" }
);
closed_event_enum!(
    TaskStageChangeReason,
    "event.task.stage_change_reason",
    {
        Captured => "captured",
        Updated => "updated",
        Queued => "queued",
        RunStarted => "run_started",
        GateOpened => "gate_opened",
        GateResolved => "gate_resolved",
        ReviewReady => "review_ready",
        Accepted => "accepted",
        Cancelled => "cancelled",
        Reopened => "reopened",
        Recovery => "recovery",
        RequestChanges => "request_changes"
    }
);
closed_event_enum!(
    GateResolutionKind,
    "event.gate.resolution_kind",
    { Answer => "answer", Retry => "retry" }
);
closed_event_enum!(
    GateSupersessionReason,
    "event.gate.supersession_reason",
    { Cancelled => "cancelled", NewGeneration => "new_generation", Replaced => "replaced" }
);
closed_event_enum!(
    RunTerminalKind,
    "event.run.terminal_kind",
    { Plan => "plan", Submission => "submission", Review => "review", GateResolved => "gate_resolved" }
);
closed_event_enum!(
    RunCancellationReason,
    "event.run.cancellation_reason",
    {
        Command => "command",
        StaleGeneration => "stale_generation",
        LeaseExpired => "lease_expired",
        Superseded => "superseded",
        WorkerShutdown => "worker_shutdown"
    }
);
closed_event_enum!(
    NotificationKind,
    "event.notification.kind",
    {
        TaskCreated => "task_created",
        TaskWaiting => "task_waiting",
        TaskReviewReady => "task_review_ready",
        TaskRecovery => "task_recovery",
        TaskAccepted => "task_accepted"
    }
);
closed_event_enum!(
    NotificationDestination,
    "event.notification.destination",
    { HumanPrimaryConversation => "human_primary_conversation" }
);

/// Closed, typed payload for one Work event.  Its fields are private so a
/// caller cannot pair an arbitrary event kind with arbitrary JSON metadata.
#[derive(Debug, Clone, PartialEq)]
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

    /// Borrow the redacted JSON representation used by the ledger.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    /// Consume the typed payload into its validated JSON representation.
    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }

    /// Reconstruct a persisted payload through the same closed schema checks
    /// used by typed constructors.
    pub fn from_persisted(kind: WorkEventKind, value: Value) -> Result<Self, WorkDomainError> {
        Self::new(kind, value)
    }

    fn new(kind: WorkEventKind, value: Value) -> Result<Self, WorkDomainError> {
        event_validation::validate_payload(kind, &value)?;
        Ok(Self { kind, value })
    }

    /// Payload for `project.created`.
    #[must_use]
    pub fn project_created(revision: u64) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::ProjectCreated,
            json!({"v": 1, "revision": revision}),
        )
    }

    /// Payload for `project.updated`.
    #[must_use]
    pub fn project_updated(
        revision: u64,
        changed_fields: Vec<ProjectChangedField>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::ProjectUpdated,
            json!({"v": 1, "revision": revision, "changed_fields": changed_fields}),
        )
    }

    /// Payload for project archive/reopen events.
    #[must_use]
    pub fn project_lifecycle(kind: WorkEventKind, revision: u64) -> Result<Self, WorkDomainError> {
        if matches!(
            kind,
            WorkEventKind::ProjectArchived | WorkEventKind::ProjectReopened
        ) {
            Self::new(kind, json!({"v": 1, "revision": revision}))
        } else {
            Err(invalid_input(
                "event.kind",
                "project lifecycle payload requires an archive/reopen kind",
            ))
        }
    }

    /// Payload for `task.captured`.
    #[must_use]
    pub fn task_captured(
        revision: u64,
        generation: u64,
        stage_id: WorkflowStageId,
        source_kind: TaskSourceKind,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskCaptured,
            json!({"v": 1, "revision": revision, "generation": generation, "stage_id": stage_id, "source_kind": source_kind}),
        )
    }

    /// Payload for `task.updated`.
    #[must_use]
    pub fn task_updated(
        revision: u64,
        generation: u64,
        changed_fields: Vec<TaskChangedField>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskUpdated,
            json!({"v": 1, "revision": revision, "generation": generation, "changed_fields": changed_fields}),
        )
    }

    /// Payload for `task.queued`.
    #[must_use]
    pub fn task_queued(
        revision: u64,
        generation: u64,
        contract_id: Option<TaskContractId>,
        next_run_kind: RunKind,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskQueued,
            json!({"v": 1, "revision": revision, "generation": generation, "contract_id": contract_id, "next_run_kind": next_run_kind}),
        )
    }

    /// Payload for `task.stage_changed`.
    #[must_use]
    pub fn task_stage_changed(
        revision: u64,
        generation: u64,
        from_stage_id: WorkflowStageId,
        to_stage_id: WorkflowStageId,
        reason: TaskStageChangeReason,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskStageChanged,
            json!({"v": 1, "revision": revision, "generation": generation, "from_stage_id": from_stage_id, "to_stage_id": to_stage_id, "reason": reason}),
        )
    }

    /// Payload for `task.cancelled`.
    #[must_use]
    pub fn task_cancelled(
        revision: u64,
        generation: u64,
        reason_present: bool,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskCancelled,
            json!({"v": 1, "revision": revision, "generation": generation, "reason_present": reason_present}),
        )
    }

    /// Payload for `task.reopened`.
    #[must_use]
    pub fn task_reopened(
        revision: u64,
        generation: u64,
        stage_id: WorkflowStageId,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskReopened,
            json!({"v": 1, "revision": revision, "generation": generation, "stage_id": stage_id}),
        )
    }

    /// Payload for `task.accepted`.
    #[must_use]
    pub fn task_accepted(
        revision: u64,
        generation: u64,
        submission_id: String,
        review_id: String,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskAccepted,
            json!({"v": 1, "revision": revision, "generation": generation, "submission_id": submission_id, "review_id": review_id}),
        )
    }

    /// Payload for `contract.created`.
    #[must_use]
    pub fn contract_created(
        contract_id: TaskContractId,
        version: u32,
        generation: u64,
        origin: ContractOrigin,
        complexity: TaskComplexity,
        criteria_count: u32,
        supersedes_contract_id: Option<TaskContractId>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::ContractCreated,
            json!({"v": 1, "contract_id": contract_id, "version": version, "generation": generation, "origin": origin, "complexity": complexity, "criteria_count": criteria_count, "supersedes_contract_id": supersedes_contract_id}),
        )
    }

    /// Payload for `gate.opened`.
    #[must_use]
    pub fn gate_opened(
        gate_id: TaskGateId,
        generation: u64,
        gate_kind: TaskGateKind,
        originating_run_id: Option<String>,
        recovery_reason: Option<TaskRecoveryReason>,
        retry_run_kind: Option<RunKind>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::GateOpened,
            json!({"v": 1, "gate_id": gate_id, "generation": generation, "gate_kind": gate_kind, "originating_run_id": originating_run_id, "recovery_reason": recovery_reason, "retry_run_kind": retry_run_kind}),
        )
    }

    /// Payload for `gate.resolved`.
    #[must_use]
    pub fn gate_resolved(
        gate_id: TaskGateId,
        generation: u64,
        gate_kind: TaskGateKind,
        message_id: TaskMessageId,
        resolution_kind: GateResolutionKind,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::GateResolved,
            json!({"v": 1, "gate_id": gate_id, "generation": generation, "gate_kind": gate_kind, "message_id": message_id, "resolution_kind": resolution_kind}),
        )
    }

    /// Payload for `gate.superseded`.
    #[must_use]
    pub fn gate_superseded(
        gate_id: TaskGateId,
        generation: u64,
        gate_kind: TaskGateKind,
        reason: GateSupersessionReason,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::GateSuperseded,
            json!({"v": 1, "gate_id": gate_id, "generation": generation, "gate_kind": gate_kind, "reason": reason}),
        )
    }

    /// Payload for `task.message_appended`.
    #[must_use]
    pub fn task_message_appended(
        message_id: TaskMessageId,
        generation: u64,
        message_kind: TaskMessageKind,
        gate_id: Option<TaskGateId>,
        contract_id: Option<TaskContractId>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskMessageAppended,
            json!({"v": 1, "message_id": message_id, "generation": generation, "message_kind": message_kind, "gate_id": gate_id, "contract_id": contract_id}),
        )
    }

    /// Payload for `task.message_consumed`.
    #[must_use]
    pub fn task_message_consumed(
        message_id: TaskMessageId,
        generation: u64,
        consumed_by_run_id: String,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::TaskMessageConsumed,
            json!({"v": 1, "message_id": message_id, "generation": generation, "consumed_by_run_id": consumed_by_run_id}),
        )
    }

    /// Payload for `run.queued`.
    #[must_use]
    pub fn run_queued(
        run_kind: RunKind,
        generation: u64,
        contract_id: Option<TaskContractId>,
        attempt_index: u32,
        review_round: u32,
        parent_run_id: Option<String>,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::RunQueued,
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "contract_id": contract_id, "attempt_index": attempt_index, "review_round": review_round, "parent_run_id": parent_run_id}),
        )
    }

    /// Payload for run claimed/started events.
    #[must_use]
    pub fn run_lifecycle(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        attempt_index: u32,
        review_round: u32,
    ) -> Result<Self, WorkDomainError> {
        if matches!(kind, WorkEventKind::RunClaimed | WorkEventKind::RunStarted) {
            Self::new(
                kind,
                json!({"v": 1, "run_kind": run_kind, "generation": generation, "attempt_index": attempt_index, "review_round": review_round}),
            )
        } else {
            Err(invalid_input(
                "event.kind",
                "run lifecycle payload requires claimed/started kind",
            ))
        }
    }

    /// Payload for `run.heartbeat`.
    #[must_use]
    pub fn run_heartbeat(
        run_kind: RunKind,
        generation: u64,
        provider_call_count: u32,
        tool_call_count: u32,
        active_milliseconds: u64,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::RunHeartbeat,
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "provider_call_count": provider_call_count, "tool_call_count": tool_call_count, "active_milliseconds": active_milliseconds}),
        )
    }

    /// Payload for `run.completed`.
    #[must_use]
    pub fn run_completed(
        run_kind: RunKind,
        generation: u64,
        terminal_kind: RunTerminalKind,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::RunCompleted,
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "terminal_kind": terminal_kind}),
        )
    }

    /// Payload for `run.waiting_for_approval`.
    #[must_use]
    pub fn run_waiting_for_approval(
        run_kind: RunKind,
        generation: u64,
        gate_id: TaskGateId,
        gate_kind: TaskGateKind,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::RunWaitingForApproval,
            json!({"v": 1, "run_kind": run_kind, "generation": generation, "gate_id": gate_id, "gate_kind": gate_kind}),
        )
    }

    /// Payload for run interrupted/failed events.
    #[must_use]
    pub fn run_failure(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        attempt_index: u32,
        error_code: SafeErrorCode,
        retryable: bool,
    ) -> Result<Self, WorkDomainError> {
        if matches!(
            kind,
            WorkEventKind::RunInterrupted | WorkEventKind::RunFailed
        ) {
            Self::new(
                kind,
                json!({"v": 1, "run_kind": run_kind, "generation": generation, "attempt_index": attempt_index, "error_code": error_code, "retryable": retryable}),
            )
        } else {
            Err(invalid_input(
                "event.kind",
                "run failure payload requires interrupted/failed kind",
            ))
        }
    }

    /// Payload for run cancellation events.
    #[must_use]
    pub fn run_cancelled(
        kind: WorkEventKind,
        run_kind: RunKind,
        generation: u64,
        reason: RunCancellationReason,
    ) -> Result<Self, WorkDomainError> {
        if matches!(
            kind,
            WorkEventKind::RunCancelRequested | WorkEventKind::RunCancelled
        ) {
            Self::new(
                kind,
                json!({"v": 1, "run_kind": run_kind, "generation": generation, "reason": reason}),
            )
        } else {
            Err(invalid_input(
                "event.kind",
                "run cancellation payload requires cancel-requested/cancelled kind",
            ))
        }
    }

    /// Payload for `submission.created`.
    #[must_use]
    pub fn submission_created(
        submission_id: String,
        contract_id: TaskContractId,
        review_round: u32,
        criteria_count: u32,
        artifact_count: u32,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::SubmissionCreated,
            json!({"v": 1, "submission_id": submission_id, "contract_id": contract_id, "review_round": review_round, "criteria_count": criteria_count, "artifact_count": artifact_count}),
        )
    }

    /// Payload for `review.created`.
    #[must_use]
    pub fn review_created(
        review_id: String,
        submission_id: String,
        contract_id: TaskContractId,
        review_round: u32,
        review_attempt_index: u32,
        supersedes_review_id: Option<String>,
        verdict: TaskReviewVerdict,
    ) -> Result<Self, WorkDomainError> {
        Self::new(
            WorkEventKind::ReviewCreated,
            json!({"v": 1, "review_id": review_id, "submission_id": submission_id, "contract_id": contract_id, "review_round": review_round, "review_attempt_index": review_attempt_index, "supersedes_review_id": supersedes_review_id, "verdict": verdict}),
        )
    }

    /// Payload for notification queue/delivery/failure events.
    #[must_use]
    pub fn notification(
        kind: WorkEventKind,
        notification_id: String,
        source_event_sequence: u64,
        notification_kind: NotificationKind,
        destination_kind: Option<NotificationDestination>,
        attempt_count: Option<u32>,
        error_code: Option<SafeErrorCode>,
        retryable: Option<bool>,
    ) -> Result<Self, WorkDomainError> {
        let value = match kind {
            WorkEventKind::NotificationQueued => {
                json!({"v": 1, "notification_id": notification_id, "source_event_sequence": source_event_sequence, "notification_kind": notification_kind, "destination_kind": destination_kind})
            }
            WorkEventKind::NotificationDelivered => {
                json!({"v": 1, "notification_id": notification_id, "source_event_sequence": source_event_sequence, "notification_kind": notification_kind, "attempt_count": attempt_count})
            }
            WorkEventKind::NotificationFailed => {
                json!({"v": 1, "notification_id": notification_id, "source_event_sequence": source_event_sequence, "notification_kind": notification_kind, "attempt_count": attempt_count, "error_code": error_code, "retryable": retryable})
            }
            _ => {
                return Err(invalid_input(
                    "event.kind",
                    "notification payload requires a notification kind",
                ));
            }
        };
        Self::new(kind, value)
    }
}

/// Immutable event appended to the global Work ledger.  `non_exhaustive`
/// prevents downstream crates from constructing an arbitrary kind/payload pair
/// with a struct literal; use [`WorkEventRecord::new`] and a typed
/// [`WorkEventPayload`] instead.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkEventRecord {
    /// Stable event identity.
    pub event_id: WorkEventId,
    /// Monotonic SQLite ledger cursor.
    pub event_sequence: u64,
    /// Closed event vocabulary value.
    pub kind: WorkEventKind,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Optional project affected by the event.
    pub project_id: Option<ProjectId>,
    /// Optional task affected by the event.
    pub task_id: Option<TaskId>,
    /// Optional run affected by the event.
    pub run_id: Option<String>,
    /// Actor/component that caused the event.
    pub actor_id: String,
    /// Direct causation identifier.
    pub causation_id: Option<String>,
    /// Required cross-run/user correlation identifier.
    pub correlation_id: String,
    /// Redacted UI-safe payload beginning with `{"v": 1}`.
    pub safe_payload: Value,
    /// Creation timestamp.
    pub created_at: String,
    /// Constructor provenance retained outside the public wire shape.
    #[serde(skip)]
    constructed_kind: WorkEventKind,
}

impl WorkEventRecord {
    /// Construct an event from a typed, kind-matched payload.
    pub fn new(
        event_id: WorkEventId,
        event_sequence: u64,
        workspace_id: WorkspaceId,
        project_id: Option<ProjectId>,
        task_id: Option<TaskId>,
        run_id: Option<String>,
        actor_id: String,
        causation_id: Option<String>,
        correlation_id: String,
        payload: WorkEventPayload,
        created_at: String,
    ) -> Result<Self, WorkDomainError> {
        let record = Self {
            event_id,
            event_sequence,
            kind: payload.kind,
            workspace_id,
            project_id,
            task_id,
            run_id,
            actor_id,
            causation_id,
            correlation_id,
            safe_payload: payload.value,
            created_at,
            constructed_kind: payload.kind,
        };
        record.validate()?;
        Ok(record)
    }
}

impl WorkEventRecord {
    /// Validate event identity and causal metadata.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.kind != self.constructed_kind {
            return Err(invalid_input(
                "work_event.kind",
                "public kind does not match the typed constructor kind",
            ));
        }
        if self.event_sequence == 0 {
            return Err(invalid_input(
                "work_event.event_sequence",
                "sequence must be positive",
            ));
        }
        if self.actor_id.trim().is_empty() || self.correlation_id.trim().is_empty() {
            return Err(invalid_input(
                "work_event",
                "actor and correlation cannot be blank",
            ));
        }
        validate_record_identifier(&self.actor_id, "work_event.actor_id", &["actor:"])?;
        validate_record_identifier(
            &self.correlation_id,
            "work_event.correlation_id",
            &["correlation:"],
        )?;
        if let Some(causation_id) = &self.causation_id {
            validate_record_identifier(
                causation_id,
                "work_event.causation_id",
                &["event:", "command:", "run:"],
            )?;
        }
        if let Some(run_id) = &self.run_id {
            validate_record_identifier(run_id, "work_event.run_id", &["run:"])?;
        }
        if self.created_at.trim().is_empty() {
            return Err(invalid_input(
                "work_event.created_at",
                "timestamp cannot be blank",
            ));
        }
        event_validation::validate_payload(self.kind, &self.safe_payload)?;
        Ok(())
    }
}

fn validate_record_identifier(
    value: &str,
    field: &'static str,
    prefixes: &[&str],
) -> Result<(), WorkDomainError> {
    if value.len() > 255
        || value.trim().is_empty()
        || value.chars().any(char::is_control)
        || !prefixes.iter().any(|prefix| value.starts_with(prefix))
    {
        return Err(invalid_input(field, "identifier has an invalid shape"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
