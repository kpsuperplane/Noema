use std::{collections::BTreeSet, str::FromStr};

use serde_json::{Map, Value};

use crate::gate::recovery_fields_are_valid;
use crate::{
    ContractOrigin, RunKind, TaskComplexity, TaskContractId, TaskGateId, TaskGateKind,
    TaskMessageId, TaskMessageKind, TaskRecoveryReason, TaskReviewVerdict, TaskSourceKind,
    WorkDomainError, WorkflowStageId, error::invalid_input,
};

use super::{
    GateResolutionKind, GateSupersessionReason, NotificationDestination, NotificationKind,
    ProjectChangedField, RunCancellationReason, RunTerminalKind, SafeErrorCode, TaskChangedField,
    TaskStageChangeReason, WorkEventKind,
};

pub(crate) fn validate_payload(
    kind: WorkEventKind,
    payload: &Value,
) -> Result<(), WorkDomainError> {
    let encoded = serde_json::to_vec(payload).map_err(|error| {
        invalid_input(
            "work_event.safe_payload",
            format!("payload is not encodable: {error}"),
        )
    })?;
    if encoded.len() > 16 * 1024 {
        return Err(invalid_input(
            "work_event.safe_payload",
            "payload exceeds the 16 KiB ledger limit",
        ));
    }
    let object = payload
        .as_object()
        .ok_or_else(|| invalid_input("work_event.safe_payload", "payload must be an object"))?;
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(invalid_input(
            "work_event.safe_payload",
            "payload must contain numeric v=1",
        ));
    }
    let fields = super::event_payload_fields(kind);
    if object.len() != fields.len() + 1
        || fields.iter().any(|field| !object.contains_key(*field))
        || object
            .keys()
            .any(|field| field != "v" && !fields.contains(&field.as_str()))
    {
        return Err(invalid_input(
            "work_event.safe_payload",
            format!("payload fields do not match {}", kind.as_str()),
        ));
    }
    for field in fields {
        validate_field(field, &object[*field])?;
    }
    validate_invariants(kind, object)
}

fn validate_field(field: &'static str, value: &Value) -> Result<(), WorkDomainError> {
    match field {
        "revision"
        | "generation"
        | "version"
        | "criteria_count"
        | "review_attempt_index"
        | "source_event_sequence"
        | "attempt_count" => positive(value, field),
        "attempt_index"
        | "review_round"
        | "provider_call_count"
        | "tool_call_count"
        | "active_milliseconds"
        | "artifact_count" => number(value, field).map(|_| ()),
        "reason_present" | "retryable" => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(invalid_input(field, "value must be a boolean"))
            }
        }
        "stage_id" | "from_stage_id" | "to_stage_id" => {
            id::<WorkflowStageId>(value, field).map(drop)
        }
        "gate_id" => nullable(value, |value| id::<TaskGateId>(value, field)).map(drop),
        "message_id" => id::<TaskMessageId>(value, field).map(drop),
        "contract_id" | "supersedes_contract_id" => {
            nullable(value, |value| id::<TaskContractId>(value, field)).map(drop)
        }
        "submission_id" => external_id(value, field, "submission:"),
        "review_id" => external_id(value, field, "review:"),
        "notification_id" => external_id(value, field, "notification:"),
        "recurrence_id" => external_id(value, field, "recurrence:"),
        "consumed_by_run_id" => external_id(value, field, "run:"),
        "originating_run_id" | "parent_run_id" => {
            nullable(value, |value| external_id(value, field, "run:")).map(drop)
        }
        "supersedes_review_id" => {
            nullable(value, |value| external_id(value, field, "review:")).map(drop)
        }
        "source_kind" => closed::<TaskSourceKind>(value, field).map(drop),
        "origin" => closed::<ContractOrigin>(value, field).map(drop),
        "complexity" => closed::<TaskComplexity>(value, field).map(drop),
        "gate_kind" => closed::<TaskGateKind>(value, field).map(drop),
        "recovery_reason" => {
            nullable(value, |value| closed::<TaskRecoveryReason>(value, field)).map(drop)
        }
        "retry_run_kind" => nullable(value, |value| closed::<RunKind>(value, field)).map(drop),
        "resolution_kind" => closed::<GateResolutionKind>(value, field).map(drop),
        "message_kind" => closed::<TaskMessageKind>(value, field).map(drop),
        "run_kind" | "next_run_kind" => closed::<RunKind>(value, field).map(drop),
        "terminal_kind" => closed::<RunTerminalKind>(value, field).map(drop),
        "verdict" => closed::<TaskReviewVerdict>(value, field).map(drop),
        "notification_kind" => closed::<NotificationKind>(value, field).map(drop),
        "destination_kind" => closed::<NotificationDestination>(value, field).map(drop),
        "error_code" => SafeErrorCode::new(string(value, field)?).map(drop),
        "changed_fields" | "reason" => Ok(()),
        _ => Err(invalid_input(field, "unknown event field")),
    }
}

fn validate_invariants(
    kind: WorkEventKind,
    object: &Map<String, Value>,
) -> Result<(), WorkDomainError> {
    use WorkEventKind as K;
    match kind {
        K::ProjectUpdated => changed_fields::<ProjectChangedField>(&object["changed_fields"]),
        K::TaskUpdated => changed_fields::<TaskChangedField>(&object["changed_fields"]),
        K::RecurrenceChanged => string(&object["reason"], "reason").map(drop),
        K::TaskQueued => closed::<RunKind>(&object["next_run_kind"], "next_run_kind").map(drop),
        K::TaskStageChanged => {
            closed::<TaskStageChangeReason>(&object["reason"], "reason").map(drop)
        }
        K::ContractCreated | K::SubmissionCreated | K::ReviewCreated => {
            id::<TaskContractId>(&object["contract_id"], "contract_id")?;
            if matches!(kind, K::SubmissionCreated | K::ReviewCreated) {
                positive(&object["review_round"], "review_round")?;
            }
            Ok(())
        }
        K::GateOpened => {
            id::<TaskGateId>(&object["gate_id"], "gate_id")?;
            let valid = recovery_fields_are_valid(
                closed(&object["gate_kind"], "gate_kind")?,
                nullable(&object["recovery_reason"], |value| {
                    closed(value, "recovery_reason")
                })?,
                nullable(&object["retry_run_kind"], |value| {
                    closed(value, "retry_run_kind")
                })?,
            );
            if valid {
                Ok(())
            } else {
                Err(invalid_input(
                    "recovery_reason",
                    "recovery reason and continuation role are inconsistent",
                ))
            }
        }
        K::GateResolved | K::GateSuperseded | K::RunWaitingForApproval => {
            id::<TaskGateId>(&object["gate_id"], "gate_id")?;
            if kind == K::GateSuperseded {
                closed::<GateSupersessionReason>(&object["reason"], "reason")?;
            }
            Ok(())
        }
        K::RunQueued | K::RunClaimed | K::RunStarted => validate_run(object),
        K::RunCompleted => validate_terminal_role(
            closed(&object["run_kind"], "run_kind")?,
            closed(&object["terminal_kind"], "terminal_kind")?,
        ),
        K::RunCancelRequested | K::RunCancelled => {
            closed::<RunCancellationReason>(&object["reason"], "reason").map(drop)
        }
        _ => Ok(()),
    }
}

fn validate_run(object: &Map<String, Value>) -> Result<(), WorkDomainError> {
    let run = closed::<RunKind>(&object["run_kind"], "run_kind")?;
    match run {
        RunKind::Planner if number(&object["review_round"], "review_round")? == 0 => {}
        RunKind::Executor | RunKind::Reviewer => positive(&object["review_round"], "review_round")?,
        _ => return Err(invalid_input("review_round", "Planner rounds must be zero")),
    }
    Ok(())
}

fn string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, WorkDomainError> {
    value
        .as_str()
        .ok_or_else(|| invalid_input(field, "value must be a string"))
}

fn closed<T: FromStr<Err = WorkDomainError>>(
    value: &Value,
    field: &'static str,
) -> Result<T, WorkDomainError> {
    T::from_str(string(value, field)?).map_err(|_| invalid_input(field, "unknown closed value"))
}

fn nullable<T>(
    value: &Value,
    validate: impl FnOnce(&Value) -> Result<T, WorkDomainError>,
) -> Result<Option<T>, WorkDomainError> {
    if value.is_null() {
        Ok(None)
    } else {
        validate(value).map(Some)
    }
}

fn id<T>(value: &Value, field: &'static str) -> Result<T, WorkDomainError>
where
    for<'a> T: TryFrom<&'a str, Error = WorkDomainError>,
{
    T::try_from(string(value, field)?).map_err(|_| invalid_input(field, "invalid identifier"))
}

fn external_id(
    value: &Value,
    field: &'static str,
    prefix: &'static str,
) -> Result<(), WorkDomainError> {
    let value = string(value, field)?;
    if value.trim().is_empty()
        || value.len() > 255
        || value.chars().any(char::is_control)
        || value
            .strip_prefix(prefix)
            .is_none_or(|suffix| suffix.trim().is_empty())
    {
        Err(invalid_input(field, "invalid identifier"))
    } else {
        Ok(())
    }
}

fn number(value: &Value, field: &'static str) -> Result<u64, WorkDomainError> {
    value
        .as_u64()
        .ok_or_else(|| invalid_input(field, "value must be an unsigned integer"))
}

fn positive(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    if number(value, field)? == 0 {
        Err(invalid_input(field, "value must be positive"))
    } else {
        Ok(())
    }
}

fn changed_fields<T: FromStr<Err = WorkDomainError>>(value: &Value) -> Result<(), WorkDomainError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_input("changed_fields", "value must be an array"))?;
    let mut seen = BTreeSet::new();
    if values.is_empty()
        || values
            .iter()
            .any(|value| closed::<T>(value, "changed_fields").is_err())
    {
        return Err(invalid_input(
            "changed_fields",
            "array cannot be empty or invalid",
        ));
    }
    for value in values {
        let field = string(value, "changed_fields")?;
        if !seen.insert(field) {
            return Err(invalid_input("changed_fields", "fields cannot repeat"));
        }
    }
    Ok(())
}

fn validate_terminal_role(run: RunKind, terminal: RunTerminalKind) -> Result<(), WorkDomainError> {
    if matches!(
        (run, terminal),
        (
            RunKind::Planner,
            RunTerminalKind::Plan | RunTerminalKind::GateResolved
        ) | (
            RunKind::Executor,
            RunTerminalKind::Submission | RunTerminalKind::GateResolved
        ) | (
            RunKind::Reviewer,
            RunTerminalKind::Review | RunTerminalKind::GateResolved
        )
    ) {
        Ok(())
    } else {
        Err(invalid_input(
            "terminal_kind",
            "terminal kind does not match run role",
        ))
    }
}
