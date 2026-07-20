use std::str::FromStr;

use noema_tasks::{
    ContractOrigin, NewTaskValidationCriterion, PERSONAL_DONE_STAGE_ID, PERSONAL_QUEUE_STAGE_ID,
    RequestTaskChanges, RunKind, TaskComplexity, TaskMessageKind, WorkCommand, WorkDomainError,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, params};

use super::{WorkCommandService, helpers, validation};
use crate::{StoreError, ids::allocate_id, work_events::append_work_event_tx};

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &RequestTaskChanges,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::RequestTaskChanges(command.clone());
    let task_id = command.precondition.task_id.clone();
    let write = service.store.with_immediate_transaction_retry(|transaction| {
        if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? {
            return Ok(replay);
        }
        let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        if task.stage_behavior != WorkflowStageBehavior::Acceptance || task.active_gate_id.is_some() {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let current_contract_id = task.current_contract_id.clone().ok_or(StoreError::Work(WorkDomainError::ReviewNotApproved))?;
        let review_id: String = transaction.query_row(
            "SELECT latest_review_id FROM tasks WHERE task_id = ?1",
            [task_id.as_str()],
            |row| row.get(0),
        ).optional()?.flatten().ok_or(StoreError::Work(WorkDomainError::ReviewNotApproved))?;
        let approved: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_reviews WHERE review_id = ?1 AND task_id = ?2 AND contract_id = ?3 AND overall_verdict = 'approve')",
            params![review_id, task_id.as_str(), current_contract_id.as_str()],
            |row| row.get(0),
        )?;
        if !approved {
            return Err(StoreError::Work(WorkDomainError::ReviewNotApproved));
        }
        validation::validate_current_approval_tx(
            transaction,
            &task,
            &review_id,
            &task.latest_submission_id.clone().ok_or(StoreError::Work(WorkDomainError::ReviewNotApproved))?,
            &current_contract_id,
        )?;
        let (current_request, complexity_wire): (String, String) = transaction.query_row(
            "SELECT request_markdown, complexity FROM task_execution_contracts WHERE contract_id = ?1 AND task_id = ?2 AND task_generation = ?3",
            params![current_contract_id.as_str(), task_id.as_str(), task.generation],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let current_complexity = TaskComplexity::from_str(&complexity_wire).map_err(StoreError::Work)?;
        let complexity = command.amendment.complexity.unwrap_or(current_complexity);
        let request_markdown = command.amendment.request_markdown.as_deref().unwrap_or(&current_request);
        let criteria = if let Some(replacement) = &command.amendment.replacement_criteria {
            replacement.clone()
        } else {
            let rows = transaction.prepare(
                "SELECT ordinal, description, expected_evidence FROM task_contract_criteria WHERE contract_id = ?1 ORDER BY ordinal, criterion_id",
            )?.query_map([current_contract_id.as_str()], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
            })?.collect::<Result<Vec<_>, _>>()?;
            rows.into_iter().map(|(ordinal, description, expected_evidence)| {
                Ok(NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: u32::try_from(ordinal).map_err(|_| StoreError::InvariantViolation {
                        message: "contract criterion ordinal exceeds u32".to_string(),
                    })?,
                    description,
                    expected_evidence,
                })
            }).collect::<Result<Vec<_>, StoreError>>()?
        };
        if criteria.is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "contract.criteria",
                message: "current contract has no criteria".to_string(),
            }));
        }
        let next_generation = helpers::increment(task.generation, "task.generation")?;
        let next_revision = helpers::increment(task.revision, "task.revision")?;
        let mut next_task = task.clone();
        next_task.generation = next_generation;
        next_task.revision = next_revision;
        next_task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
        next_task.stage_behavior = WorkflowStageBehavior::Dispatch;
        next_task.current_contract_id = None;
        next_task.active_gate_id = None;
        next_task.latest_run_id = None;
        next_task.latest_submission_id = None;
        next_task.latest_review_id = None;
        let (contract_id, _) = super::super::tasks::create_contract_tx(
            transaction,
            service.provider_registry.as_ref(),
            &next_task,
            super::super::tasks::CreateContract {
                origin: ContractOrigin::HumanRevision,
                request_markdown,
                execution_plan_markdown: None,
                criteria: &criteria,
                complexity,
                supersedes_contract_id: Some(&current_contract_id),
                event: helpers::event_context(&command.meta),
            },
        )?;
        let message_id = noema_tasks::TaskMessageId::new(allocate_id("task_message")).map_err(StoreError::Work)?;
        transaction.execute(
            "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, review_id, message_kind, body_markdown, author_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, 'human_change_request', ?6, ?7)",
            params![message_id.as_str(), task_id.as_str(), next_generation, contract_id.as_str(), review_id, command.amendment.feedback_markdown, command.meta.actor_id],
        )?;
        transaction.execute(
            "UPDATE tasks SET generation = ?2, revision = ?3, stage_id = ?4, current_contract_id = ?5, active_gate_id = NULL, latest_run_id = NULL, latest_submission_id = NULL, latest_review_id = NULL, accepted_submission_id = NULL, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), completed_at = NULL, cancelled_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?6 AND revision = ?7",
            params![task_id.as_str(), next_generation, next_revision, PERSONAL_QUEUE_STAGE_ID, contract_id.as_str(), task.generation, task.revision],
        )?;
        let _message_event = append_work_event_tx(transaction, helpers::event_context(&command.meta).task_scope(&next_task, None), WorkEventPayload::task_message_appended(message_id, next_generation, TaskMessageKind::HumanChangeRequest, None, Some(contract_id.clone())).map_err(StoreError::Work)?)?;
        let _queued_event = append_work_event_tx(transaction, helpers::event_context(&command.meta).task_scope(&next_task, None), WorkEventPayload::task_queued(next_revision, next_generation, Some(contract_id.clone()), RunKind::Executor).map_err(StoreError::Work)?)?;
        let _stage_event = append_work_event_tx(transaction, helpers::event_context(&command.meta).task_scope(&next_task, None), WorkEventPayload::task_stage_changed(next_revision, next_generation, WorkflowStageId::new(PERSONAL_DONE_STAGE_ID).map_err(StoreError::Work)?, next_task.stage_id.clone(), noema_tasks::TaskStageChangeReason::RequestChanges).map_err(StoreError::Work)?)?;
        let (run_id, run_event) = helpers::queue_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            &next_task,
            helpers::QueueRun {
                run_kind: RunKind::Executor,
                contract_id: Some(&contract_id),
                planner_complexity: None,
                review_round: 1,
                attempt_index: 0,
                parent_run_id: None,
                triggering_submission_id: None,
                triggering_review_id: Some(&review_id),
                event: helpers::event_context(&command.meta),
            },
        )?;
        helpers::finish_write_tx(
            transaction,
            &envelope,
            helpers::task_write(run_event, task_id.clone()).contract(Some(contract_id)).run(Some(run_id)),
        )
    }).await?;
    Ok(write)
}
