use super::WorkEventKind;

/// Allowed and required JSON keys for each closed event kind.
pub(crate) const fn payload_contract(
    kind: WorkEventKind,
) -> (&'static [&'static str], &'static [&'static str]) {
    match kind {
        WorkEventKind::ProjectCreated => (&["v", "revision"], &["revision"]),
        WorkEventKind::ProjectUpdated => (
            &["v", "revision", "changed_fields"],
            &["revision", "changed_fields"],
        ),
        WorkEventKind::ProjectArchived | WorkEventKind::ProjectReopened => {
            (&["v", "revision"], &["revision"])
        }
        WorkEventKind::TaskCaptured => (
            &["v", "revision", "generation", "stage_id", "source_kind"],
            &["revision", "generation", "stage_id", "source_kind"],
        ),
        WorkEventKind::TaskUpdated => (
            &["v", "revision", "generation", "changed_fields"],
            &["revision", "generation", "changed_fields"],
        ),
        WorkEventKind::TaskQueued => (
            &[
                "v",
                "revision",
                "generation",
                "contract_id",
                "next_run_kind",
            ],
            &["revision", "generation", "contract_id", "next_run_kind"],
        ),
        WorkEventKind::TaskStageChanged => (
            &[
                "v",
                "revision",
                "generation",
                "from_stage_id",
                "to_stage_id",
                "reason",
            ],
            &[
                "revision",
                "generation",
                "from_stage_id",
                "to_stage_id",
                "reason",
            ],
        ),
        WorkEventKind::TaskCancelled => (
            &["v", "revision", "generation", "reason_present"],
            &["revision", "generation", "reason_present"],
        ),
        WorkEventKind::TaskReopened => (
            &["v", "revision", "generation", "stage_id"],
            &["revision", "generation", "stage_id"],
        ),
        WorkEventKind::TaskAccepted => (
            &["v", "revision", "generation", "submission_id", "review_id"],
            &["revision", "generation", "submission_id", "review_id"],
        ),
        WorkEventKind::ContractCreated => (
            &[
                "v",
                "contract_id",
                "version",
                "generation",
                "origin",
                "complexity",
                "criteria_count",
                "supersedes_contract_id",
            ],
            &[
                "contract_id",
                "version",
                "generation",
                "origin",
                "complexity",
                "criteria_count",
                "supersedes_contract_id",
            ],
        ),
        WorkEventKind::GateOpened => (
            &[
                "v",
                "gate_id",
                "generation",
                "gate_kind",
                "originating_run_id",
                "recovery_reason",
                "retry_run_kind",
            ],
            &[
                "gate_id",
                "generation",
                "gate_kind",
                "originating_run_id",
                "recovery_reason",
                "retry_run_kind",
            ],
        ),
        WorkEventKind::GateResolved => (
            &[
                "v",
                "gate_id",
                "generation",
                "gate_kind",
                "message_id",
                "resolution_kind",
            ],
            &[
                "gate_id",
                "generation",
                "gate_kind",
                "message_id",
                "resolution_kind",
            ],
        ),
        WorkEventKind::GateSuperseded => (
            &["v", "gate_id", "generation", "gate_kind", "reason"],
            &["gate_id", "generation", "gate_kind", "reason"],
        ),
        WorkEventKind::TaskMessageAppended => (
            &[
                "v",
                "message_id",
                "generation",
                "message_kind",
                "gate_id",
                "contract_id",
            ],
            &[
                "message_id",
                "generation",
                "message_kind",
                "gate_id",
                "contract_id",
            ],
        ),
        WorkEventKind::TaskMessageConsumed => (
            &["v", "message_id", "generation", "consumed_by_run_id"],
            &["message_id", "generation", "consumed_by_run_id"],
        ),
        WorkEventKind::RunQueued => (
            &[
                "v",
                "run_kind",
                "generation",
                "contract_id",
                "attempt_index",
                "review_round",
                "parent_run_id",
            ],
            &[
                "run_kind",
                "generation",
                "contract_id",
                "attempt_index",
                "review_round",
                "parent_run_id",
            ],
        ),
        WorkEventKind::RunClaimed | WorkEventKind::RunStarted => (
            &[
                "v",
                "run_kind",
                "generation",
                "attempt_index",
                "review_round",
            ],
            &["run_kind", "generation", "attempt_index", "review_round"],
        ),
        WorkEventKind::RunHeartbeat => (
            &[
                "v",
                "run_kind",
                "generation",
                "provider_call_count",
                "tool_call_count",
                "active_milliseconds",
            ],
            &[
                "run_kind",
                "generation",
                "provider_call_count",
                "tool_call_count",
                "active_milliseconds",
            ],
        ),
        WorkEventKind::RunCompleted => (
            &["v", "run_kind", "generation", "terminal_kind"],
            &["run_kind", "generation", "terminal_kind"],
        ),
        WorkEventKind::RunWaitingForApproval => (
            &["v", "run_kind", "generation", "gate_id", "gate_kind"],
            &["run_kind", "generation", "gate_id", "gate_kind"],
        ),
        WorkEventKind::RunInterrupted | WorkEventKind::RunFailed => (
            &[
                "v",
                "run_kind",
                "generation",
                "attempt_index",
                "error_code",
                "retryable",
            ],
            &[
                "run_kind",
                "generation",
                "attempt_index",
                "error_code",
                "retryable",
            ],
        ),
        WorkEventKind::RunCancelRequested | WorkEventKind::RunCancelled => (
            &["v", "run_kind", "generation", "reason"],
            &["run_kind", "generation", "reason"],
        ),
        WorkEventKind::SubmissionCreated => (
            &[
                "v",
                "submission_id",
                "contract_id",
                "review_round",
                "criteria_count",
                "artifact_count",
            ],
            &[
                "submission_id",
                "contract_id",
                "review_round",
                "criteria_count",
                "artifact_count",
            ],
        ),
        WorkEventKind::ReviewCreated => (
            &[
                "v",
                "review_id",
                "submission_id",
                "contract_id",
                "review_round",
                "review_attempt_index",
                "supersedes_review_id",
                "verdict",
            ],
            &[
                "review_id",
                "submission_id",
                "contract_id",
                "review_round",
                "review_attempt_index",
                "supersedes_review_id",
                "verdict",
            ],
        ),
        WorkEventKind::NotificationQueued => (
            &[
                "v",
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "destination_kind",
            ],
            &[
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "destination_kind",
            ],
        ),
        WorkEventKind::NotificationDelivered => (
            &[
                "v",
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "attempt_count",
            ],
            &[
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "attempt_count",
            ],
        ),
        WorkEventKind::NotificationFailed => (
            &[
                "v",
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "attempt_count",
                "error_code",
                "retryable",
            ],
            &[
                "notification_id",
                "source_event_sequence",
                "notification_kind",
                "attempt_count",
                "error_code",
                "retryable",
            ],
        ),
    }
}
