//! Shared human-recovery transitions for stopped Work runs.

use noema_tasks::{
    RunKind, TaskGateKind, TaskRecoveryReason, TaskStageChangeReason, WorkDomainError,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{Transaction, params};

use super::helpers;
use crate::{
    StoreError, ids::allocate_id, work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(crate) struct ConfigurationRecovery<'a> {
    pub retry_run_kind: RunKind,
    pub originating_run_id: Option<&'a str>,
    pub event: helpers::CommandEventContext<'a>,
}

pub(crate) struct RecoveryGate<'a> {
    pub recovery_reason: TaskRecoveryReason,
    pub retry_run_kind: RunKind,
    pub prompt: &'a str,
    pub context: &'a str,
    pub originating_run_id: Option<&'a str>,
    pub event: helpers::CommandEventContext<'a>,
}

pub(crate) fn open_configuration_recovery_tx(
    transaction: &Transaction<'_>,
    task: &mut helpers::TaskState,
    request: ConfigurationRecovery<'_>,
) -> Result<helpers::CommandWrite, StoreError> {
    let ConfigurationRecovery {
        retry_run_kind,
        originating_run_id,
        event,
    } = request;
    open_recovery_gate_tx(
        transaction,
        task,
        RecoveryGate {
            recovery_reason: TaskRecoveryReason::ConfigurationUnavailable,
            retry_run_kind,
            prompt: "The selected provider route is unavailable.",
            context: "Choose Retry after restoring the configured provider route.",
            originating_run_id,
            event,
        },
    )
}

pub(crate) fn open_recovery_gate_tx(
    transaction: &Transaction<'_>,
    task: &mut helpers::TaskState,
    request: RecoveryGate<'_>,
) -> Result<helpers::CommandWrite, StoreError> {
    let RecoveryGate {
        recovery_reason,
        retry_run_kind,
        prompt,
        context,
        originating_run_id,
        event,
    } = request;
    let gate_id = noema_tasks::TaskGateId::new(allocate_id("gate")).map_err(StoreError::Work)?;
    transaction.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES (?1, ?2, ?3, 'recovery', 'open', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            gate_id.as_str(),
            task.task_id.as_str(),
            task.generation,
            recovery_reason.as_str(),
            retry_run_kind.as_str(),
            prompt,
            context,
            event.actor_id,
            originating_run_id,
        ],
    )?;
    let revision = helpers::increment(task.revision, "task.revision")?;
    let from_stage = task.stage_id.clone();
    let changed = transaction.execute(
        "UPDATE tasks SET stage_id = 'stage:personal:waiting', active_gate_id = ?2,
                queued_at = NULL, revision = ?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE task_id = ?1 AND generation = ?4 AND revision = ?5",
        params![
            task.task_id.as_str(),
            gate_id.as_str(),
            revision,
            task.generation,
            task.revision
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    task.stage_id = WorkflowStageId::new("stage:personal:waiting").map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::HumanGate;
    task.active_gate_id = Some(gate_id.clone());
    task.revision = revision;
    let scope = || {
        event.scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            originating_run_id,
        )
    };
    let _gate_event = append_work_event_tx(
        transaction,
        scope(),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            TaskGateKind::Recovery,
            originating_run_id.map(ToOwned::to_owned),
            Some(recovery_reason),
            Some(retry_run_kind),
        )
        .map_err(StoreError::Work)?,
    )?;
    let mut event = append_work_event_tx(
        transaction,
        scope(),
        WorkEventPayload::task_stage_changed(
            revision,
            task.generation,
            from_stage,
            task.stage_id.clone(),
            TaskStageChangeReason::Recovery,
        )
        .map_err(StoreError::Work)?,
    )?;
    if let Some(notification_event) = enqueue_work_notification_tx(
        transaction,
        &event,
        noema_tasks::NotificationKind::TaskRecovery,
        &serde_json::json!({
            "task_id": task.task_id.as_str(),
            "gate_id": gate_id.as_str(),
            "action_needed": true,
        }),
    )? {
        event = notification_event;
    }
    Ok(helpers::task_write(event, task.task_id.clone())
        .project(task.project_id.clone())
        .gate(Some(gate_id))
        .run(originating_run_id.map(ToOwned::to_owned)))
}
