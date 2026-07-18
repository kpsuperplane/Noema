use std::{collections::BTreeSet, str::FromStr};

use serde_json::Value;

use crate::{
    ContractOrigin, RunKind, TaskComplexity, TaskContractId, TaskGateId, TaskGateKind,
    TaskMessageId, TaskMessageKind, TaskRecoveryReason, TaskReviewVerdict, TaskSourceKind,
    WorkDomainError, WorkflowStageId, error::invalid_input,
};

use super::{
    GateResolutionKind, GateSupersessionReason, NotificationDestination, NotificationKind,
    ProjectChangedField, RunCancellationReason, RunTerminalKind, SafeErrorCode, TaskChangedField,
    TaskStageChangeReason, WorkEventKind, event_schema,
};

/// Validate the redacted, versioned payload for one event kind.
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
    match object.get("v") {
        Some(Value::Number(version)) if version.as_u64() == Some(1) => {}
        _ => {
            return Err(invalid_input(
                "work_event.safe_payload",
                "payload must contain numeric v=1",
            ));
        }
    }
    if contains_sensitive_key(payload) {
        return Err(invalid_input(
            "work_event.safe_payload",
            "payload contains a secret or raw/private field",
        ));
    }
    let (allowed, required) = event_schema::payload_contract(kind);
    for key in object.keys() {
        if !allowed.iter().any(|candidate| candidate == key) {
            return Err(invalid_input(
                "work_event.safe_payload",
                format!("field {key} is not allowed for {}", kind.as_str()),
            ));
        }
    }
    for key in required {
        if !object.contains_key(*key) {
            return Err(invalid_input(
                "work_event.safe_payload",
                format!("required field {key} is missing for {}", kind.as_str()),
            ));
        }
    }
    validate_shape(kind, object)
}

fn validate_shape(
    kind: WorkEventKind,
    object: &serde_json::Map<String, Value>,
) -> Result<(), WorkDomainError> {
    use WorkEventKind as K;
    match kind {
        K::ProjectCreated | K::ProjectArchived | K::ProjectReopened => {
            positive_u64(required(object, "revision")?, "revision")?;
        }
        K::ProjectUpdated => {
            positive_u64(required(object, "revision")?, "revision")?;
            project_changed_fields(required(object, "changed_fields")?)?;
        }
        K::TaskCaptured => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            typed_id::<WorkflowStageId>(required(object, "stage_id")?, "stage_id")?;
            enum_value::<TaskSourceKind>(required(object, "source_kind")?, "source_kind")?;
        }
        K::TaskUpdated => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            task_changed_fields(required(object, "changed_fields")?)?;
        }
        K::TaskQueued => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            let contract_id = optional_typed_id::<TaskContractId>(
                required(object, "contract_id")?,
                "contract_id",
            )?;
            let run_kind =
                enum_value::<RunKind>(required(object, "next_run_kind")?, "next_run_kind")?;
            role_contract_compatibility(run_kind, contract_id.is_some(), "task.queued")?;
        }
        K::TaskStageChanged => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            typed_id::<WorkflowStageId>(required(object, "from_stage_id")?, "from_stage_id")?;
            typed_id::<WorkflowStageId>(required(object, "to_stage_id")?, "to_stage_id")?;
            enum_value::<TaskStageChangeReason>(required(object, "reason")?, "reason")?;
        }
        K::TaskCancelled => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            boolean(required(object, "reason_present")?, "reason_present")?;
        }
        K::TaskReopened => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            typed_id::<WorkflowStageId>(required(object, "stage_id")?, "stage_id")?;
        }
        K::TaskAccepted => {
            positive_u64(required(object, "revision")?, "revision")?;
            positive_u64(required(object, "generation")?, "generation")?;
            external_id(
                required(object, "submission_id")?,
                "submission_id",
                "submission:",
            )?;
            external_id(required(object, "review_id")?, "review_id", "review:")?;
        }
        K::ContractCreated => {
            typed_id::<TaskContractId>(required(object, "contract_id")?, "contract_id")?;
            positive_u32(required(object, "version")?, "version")?;
            positive_u64(required(object, "generation")?, "generation")?;
            enum_value::<ContractOrigin>(required(object, "origin")?, "origin")?;
            enum_value::<TaskComplexity>(required(object, "complexity")?, "complexity")?;
            positive_u32(required(object, "criteria_count")?, "criteria_count")?;
            optional_typed_id::<TaskContractId>(
                required(object, "supersedes_contract_id")?,
                "supersedes_contract_id",
            )?;
        }
        K::GateOpened => {
            typed_id::<TaskGateId>(required(object, "gate_id")?, "gate_id")?;
            positive_u64(required(object, "generation")?, "generation")?;
            let gate_kind =
                enum_value::<TaskGateKind>(required(object, "gate_kind")?, "gate_kind")?;
            optional_external_id(
                required(object, "originating_run_id")?,
                "originating_run_id",
                "run:",
            )?;
            let recovery_reason = optional_enum::<TaskRecoveryReason>(
                required(object, "recovery_reason")?,
                "recovery_reason",
            )?;
            let retry_run_kind =
                optional_enum::<RunKind>(required(object, "retry_run_kind")?, "retry_run_kind")?;
            validate_gate_recovery(gate_kind, recovery_reason, retry_run_kind)?;
        }
        K::GateResolved => {
            typed_id::<TaskGateId>(required(object, "gate_id")?, "gate_id")?;
            positive_u64(required(object, "generation")?, "generation")?;
            enum_value::<TaskGateKind>(required(object, "gate_kind")?, "gate_kind")?;
            typed_id::<TaskMessageId>(required(object, "message_id")?, "message_id")?;
            enum_value::<GateResolutionKind>(
                required(object, "resolution_kind")?,
                "resolution_kind",
            )?;
        }
        K::GateSuperseded => {
            typed_id::<TaskGateId>(required(object, "gate_id")?, "gate_id")?;
            positive_u64(required(object, "generation")?, "generation")?;
            enum_value::<TaskGateKind>(required(object, "gate_kind")?, "gate_kind")?;
            enum_value::<GateSupersessionReason>(required(object, "reason")?, "reason")?;
        }
        K::TaskMessageAppended => {
            typed_id::<TaskMessageId>(required(object, "message_id")?, "message_id")?;
            positive_u64(required(object, "generation")?, "generation")?;
            enum_value::<TaskMessageKind>(required(object, "message_kind")?, "message_kind")?;
            optional_typed_id::<TaskGateId>(required(object, "gate_id")?, "gate_id")?;
            optional_typed_id::<TaskContractId>(required(object, "contract_id")?, "contract_id")?;
        }
        K::TaskMessageConsumed => {
            typed_id::<TaskMessageId>(required(object, "message_id")?, "message_id")?;
            positive_u64(required(object, "generation")?, "generation")?;
            external_id(
                required(object, "consumed_by_run_id")?,
                "consumed_by_run_id",
                "run:",
            )?;
        }
        K::RunQueued => {
            let run_kind = enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            let contract_id = optional_typed_id::<TaskContractId>(
                required(object, "contract_id")?,
                "contract_id",
            )?;
            nonnegative_u32(required(object, "attempt_index")?, "attempt_index")?;
            review_round(required(object, "review_round")?, run_kind)?;
            optional_external_id(required(object, "parent_run_id")?, "parent_run_id", "run:")?;
            role_contract_compatibility(run_kind, contract_id.is_some(), "run.queued")?;
        }
        K::RunClaimed | K::RunStarted => {
            let run_kind = enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            nonnegative_u32(required(object, "attempt_index")?, "attempt_index")?;
            review_round(required(object, "review_round")?, run_kind)?;
        }
        K::RunHeartbeat => {
            enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            nonnegative_u32(
                required(object, "provider_call_count")?,
                "provider_call_count",
            )?;
            nonnegative_u32(required(object, "tool_call_count")?, "tool_call_count")?;
            nonnegative_u64(
                required(object, "active_milliseconds")?,
                "active_milliseconds",
            )?;
        }
        K::RunCompleted => {
            let run_kind = enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            let terminal =
                enum_value::<RunTerminalKind>(required(object, "terminal_kind")?, "terminal_kind")?;
            validate_terminal_role(run_kind, terminal)?;
        }
        K::RunWaitingForApproval => {
            enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            typed_id::<TaskGateId>(required(object, "gate_id")?, "gate_id")?;
            enum_value::<TaskGateKind>(required(object, "gate_kind")?, "gate_kind")?;
        }
        K::RunInterrupted | K::RunFailed => {
            enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            nonnegative_u32(required(object, "attempt_index")?, "attempt_index")?;
            safe_error(required(object, "error_code")?, "error_code")?;
            boolean(required(object, "retryable")?, "retryable")?;
        }
        K::RunCancelRequested | K::RunCancelled => {
            enum_value::<RunKind>(required(object, "run_kind")?, "run_kind")?;
            positive_u64(required(object, "generation")?, "generation")?;
            enum_value::<RunCancellationReason>(required(object, "reason")?, "reason")?;
        }
        K::SubmissionCreated => {
            external_id(
                required(object, "submission_id")?,
                "submission_id",
                "submission:",
            )?;
            typed_id::<TaskContractId>(required(object, "contract_id")?, "contract_id")?;
            positive_u32(required(object, "review_round")?, "review_round")?;
            positive_u32(required(object, "criteria_count")?, "criteria_count")?;
            nonnegative_u32(required(object, "artifact_count")?, "artifact_count")?;
        }
        K::ReviewCreated => {
            external_id(required(object, "review_id")?, "review_id", "review:")?;
            external_id(
                required(object, "submission_id")?,
                "submission_id",
                "submission:",
            )?;
            typed_id::<TaskContractId>(required(object, "contract_id")?, "contract_id")?;
            positive_u32(required(object, "review_round")?, "review_round")?;
            positive_u32(
                required(object, "review_attempt_index")?,
                "review_attempt_index",
            )?;
            optional_external_id(
                required(object, "supersedes_review_id")?,
                "supersedes_review_id",
                "review:",
            )?;
            enum_value::<TaskReviewVerdict>(required(object, "verdict")?, "verdict")?;
        }
        K::NotificationQueued => {
            external_id(
                required(object, "notification_id")?,
                "notification_id",
                "notification:",
            )?;
            positive_u64(
                required(object, "source_event_sequence")?,
                "source_event_sequence",
            )?;
            enum_value::<NotificationKind>(
                required(object, "notification_kind")?,
                "notification_kind",
            )?;
            enum_value::<NotificationDestination>(
                required(object, "destination_kind")?,
                "destination_kind",
            )?;
        }
        K::NotificationDelivered => {
            external_id(
                required(object, "notification_id")?,
                "notification_id",
                "notification:",
            )?;
            positive_u64(
                required(object, "source_event_sequence")?,
                "source_event_sequence",
            )?;
            enum_value::<NotificationKind>(
                required(object, "notification_kind")?,
                "notification_kind",
            )?;
            positive_u32(required(object, "attempt_count")?, "attempt_count")?;
        }
        K::NotificationFailed => {
            external_id(
                required(object, "notification_id")?,
                "notification_id",
                "notification:",
            )?;
            positive_u64(
                required(object, "source_event_sequence")?,
                "source_event_sequence",
            )?;
            enum_value::<NotificationKind>(
                required(object, "notification_kind")?,
                "notification_kind",
            )?;
            positive_u32(required(object, "attempt_count")?, "attempt_count")?;
            safe_error(required(object, "error_code")?, "error_code")?;
            boolean(required(object, "retryable")?, "retryable")?;
        }
    }
    Ok(())
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a Value, WorkDomainError> {
    object
        .get(field)
        .ok_or_else(|| invalid_input("work_event.safe_payload", format!("missing field {field}")))
}

fn string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, WorkDomainError> {
    value
        .as_str()
        .ok_or_else(|| invalid_input(field, "value must be a string"))
}

fn enum_value<T>(value: &Value, field: &'static str) -> Result<T, WorkDomainError>
where
    T: FromStr<Err = WorkDomainError>,
{
    T::from_str(string(value, field)?).map_err(|_| invalid_input(field, "unknown closed value"))
}

fn optional_enum<T>(value: &Value, field: &'static str) -> Result<Option<T>, WorkDomainError>
where
    T: FromStr<Err = WorkDomainError>,
{
    if value.is_null() {
        Ok(None)
    } else {
        enum_value(value, field).map(Some)
    }
}

fn typed_id<T>(value: &Value, field: &'static str) -> Result<T, WorkDomainError>
where
    for<'a> T: TryFrom<&'a str, Error = WorkDomainError>,
{
    T::try_from(string(value, field)?).map_err(|_| invalid_input(field, "invalid identifier"))
}

fn optional_typed_id<T>(value: &Value, field: &'static str) -> Result<Option<T>, WorkDomainError>
where
    for<'a> T: TryFrom<&'a str, Error = WorkDomainError>,
{
    if value.is_null() {
        Ok(None)
    } else {
        typed_id(value, field).map(Some)
    }
}

fn external_id(
    value: &Value,
    field: &'static str,
    prefix: &'static str,
) -> Result<(), WorkDomainError> {
    let value = string(value, field)?;
    if value.trim().is_empty()
        || value.len() > 255
        || !value.starts_with(prefix)
        || value[prefix.len()..].trim().is_empty()
        || value.chars().any(char::is_control)
    {
        return Err(invalid_input(field, "invalid identifier"));
    }
    Ok(())
}

fn optional_external_id(
    value: &Value,
    field: &'static str,
    prefix: &'static str,
) -> Result<(), WorkDomainError> {
    if value.is_null() {
        Ok(())
    } else {
        external_id(value, field, prefix)
    }
}

fn number(value: &Value, field: &'static str) -> Result<u64, WorkDomainError> {
    value
        .as_u64()
        .ok_or_else(|| invalid_input(field, "value must be an unsigned integer"))
}

fn positive_u64(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    if number(value, field)? == 0 {
        Err(invalid_input(field, "value must be positive"))
    } else {
        Ok(())
    }
}

fn positive_u32(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    positive_u64(value, field)
}

fn nonnegative_u32(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    number(value, field).map(|_| ())
}

fn nonnegative_u64(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    number(value, field).map(|_| ())
}

fn boolean(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    if value.is_boolean() {
        Ok(())
    } else {
        Err(invalid_input(field, "value must be a boolean"))
    }
}

fn safe_error(value: &Value, field: &'static str) -> Result<(), WorkDomainError> {
    SafeErrorCode::new(string(value, field)?).map(|_| ())
}

fn project_changed_fields(value: &Value) -> Result<(), WorkDomainError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_input("changed_fields", "value must be an array"))?;
    if values.is_empty() {
        return Err(invalid_input("changed_fields", "array cannot be empty"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        let field = enum_value::<ProjectChangedField>(value, "changed_fields")?;
        if !seen.insert(field.as_str()) {
            return Err(invalid_input("changed_fields", "fields cannot repeat"));
        }
    }
    Ok(())
}

fn task_changed_fields(value: &Value) -> Result<(), WorkDomainError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_input("changed_fields", "value must be an array"))?;
    if values.is_empty() {
        return Err(invalid_input("changed_fields", "array cannot be empty"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        let field = enum_value::<TaskChangedField>(value, "changed_fields")?;
        if !seen.insert(field.as_str()) {
            return Err(invalid_input("changed_fields", "fields cannot repeat"));
        }
    }
    Ok(())
}

fn review_round(value: &Value, run_kind: RunKind) -> Result<(), WorkDomainError> {
    match run_kind {
        RunKind::Planner => {
            if number(value, "review_round")? != 0 {
                return Err(invalid_input("review_round", "Planner rounds must be zero"));
            }
            Ok(())
        }
        RunKind::Executor | RunKind::Reviewer => positive_u32(value, "review_round"),
    }
}

fn role_contract_compatibility(
    run_kind: RunKind,
    has_contract: bool,
    field: &'static str,
) -> Result<(), WorkDomainError> {
    let valid = match run_kind {
        RunKind::Planner => !has_contract,
        RunKind::Executor | RunKind::Reviewer => has_contract,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_input(
            field,
            "run role and contract presence are inconsistent",
        ))
    }
}

fn validate_terminal_role(
    run_kind: RunKind,
    terminal: RunTerminalKind,
) -> Result<(), WorkDomainError> {
    let valid = matches!(
        (run_kind, terminal),
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
    );
    if valid {
        Ok(())
    } else {
        Err(invalid_input(
            "terminal_kind",
            "terminal kind does not match run role",
        ))
    }
}

fn validate_gate_recovery(
    gate_kind: TaskGateKind,
    reason: Option<TaskRecoveryReason>,
    retry_run_kind: Option<RunKind>,
) -> Result<(), WorkDomainError> {
    match gate_kind {
        TaskGateKind::Recovery => match (reason, retry_run_kind) {
            (Some(TaskRecoveryReason::InvariantFault), None)
            | (Some(TaskRecoveryReason::ReviewRoundsExhausted), Some(RunKind::Executor))
            | (
                Some(
                    TaskRecoveryReason::InfrastructureRetriesExhausted
                    | TaskRecoveryReason::UnsafeEffectUncertain
                    | TaskRecoveryReason::ConfigurationUnavailable,
                ),
                Some(_),
            ) => Ok(()),
            _ => Err(invalid_input(
                "recovery_reason",
                "recovery reason and continuation role are inconsistent",
            )),
        },
        TaskGateKind::Clarification | TaskGateKind::Approval
            if reason.is_none() && retry_run_kind.is_none() =>
        {
            Ok(())
        }
        _ => Err(invalid_input(
            "gate_kind",
            "only Recovery gates carry recovery continuation fields",
        )),
    }
}

fn contains_sensitive_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            let normalized = key.to_ascii_lowercase();
            [
                "secret",
                "credential",
                "password",
                "token",
                "prompt",
                "answer",
                "description",
                "result",
                "feedback",
                "transcript",
                "provider_payload",
                "lease",
            ]
            .iter()
            .any(|needle| normalized.contains(needle))
                || contains_sensitive_key(value)
        }),
        Value::Array(values) => values.iter().any(contains_sensitive_key),
        _ => false,
    }
}
