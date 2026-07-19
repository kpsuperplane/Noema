use noema_tasks::{
    AcceptTask, PERSONAL_COMPLETED_STAGE_ID, PERSONAL_REVIEW_STAGE_ID, WorkCommand,
    WorkDomainError, WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, params};

use super::{WorkCommandService, helpers, validation};
use crate::{
    StoreError, work_events::append_work_event_tx, work_notifications::enqueue_work_notification_tx,
};

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &AcceptTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::AcceptTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    let write = service.store.with_immediate_transaction_retry(|transaction| {
        if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? { return Ok(replay); }
        let mut task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        if task.stage_behavior != WorkflowStageBehavior::Acceptance || task.active_gate_id.is_some() { return Err(StoreError::Work(WorkDomainError::InvalidTransition)); }
        let review = transaction.query_row("SELECT latest_review_id, latest_submission_id FROM tasks WHERE task_id = ?1", [task_id.as_str()], |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))).optional()?.ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
        let (review_id, submission_id) = review;
        let (Some(review_id), Some(submission_id)) = (review_id, submission_id) else { return Err(StoreError::Work(WorkDomainError::ReviewNotApproved)); };
        let verdict: String = transaction.query_row("SELECT overall_verdict FROM task_reviews WHERE review_id = ?1 AND task_id = ?2 AND contract_id = ?3", params![review_id, task_id.as_str(), task.current_contract_id.as_ref().map(ToString::to_string)], |row| row.get(0)).optional()?.ok_or(StoreError::Work(WorkDomainError::ReviewNotApproved))?;
        if verdict != "approve" { return Err(StoreError::Work(WorkDomainError::ReviewNotApproved)); }
        validation::validate_current_approval_tx(
            transaction,
            &task,
            &review_id,
            &submission_id,
            task.current_contract_id.as_ref().ok_or(StoreError::Work(
                WorkDomainError::ContractRequired,
            ))?,
        )?;
        let revision = helpers::increment(task.revision, "task.revision")?;
        transaction.execute("UPDATE tasks SET stage_id = ?2, accepted_submission_id = ?3, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), queued_at = NULL, revision = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?5 AND generation = ?6", params![task_id.as_str(), PERSONAL_COMPLETED_STAGE_ID, submission_id, revision, task.revision, task.generation])?;
        task.stage_id = WorkflowStageId::new(PERSONAL_COMPLETED_STAGE_ID).map_err(StoreError::Work)?; task.stage_behavior = WorkflowStageBehavior::TerminalSuccess; task.revision = revision;
        let _accepted_event = append_work_event_tx(transaction, helpers::event_context(&command.meta).task_scope(&task, None), WorkEventPayload::task_accepted(revision, task.generation, submission_id.clone(), review_id.clone()).map_err(StoreError::Work)?)?;
        let mut event = append_work_event_tx(transaction, helpers::event_context(&command.meta).task_scope(&task, None), WorkEventPayload::task_stage_changed(revision, task.generation, WorkflowStageId::new(PERSONAL_REVIEW_STAGE_ID).map_err(StoreError::Work)?, task.stage_id.clone(), noema_tasks::TaskStageChangeReason::Accepted).map_err(StoreError::Work)?)?;
        if let Some(notification_event) = enqueue_work_notification_tx(
            transaction,
            &event,
            noema_tasks::NotificationKind::TaskAccepted,
            &serde_json::json!({"task_id": task_id.as_str(), "submission_id": submission_id}),
        )? {
            event = notification_event;
        }
        helpers::finish_write_tx(transaction, &envelope, helpers::task_write(event, task_id.clone()).contract(task.current_contract_id))
    }).await?;
    Ok(write)
}
