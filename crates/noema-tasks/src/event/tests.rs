use std::str::FromStr;

use noema_workspaces::{ProjectId, WorkspaceId};
use serde_json::json;

use super::{WorkEventKind, WorkEventPayload, WorkEventRecord};
use crate::{
    ContractOrigin, GateResolutionKind, GateSupersessionReason, NotificationDestination,
    NotificationKind, ProjectChangedField, RunCancellationReason, RunKind, RunTerminalKind,
    SafeErrorCode, TaskChangedField, TaskComplexity, TaskContractId, TaskGateId, TaskGateKind,
    TaskId, TaskMessageId, TaskMessageKind, TaskReviewVerdict, TaskSourceKind,
    TaskStageChangeReason, WorkEventId, WorkflowStageId,
};

#[test]
fn event_vocabulary_is_closed_and_wire_stable() {
    for kind in [
        WorkEventKind::TaskCaptured,
        WorkEventKind::RunCompleted,
        WorkEventKind::NotificationFailed,
    ] {
        let wire = kind.as_str();
        assert_eq!(WorkEventKind::from_str(wire).unwrap(), kind);
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{wire}\""));
    }
    assert!(WorkEventKind::from_str("task.failed").is_err());
}

fn record(payload: WorkEventPayload) -> WorkEventRecord {
    WorkEventRecord::new(
        WorkEventId::new("event:1").unwrap(),
        1,
        WorkspaceId::new("workspace:personal").unwrap(),
        Some(ProjectId::new("project:alpha").unwrap()),
        Some(TaskId::new("task:one").unwrap()),
        None,
        "actor:system".to_string(),
        None,
        "correlation:one".to_string(),
        payload,
        "2026-01-01T00:00:00Z".to_string(),
    )
    .unwrap()
}

#[test]
fn typed_payloads_cover_every_kind_and_revalidate_after_mutation() {
    let payloads = typed_payloads();
    let mut seen = std::collections::BTreeSet::new();
    assert_eq!(payloads.len(), 32);
    for (expected_kind, payload) in payloads {
        assert!(
            seen.insert(expected_kind.as_str()),
            "duplicate constructor coverage for {expected_kind}"
        );
        assert_eq!(payload.kind(), expected_kind);
        let mut event = record(payload);
        assert_eq!(event.kind, expected_kind);
        assert!(event.validate().is_ok());
        event.safe_payload["unknown"] = json!(true);
        assert!(event.validate().is_err());
    }
    assert_eq!(seen.len(), 32);
}

#[test]
fn persisted_payload_reconstruction_roundtrips_and_fails_closed() {
    let original = WorkEventPayload::project_created(1).unwrap();
    let value = original.into_value();
    let restored =
        WorkEventPayload::from_persisted(WorkEventKind::ProjectCreated, value.clone()).unwrap();
    assert_eq!(restored.kind(), WorkEventKind::ProjectCreated);
    assert_eq!(restored.as_value(), &value);

    let mut restored_record = record(restored);
    assert!(restored_record.validate().is_ok());
    restored_record.kind = WorkEventKind::ProjectArchived;
    assert!(restored_record.validate().is_err());

    assert!(WorkEventPayload::from_persisted(WorkEventKind::TaskCaptured, value.clone()).is_err());
    let mut invalid_value = value;
    invalid_value["v"] = json!(2);
    assert!(
        WorkEventPayload::from_persisted(WorkEventKind::ProjectCreated, invalid_value).is_err()
    );
}

fn typed_payloads() -> Vec<(WorkEventKind, WorkEventPayload)> {
    use WorkEventKind as K;

    let contract = || TaskContractId::new("contract:one").unwrap();
    let gate = || TaskGateId::new("gate:one").unwrap();
    let message = || TaskMessageId::new("task_message:one").unwrap();
    let stage = || WorkflowStageId::new("stage:personal:inbox").unwrap();
    let safe_code = || SafeErrorCode::new("timeout").unwrap();

    vec![
        (
            K::ProjectCreated,
            WorkEventPayload::project_created(1).unwrap(),
        ),
        (
            K::ProjectUpdated,
            WorkEventPayload::project_updated(1, vec![ProjectChangedField::Name]).unwrap(),
        ),
        (
            K::ProjectArchived,
            WorkEventPayload::project_lifecycle(K::ProjectArchived, 1).unwrap(),
        ),
        (
            K::ProjectReopened,
            WorkEventPayload::project_lifecycle(K::ProjectReopened, 1).unwrap(),
        ),
        (
            K::TaskCaptured,
            WorkEventPayload::task_captured(1, 1, stage(), TaskSourceKind::WorkUi).unwrap(),
        ),
        (
            K::TaskUpdated,
            WorkEventPayload::task_updated(1, 1, vec![TaskChangedField::Title]).unwrap(),
        ),
        (
            K::TaskQueued,
            WorkEventPayload::task_queued(1, 1, Some(contract()), RunKind::Executor).unwrap(),
        ),
        (
            K::TaskStageChanged,
            WorkEventPayload::task_stage_changed(
                1,
                1,
                stage(),
                WorkflowStageId::new("stage:personal:active").unwrap(),
                TaskStageChangeReason::Queued,
            )
            .unwrap(),
        ),
        (
            K::TaskCancelled,
            WorkEventPayload::task_cancelled(1, 1, true).unwrap(),
        ),
        (
            K::TaskReopened,
            WorkEventPayload::task_reopened(1, 1, stage()).unwrap(),
        ),
        (
            K::TaskAccepted,
            WorkEventPayload::task_accepted(
                1,
                1,
                "submission:one".to_string(),
                "review:one".to_string(),
            )
            .unwrap(),
        ),
        (
            K::ContractCreated,
            WorkEventPayload::contract_created(
                contract(),
                1,
                1,
                ContractOrigin::Planned,
                TaskComplexity::Medium,
                1,
                None,
            )
            .unwrap(),
        ),
        (
            K::GateOpened,
            WorkEventPayload::gate_opened(gate(), 1, TaskGateKind::Clarification, None, None, None)
                .unwrap(),
        ),
        (
            K::GateResolved,
            WorkEventPayload::gate_resolved(
                gate(),
                1,
                TaskGateKind::Clarification,
                message(),
                GateResolutionKind::Answer,
            )
            .unwrap(),
        ),
        (
            K::GateSuperseded,
            WorkEventPayload::gate_superseded(
                gate(),
                1,
                TaskGateKind::Clarification,
                GateSupersessionReason::Cancelled,
            )
            .unwrap(),
        ),
        (
            K::TaskMessageAppended,
            WorkEventPayload::task_message_appended(
                message(),
                1,
                TaskMessageKind::HumanAnswer,
                Some(gate()),
                Some(contract()),
            )
            .unwrap(),
        ),
        (
            K::TaskMessageConsumed,
            WorkEventPayload::task_message_consumed(message(), 1, "run:one".to_string()).unwrap(),
        ),
        (
            K::RunQueued,
            WorkEventPayload::run_queued(RunKind::Executor, 1, Some(contract()), 0, 1, None)
                .unwrap(),
        ),
        (
            K::RunClaimed,
            WorkEventPayload::run_lifecycle(K::RunClaimed, RunKind::Planner, 1, 0, 0).unwrap(),
        ),
        (
            K::RunStarted,
            WorkEventPayload::run_lifecycle(K::RunStarted, RunKind::Planner, 1, 0, 0).unwrap(),
        ),
        (
            K::RunHeartbeat,
            WorkEventPayload::run_heartbeat(RunKind::Reviewer, 1, 0, 0, 0).unwrap(),
        ),
        (
            K::RunCompleted,
            WorkEventPayload::run_completed(RunKind::Planner, 1, RunTerminalKind::Plan).unwrap(),
        ),
        (
            K::RunWaitingForApproval,
            WorkEventPayload::run_waiting_for_approval(
                RunKind::Reviewer,
                1,
                gate(),
                TaskGateKind::Approval,
            )
            .unwrap(),
        ),
        (
            K::RunInterrupted,
            WorkEventPayload::run_failure(
                K::RunInterrupted,
                RunKind::Planner,
                1,
                0,
                safe_code(),
                false,
            )
            .unwrap(),
        ),
        (
            K::RunFailed,
            WorkEventPayload::run_failure(K::RunFailed, RunKind::Planner, 1, 0, safe_code(), false)
                .unwrap(),
        ),
        (
            K::RunCancelRequested,
            WorkEventPayload::run_cancelled(
                K::RunCancelRequested,
                RunKind::Planner,
                1,
                RunCancellationReason::Command,
            )
            .unwrap(),
        ),
        (
            K::RunCancelled,
            WorkEventPayload::run_cancelled(
                K::RunCancelled,
                RunKind::Planner,
                1,
                RunCancellationReason::Command,
            )
            .unwrap(),
        ),
        (
            K::SubmissionCreated,
            WorkEventPayload::submission_created("submission:one".to_string(), contract(), 1, 1, 0)
                .unwrap(),
        ),
        (
            K::ReviewCreated,
            WorkEventPayload::review_created(
                "review:one".to_string(),
                "submission:one".to_string(),
                contract(),
                1,
                1,
                None,
                TaskReviewVerdict::Approve,
            )
            .unwrap(),
        ),
        (
            K::NotificationQueued,
            WorkEventPayload::notification(
                K::NotificationQueued,
                "notification:one".to_string(),
                1,
                NotificationKind::TaskCreated,
                Some(NotificationDestination::HumanPrimaryConversation),
                None,
                None,
                None,
            )
            .unwrap(),
        ),
        (
            K::NotificationDelivered,
            WorkEventPayload::notification(
                K::NotificationDelivered,
                "notification:one".to_string(),
                1,
                NotificationKind::TaskReviewReady,
                None,
                Some(1),
                None,
                None,
            )
            .unwrap(),
        ),
        (
            K::NotificationFailed,
            WorkEventPayload::notification(
                K::NotificationFailed,
                "notification:one".to_string(),
                1,
                NotificationKind::TaskRecovery,
                None,
                Some(1),
                Some(safe_code()),
                Some(false),
            )
            .unwrap(),
        ),
    ]
}

#[test]
fn event_validation_rejects_unknown_sensitive_and_oversized_fields() {
    let mut unknown = record(WorkEventPayload::project_created(1).unwrap());
    unknown.safe_payload["unexpected"] = json!(true);
    assert!(unknown.validate().is_err());

    let mut sensitive = record(WorkEventPayload::project_created(1).unwrap());
    sensitive.safe_payload["provider_payload"] = json!("raw");
    assert!(sensitive.validate().is_err());

    let mut oversized = record(WorkEventPayload::project_created(1).unwrap());
    oversized.safe_payload["revision"] = json!("x".repeat(16 * 1024));
    assert!(oversized.validate().is_err());
}

#[test]
fn event_validation_rejects_bad_types_enums_ids_duplicates_and_prose_codes() {
    let mut bad_enum = record(WorkEventPayload::project_created(1).unwrap());
    bad_enum.kind = WorkEventKind::TaskCaptured;
    assert!(bad_enum.validate().is_err());

    let mut grouped_kind = record(WorkEventPayload::project_created(1).unwrap());
    grouped_kind.kind = WorkEventKind::ProjectArchived;
    assert!(grouped_kind.validate().is_err());

    let mut bad_type = record(WorkEventPayload::project_created(1).unwrap());
    bad_type.safe_payload["revision"] = json!("one");
    assert!(bad_type.validate().is_err());

    let mut bad_source_kind = record(
        WorkEventPayload::task_captured(
            1,
            1,
            WorkflowStageId::new("stage:personal:inbox").unwrap(),
            TaskSourceKind::WorkUi,
        )
        .unwrap(),
    );
    bad_source_kind.safe_payload["source_kind"] = json!("bogus");
    assert!(bad_source_kind.validate().is_err());

    let mut bad_id = record(WorkEventPayload::project_created(1).unwrap());
    bad_id.safe_payload["revision"] = json!(1);
    bad_id.kind = WorkEventKind::TaskCaptured;
    bad_id.safe_payload = json!({
        "v": 1,
        "revision": 1,
        "generation": 1,
        "stage_id": "stage:bad\nvalue",
        "source_kind": "work_ui"
    });
    assert!(bad_id.validate().is_err());

    let mut bad_fields = record(
        WorkEventPayload::project_updated(1, vec![crate::ProjectChangedField::Name]).unwrap(),
    );
    bad_fields.safe_payload["changed_fields"] = json!(["name", "name"]);
    assert!(bad_fields.validate().is_err());
    bad_fields.safe_payload["changed_fields"] = json!(["prose"]);
    assert!(bad_fields.validate().is_err());

    let mut bad_code = record(
        WorkEventPayload::run_failure(
            WorkEventKind::RunFailed,
            crate::RunKind::Planner,
            1,
            0,
            SafeErrorCode::new("safe_code").unwrap(),
            false,
        )
        .unwrap(),
    );
    bad_code.safe_payload["error_code"] = json!("provider failed because the answer was secret");
    assert!(bad_code.validate().is_err());
}
