use noema_tasks::{AcceptTask, WorkCommand, WorkDomainError, WorkflowStageBehavior};
use rusqlite::{OptionalExtension, params};

use super::{WorkCommandService, helpers, validation};
use crate::StoreError;

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
        let write = helpers::accept_review_tx(
            transaction,
            &mut task,
            &review_id,
            &submission_id,
            helpers::event_context(&command.meta),
            None,
            false,
        )?;
        helpers::finish_write_tx(transaction, &envelope, write)
    }).await?;
    Ok(write)
}
