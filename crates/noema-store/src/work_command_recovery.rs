//! Shared configuration-recovery transition for child-run admission failures.

use noema_tasks::{
    RunKind, TaskGateKind, TaskRecoveryReason, TaskStageChangeReason, WorkDomainError,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{Transaction, params};

use super::helpers;
use crate::{
    StoreError,
    ids::allocate_id,
    work_events::{WorkEventScope, append_work_event_tx},
    work_notifications::enqueue_work_notification_tx,
};

pub(crate) struct ConfigurationRecovery<'a> {
    pub retry_run_kind: RunKind,
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
    let gate_id = noema_tasks::TaskGateId::new(allocate_id("gate")).map_err(StoreError::Work)?;
    transaction.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES (?1, ?2, ?3, ?4, 'recovery', 'open', 'configuration_unavailable', ?5, 'The selected provider route is unavailable.', 'Choose Retry after restoring the configured provider route.', ?6, ?7)",
        params![
            gate_id.as_str(),
            task.task_id.as_str(),
            task.generation,
            task.current_contract_id.as_ref().map(ToString::to_string),
            retry_run_kind.as_str(),
            event.actor_id,
            originating_run_id,
        ],
    )?;
    let revision = task.revision.checked_add(1).ok_or_else(|| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "task.revision",
            message: "revision overflow".to_string(),
        })
    })?;
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
    let scope = || WorkEventScope {
        workspace_id: task.workspace_id.clone(),
        project_id: task.project_id.clone(),
        task_id: Some(task.task_id.clone()),
        run_id: originating_run_id.map(ToOwned::to_owned),
        actor_id: event.actor_id.to_string(),
        causation_id: event.causation_id.map(ToOwned::to_owned),
        correlation_id: event.correlation_id.to_string(),
    };
    let _gate_event = append_work_event_tx(
        transaction,
        scope(),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            TaskGateKind::Recovery,
            originating_run_id.map(ToOwned::to_owned),
            Some(TaskRecoveryReason::ConfigurationUnavailable),
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
        &serde_json::json!({"task_id": task.task_id.as_str(), "gate_id": gate_id.as_str()}),
    )? {
        event = notification_event;
    }
    Ok(helpers::write_marker(
        event,
        Some(task.task_id.clone()),
        task.project_id.clone(),
        task.current_contract_id.clone(),
        Some(gate_id),
        originating_run_id.map(ToOwned::to_owned),
    ))
}
