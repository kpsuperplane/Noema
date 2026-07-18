use std::str::FromStr;

use noema_tasks::{
    CancelTask, GateSupersessionReason, PERSONAL_CANCELLED_STAGE_ID, PERSONAL_INBOX_STAGE_ID,
    ReopenTask, RunKind, TaskGateKind, WorkCommand, WorkDomainError, WorkEventKind,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, params};

use super::{WorkCommandService, helpers, scope, scope_with_run};
use crate::{StoreError, work_events::append_work_event_tx};

pub(super) async fn cancel(
    service: &WorkCommandService,
    command: &CancelTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::CancelTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    let write = service.store.with_immediate_transaction_retry(|transaction| {
        if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? { return Ok(replay); }
        let mut task = helpers::load_task_state_tx(transaction, &task_id)?;
        helpers::check_task_fence(&task, command.precondition.expected_revision, command.precondition.expected_generation)?;
        if task.stage_behavior.is_terminal() { return Err(StoreError::Work(WorkDomainError::InvalidTransition)); }
        let from_stage = task.stage_id.clone();
        let open_gate = if let Some(gate_id) = task.active_gate_id.clone() {
            let kind = transaction
                .query_row(
                    "SELECT gate_kind FROM task_gates WHERE gate_id = ?1 AND task_id = ?2 AND task_generation = ?3 AND gate_state = 'open'",
                    params![gate_id.as_str(), task.task_id.as_str(), task.generation],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task {} points at a missing open gate", task.task_id),
                })?;
            Some((gate_id, TaskGateKind::from_str(&kind).map_err(StoreError::Work)?))
        } else {
            None
        };
        let runs = transaction.prepare("SELECT run_id, run_kind, status, task_generation FROM agent_runs WHERE task_id = ?1 AND status IN ('queued', 'leased', 'running', 'waiting_for_approval') ORDER BY created_at, run_id")?.query_map([task_id.as_str()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?)))?.collect::<Result<Vec<_>, _>>()?;
        for (run_id, run_kind, status, run_generation) in runs {
            let kind = RunKind::from_str(&run_kind).map_err(StoreError::Work)?;
            let run_generation = helpers::positive_u64(run_generation, "run.task_generation")?;
            let event_kind = if matches!(status.as_str(), "leased" | "running") { WorkEventKind::RunCancelRequested } else { WorkEventKind::RunCancelled };
            transaction.execute("UPDATE agent_runs SET status = 'cancelled', cancellation_requested = 1, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status IN ('queued', 'leased', 'running', 'waiting_for_approval')", [run_id.as_str()])?;
            append_work_event_tx(transaction, scope_with_run(&task, &run_id, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::run_cancelled(event_kind, kind, run_generation, noema_tasks::RunCancellationReason::Command).map_err(StoreError::Work)?)?;
            if event_kind == WorkEventKind::RunCancelRequested {
                append_work_event_tx(transaction, scope_with_run(&task, &run_id, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::run_cancelled(WorkEventKind::RunCancelled, kind, run_generation, noema_tasks::RunCancellationReason::Command).map_err(StoreError::Work)?)?;
            }
        }
        if let Some((gate_id, kind)) = open_gate {
            transaction.execute("UPDATE task_gates SET gate_state = 'superseded', resolved_by_actor_id = ?2, resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE gate_id = ?1 AND gate_state = 'open'", params![gate_id.as_str(), command.meta.actor_id])?;
            append_work_event_tx(transaction, scope(&task, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::gate_superseded(gate_id, task.generation, kind, GateSupersessionReason::Cancelled).map_err(StoreError::Work)?)?;
        }
        let generation = task.generation.checked_add(1).ok_or_else(|| StoreError::Work(WorkDomainError::InvalidInput { field: "task.generation", message: "generation overflow".to_string() }))?;
        let revision = task.revision.checked_add(1).ok_or_else(|| StoreError::Work(WorkDomainError::InvalidInput { field: "task.revision", message: "revision overflow".to_string() }))?;
        transaction.execute("UPDATE tasks SET generation = ?2, revision = ?3, stage_id = ?4, current_contract_id = NULL, active_gate_id = NULL, latest_run_id = NULL, latest_submission_id = NULL, latest_review_id = NULL, accepted_submission_id = NULL, cancelled_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), completed_at = NULL, queued_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?5 AND revision = ?6", params![task_id.as_str(), generation, revision, PERSONAL_CANCELLED_STAGE_ID, task.generation, task.revision])?;
        task.generation = generation; task.revision = revision; task.stage_id = WorkflowStageId::new(PERSONAL_CANCELLED_STAGE_ID).map_err(StoreError::Work)?; task.stage_behavior = WorkflowStageBehavior::TerminalCancelled; task.current_contract_id = None; task.latest_run_id = None; task.latest_submission_id = None; task.latest_review_id = None;
        let _cancelled_event = append_work_event_tx(transaction, scope(&task, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::task_cancelled(revision, generation, command.reason.is_some()).map_err(StoreError::Work)?)?;
        let event = append_work_event_tx(transaction, scope(&task, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::task_stage_changed(revision, generation, from_stage, task.stage_id.clone(), noema_tasks::TaskStageChangeReason::Cancelled).map_err(StoreError::Work)?)?;
        let mut write = helpers::write_marker(event, Some(task_id.clone()), None, None, None, None); helpers::save_receipt_tx(transaction, &envelope, &mut write)?; Ok(write)
    }).await?;
    Ok(write)
}

pub(super) async fn reopen(
    service: &WorkCommandService,
    command: &ReopenTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::ReopenTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    let write = service.store.with_immediate_transaction_retry(|transaction| {
        if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? { return Ok(replay); }
        let mut task = helpers::load_task_state_tx(transaction, &task_id)?;
        helpers::check_task_fence(&task, command.precondition.expected_revision, command.precondition.expected_generation)?;
        if !task.stage_behavior.is_terminal() { return Err(StoreError::Work(WorkDomainError::InvalidTransition)); }
        let generation = task.generation.checked_add(1).ok_or_else(|| StoreError::Work(WorkDomainError::InvalidInput { field: "task.generation", message: "generation overflow".to_string() }))?;
        let revision = task.revision.checked_add(1).ok_or_else(|| StoreError::Work(WorkDomainError::InvalidInput { field: "task.revision", message: "revision overflow".to_string() }))?;
        transaction.execute("UPDATE tasks SET generation = ?2, revision = ?3, stage_id = ?4, current_contract_id = NULL, active_gate_id = NULL, latest_run_id = NULL, latest_submission_id = NULL, latest_review_id = NULL, accepted_submission_id = NULL, queued_at = NULL, completed_at = NULL, cancelled_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?5 AND revision = ?6", params![task_id.as_str(), generation, revision, PERSONAL_INBOX_STAGE_ID, task.generation, task.revision])?;
        let from_stage = task.stage_id.clone(); task.generation = generation; task.revision = revision; task.stage_id = WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?; task.stage_behavior = WorkflowStageBehavior::Intake;
        let _reopened_event = append_work_event_tx(transaction, scope(&task, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::task_reopened(revision, generation, task.stage_id.clone()).map_err(StoreError::Work)?)?;
        let event = append_work_event_tx(transaction, scope(&task, &command.meta.actor_id, command.meta.causation_id.as_deref(), &command.meta.correlation_id), WorkEventPayload::task_stage_changed(revision, generation, from_stage, task.stage_id.clone(), noema_tasks::TaskStageChangeReason::Reopened).map_err(StoreError::Work)?)?;
        let mut write = helpers::write_marker(event, Some(task_id.clone()), None, None, None, None); helpers::save_receipt_tx(transaction, &envelope, &mut write)?; Ok(write)
    }).await?;
    Ok(write)
}
