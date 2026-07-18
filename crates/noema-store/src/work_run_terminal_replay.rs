use noema_tasks::{AgentRunRecord, RunKind, RunStatus, TaskGateId, WorkDomainError, WorkEventKind};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    ReportRunFailure, ReportTaskBlocked, SubmitTaskResult, WorkRunTerminal,
    terminal_helpers::{
        child_run_event_tx, contract_criterion_ids_tx, latest_run_event_tx,
        load_terminal_run_identity_tx, notification_queued_event_for_run_tx, submission_matches_tx,
        validate_namespace,
    },
    terminal_plan, terminal_review,
};
use crate::{StoreError, work_commands::helpers, work_events::work_event_from_row};

pub(super) fn replay_terminal_tx(
    transaction: &Transaction<'_>,
    terminal: &WorkRunTerminal,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    match terminal {
        WorkRunTerminal::Plan(command) => terminal_plan::replay_tx(transaction, command),
        WorkRunTerminal::TaskResult(command) => replay_result_tx(transaction, command),
        WorkRunTerminal::Review(command) => terminal_review::replay_tx(transaction, command),
        WorkRunTerminal::Blocked(command) => replay_blocked_tx(transaction, command),
    }
}

fn replay_result_tx(
    transaction: &Transaction<'_>,
    command: &SubmitTaskResult,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let run = load_terminal_run_identity_tx(transaction, &command.fence, Some(RunKind::Executor))?;
    let contract_id = command
        .fence
        .contract_id
        .as_ref()
        .ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
    let criterion_ids = contract_criterion_ids_tx(transaction, contract_id)?;
    let submission = command
        .submission
        .normalized(&criterion_ids)
        .map_err(StoreError::Work)?;
    if submission.task_id != run.task_id
        || submission.contract_id != *contract_id
        || submission.executor_run_id != run.run_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if let Some(submission_id) = submission.submission_id.as_deref() {
        validate_namespace(submission_id, "submission:", "submission.submission_id")?;
    }
    let existing: Option<(String, String)> = transaction
        .query_row(
            "SELECT submission_id, contract_id FROM task_submissions WHERE executor_run_id = ?1",
            [run.run_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((existing_id, existing_contract)) = existing else {
        return Ok(None);
    };
    if existing_contract != contract_id.as_str()
        || !submission_matches_tx(transaction, &existing_id, &submission)?
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let child = child_run_event_tx(transaction, &run.run_id)?;
    let (event, result_run_id) = child.map(|(run_id, event)| (event, run_id)).unwrap_or((
        latest_run_event_tx(transaction, &run.run_id)?,
        run.run_id.clone(),
    ));
    Ok(Some(helpers::write_marker(
        event,
        Some(run.task_id),
        None,
        Some(contract_id.clone()),
        None,
        Some(result_run_id),
    )))
}

pub(super) fn replay_blocked_tx(
    transaction: &Transaction<'_>,
    report: &ReportTaskBlocked,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let run = load_terminal_run_identity_tx(transaction, &report.fence, Some(RunKind::Executor))?;
    if matches!(run.status, RunStatus::Leased | RunStatus::Running) {
        return Ok(None);
    }
    if run.run_kind != RunKind::Executor {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let Some(terminal_event) = run_waiting_event_tx(transaction, &run.run_id)? else {
        return Ok(None);
    };
    let gate_id = terminal_event
        .safe_payload
        .get("gate_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("blocked run {} has no waiting gate identity", run.run_id),
        })?;
    let gate = transaction
        .query_row(
            "SELECT gate_id, gate_kind, prompt_markdown, context_markdown
             FROM task_gates WHERE gate_id = ?1 AND originating_run_id = ?2",
            params![gate_id, run.run_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((gate_id, gate_kind, prompt, context)) = gate else {
        return Err(StoreError::InvariantViolation {
            message: format!("blocked run {} references a missing gate", run.run_id),
        });
    };
    if gate_kind != report.gate_kind.as_str()
        || normalize_required(&prompt) != normalize_required(&report.prompt_markdown)
        || normalize_optional(&context) != normalize_optional(&report.context_markdown)
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    if !blocked_payload_matches(&terminal_event.safe_payload, &run, &gate_id, report) {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let event = notification_for_event_tx(transaction, &terminal_event)?.unwrap_or(terminal_event);
    let project_id = event.project_id.clone();
    Ok(Some(helpers::write_marker(
        event,
        Some(run.task_id),
        project_id,
        run.contract_id,
        Some(TaskGateId::new(gate_id).map_err(StoreError::Work)?),
        Some(run.run_id),
    )))
}

pub(super) fn replay_failure_tx(
    transaction: &Transaction<'_>,
    report: &ReportRunFailure,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let run = load_terminal_run_identity_tx(transaction, &report.fence, None)?;
    if matches!(run.status, RunStatus::Leased | RunStatus::Running) {
        return Ok(None);
    }
    let expected_kind = if report.status == RunStatus::Interrupted {
        WorkEventKind::RunInterrupted
    } else {
        WorkEventKind::RunFailed
    };
    let events = transaction
        .prepare(
            "SELECT event_sequence, event_id, event_kind, workspace_id, project_id, task_id,
                    run_id, actor_id, causation_id, correlation_id, payload_json, created_at
             FROM work_events WHERE run_id = ?1
               AND event_kind IN ('run.interrupted', 'run.failed')
             ORDER BY event_sequence LIMIT 2",
        )?
        .query_map([run.run_id.as_str()], work_event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    if events.is_empty() {
        return Ok(None);
    }
    if events.len() != 1
        || run.status != report.status
        || run.error_code.as_deref() != Some(report.error_code.as_str())
        || normalize_optional(run.error_message.as_deref().unwrap_or(""))
            != normalize_failure_message(report.error_message.as_deref())
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let event = &events[0];
    if event.kind != expected_kind || !failure_payload_matches(&event.safe_payload, &run, report) {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let recovery_gate = recovery_gate_id_tx(transaction, &run.run_id)?;
    let (event, gate_id, result_run_id) = if let Some(gate_id) = recovery_gate {
        let notification = notification_queued_event_for_run_tx(
            transaction,
            &run.run_id,
            noema_tasks::NotificationKind::TaskRecovery,
        )?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("failure recovery gate {gate_id} has no notification marker"),
        })?;
        (
            notification,
            Some(TaskGateId::new(gate_id).map_err(StoreError::Work)?),
            run.run_id.clone(),
        )
    } else if let Some((child_run_id, event)) = child_run_event_tx(transaction, &run.run_id)? {
        (event, None, child_run_id)
    } else {
        return Err(StoreError::InvariantViolation {
            message: format!("failed run {} has no committed recovery branch", run.run_id),
        });
    };
    let project_id = event.project_id.clone();
    Ok(Some(helpers::write_marker(
        event,
        Some(run.task_id),
        project_id,
        run.contract_id,
        gate_id,
        Some(result_run_id),
    )))
}

fn blocked_payload_matches(
    payload: &serde_json::Value,
    run: &AgentRunRecord,
    gate_id: &str,
    report: &ReportTaskBlocked,
) -> bool {
    payload.get("run_kind").and_then(serde_json::Value::as_str) == Some(run.run_kind.as_str())
        && payload
            .get("generation")
            .and_then(serde_json::Value::as_u64)
            == Some(run.task_generation)
        && payload.get("gate_id").and_then(serde_json::Value::as_str) == Some(gate_id)
        && payload.get("gate_kind").and_then(serde_json::Value::as_str)
            == Some(report.gate_kind.as_str())
}

fn failure_payload_matches(
    payload: &serde_json::Value,
    run: &AgentRunRecord,
    report: &ReportRunFailure,
) -> bool {
    payload.get("run_kind").and_then(serde_json::Value::as_str) == Some(run.run_kind.as_str())
        && payload
            .get("generation")
            .and_then(serde_json::Value::as_u64)
            == Some(run.task_generation)
        && payload
            .get("attempt_index")
            .and_then(serde_json::Value::as_u64)
            == Some(u64::from(run.attempt_index))
        && payload
            .get("error_code")
            .and_then(serde_json::Value::as_str)
            == Some(report.error_code.as_str())
        && payload
            .get("retryable")
            .and_then(serde_json::Value::as_bool)
            == Some(report.retryable)
}

fn run_waiting_event_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<noema_tasks::WorkEventRecord>, StoreError> {
    transaction
        .query_row(
            "SELECT event_sequence, event_id, event_kind, workspace_id, project_id, task_id, run_id, actor_id, causation_id, correlation_id, payload_json, created_at FROM work_events WHERE run_id = ?1 AND event_kind = 'run.waiting_for_approval' ORDER BY event_sequence LIMIT 1",
            [run_id],
            work_event_from_row,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn notification_for_event_tx(
    transaction: &Transaction<'_>,
    event: &noema_tasks::WorkEventRecord,
) -> Result<Option<noema_tasks::WorkEventRecord>, StoreError> {
    transaction
        .query_row(
            "SELECT event_sequence, event_id, event_kind, workspace_id, project_id, task_id, run_id, actor_id, causation_id, correlation_id, payload_json, created_at FROM work_events WHERE causation_id = ?1 AND event_kind = 'notification.queued' ORDER BY event_sequence LIMIT 1",
            [event.event_id.as_str()],
            work_event_from_row,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn recovery_gate_id_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<String>, StoreError> {
    transaction
        .query_row(
            "SELECT gate_id FROM task_gates WHERE originating_run_id = ?1 AND gate_kind = 'recovery' AND recovery_reason IS NOT NULL ORDER BY opened_at, gate_id LIMIT 1",
            [run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(super) fn normalize_required(value: &str) -> String {
    value.trim().to_string()
}

pub(super) fn normalize_optional(value: &str) -> String {
    value.trim().to_string()
}

pub(super) fn normalize_failure_message(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("run failed")
        .to_string()
}
