use noema_tasks::{
    AgentRunRecord, RunKind, RunStatus, TaskGateKind, TaskRecoveryReason, WorkDomainError,
    WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::{
    WorkRunFence,
    rows::{self, load_run_tx},
};
use crate::{
    StoreError,
    ids::allocate_id,
    run_items::finish_agent_run_records_tx,
    work_commands::helpers,
    work_events::{WORK_EVENT_COLUMNS, WorkEventScope, decode_work_event_record},
};

pub(super) fn load_running_fence_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: RunKind,
) -> Result<(AgentRunRecord, helpers::TaskState), StoreError> {
    let (run, task) = load_fenced_run_tx(transaction, fence, Some(expected_kind), false)?;
    if run.status != RunStatus::Running {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    Ok((run, task))
}

/// Load only the immutable run identity needed to recognize a committed
/// terminal replay. Current task generation and lease checks belong to fresh
/// writes and happen only after replay lookup misses.
pub(super) fn load_terminal_run_identity_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: Option<RunKind>,
) -> Result<AgentRunRecord, StoreError> {
    let run = load_run_tx(transaction, &fence.run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if run.task_generation != fence.task_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    if expected_kind.is_some_and(|kind| run.run_kind != kind) {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    Ok(run)
}

pub(super) fn load_fenced_run_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: Option<RunKind>,
    allow_expired_lease: bool,
) -> Result<(AgentRunRecord, helpers::TaskState), StoreError> {
    let run = load_run_tx(transaction, &fence.run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if run.task_generation != fence.task_generation
        || run.lease_token.as_deref() != Some(fence.lease_token.as_str())
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let lease_valid: bool = transaction.query_row(
        if allow_expired_lease {
            "SELECT lease_expires_at IS NOT NULL AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2"
        } else {
            "SELECT lease_expires_at IS NOT NULL AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2"
        },
        params![fence.run_id.as_str(), fence.lease_token.as_str()],
        |row| row.get(0),
    )?;
    if !lease_valid {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if expected_kind.is_some_and(|kind| run.run_kind != kind) {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
    if task.generation != fence.task_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    Ok((run, task))
}

pub(super) fn mark_run_completed_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    rows::execute_fenced_update_tx(
        transaction,
        "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'running' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?3 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?3 AND cancellation_requested = 0",
        params![run.run_id, fence.lease_token, fence.task_generation],
    )?;
    finish_agent_run_records_tx(transaction, &run.run_id, RunStatus::Completed)
}

pub(super) fn mark_run_waiting_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    rows::execute_fenced_update_tx(
        transaction,
        "UPDATE agent_runs SET status = 'waiting_for_approval', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, ended_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'running' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?3 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?3 AND cancellation_requested = 0",
        params![run.run_id, fence.lease_token, fence.task_generation],
    )
}

pub(super) fn bump_task_to_waiting_tx(
    transaction: &Transaction<'_>,
    task: &mut helpers::TaskState,
) -> Result<u64, StoreError> {
    let revision = helpers::increment(task.revision, "task.revision")?;
    let changed = transaction.execute(
        "UPDATE tasks SET stage_id = 'stage:personal:waiting', active_gate_id = (SELECT gate_id FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND gate_state = 'open'), queued_at = NULL, revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?2 AND revision = ?4",
        params![task.task_id.as_str(), task.generation, revision, task.revision],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    task.stage_id = WorkflowStageId::new("stage:personal:waiting").map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::HumanGate;
    task.active_gate_id = transaction.query_row("SELECT gate_id FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND gate_state = 'open'", params![task.task_id.as_str(), task.generation], |row| row.get::<_, String>(0)).optional()?.map(noema_tasks::TaskGateId::new).transpose().map_err(StoreError::Work)?;
    task.revision = revision;
    Ok(revision)
}

pub(super) struct OpenGate<'a> {
    pub gate_kind: TaskGateKind,
    pub prompt: &'a str,
    pub context: &'a str,
    pub suggested_answers: &'a [String],
    pub actor_id: &'a str,
    pub originating_run_id: Option<&'a str>,
    pub recovery_reason: Option<TaskRecoveryReason>,
    pub retry_run_kind: Option<RunKind>,
}

pub(super) fn insert_gate_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    request: OpenGate<'_>,
) -> Result<noema_tasks::TaskGateId, StoreError> {
    let gate_id = noema_tasks::TaskGateId::new(allocate_id("gate")).map_err(StoreError::Work)?;
    let suggested_answers_json = serde_json::to_string(request.suggested_answers)?;
    transaction.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, suggested_answers_json, opened_by_actor_id, originating_run_id) VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![gate_id.as_str(), task.task_id.as_str(), task.generation, request.gate_kind.as_str(), request.recovery_reason.map(|value| value.as_str()), request.retry_run_kind.map(|value| value.as_str()), request.prompt, request.context, suggested_answers_json, request.actor_id, request.originating_run_id],
    )?;
    Ok(gate_id)
}

pub(super) fn latest_run_event_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<noema_tasks::WorkEventRecord, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 ORDER BY event_sequence DESC LIMIT 1"),
            [run_id],
            decode_work_event_record,
        )
        .map_err(StoreError::Sqlite)
}

fn run_queued_event_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<noema_tasks::WorkEventRecord, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 AND event_kind = 'run.queued' ORDER BY event_sequence LIMIT 1"),
            [run_id],
            decode_work_event_record,
        )
        .map_err(StoreError::Sqlite)
}

pub(super) fn notification_queued_event_for_run_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
    kind: noema_tasks::NotificationKind,
) -> Result<Option<noema_tasks::WorkEventRecord>, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 AND event_kind = 'notification.queued' AND json_extract(payload_json, '$.notification_kind') = ?2 ORDER BY event_sequence LIMIT 1"),
            params![run_id, kind.as_str()],
            decode_work_event_record,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(super) fn child_run_event_tx(
    transaction: &Transaction<'_>,
    parent_run_id: &str,
) -> Result<Option<(String, noema_tasks::WorkEventRecord)>, StoreError> {
    let child_run_id: Option<String> = transaction
        .query_row(
            "SELECT run_id FROM agent_runs WHERE parent_run_id = ?1 ORDER BY created_at, run_id LIMIT 1",
            [parent_run_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(child_run_id) = child_run_id else {
        return Ok(None);
    };
    let event = run_queued_event_tx(transaction, &child_run_id)?;
    Ok(Some((child_run_id, event)))
}

pub(super) fn scope(
    task: &helpers::TaskState,
    event: (&str, Option<&str>, &str),
    run_id: Option<&str>,
) -> WorkEventScope {
    let (actor_id, causation_id, correlation_id) = event;
    rows::event_scope(task, run_id, actor_id, causation_id, correlation_id)
}

pub(super) fn run_scope(
    task: &helpers::TaskState,
    event: (&str, Option<&str>, &str),
    run_id: &str,
) -> WorkEventScope {
    scope(task, event, Some(run_id))
}
