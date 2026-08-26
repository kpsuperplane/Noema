//! Reopen completed or cancelled Tasks against their current files.

use noema_tasks::{
    PERSONAL_QUEUE_STAGE_ID, ReopenTask, RunKind, TaskMessageKind, WorkCommand, WorkDomainError,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{Transaction, params};

use super::{WorkCommandService, helpers};
use crate::{StoreError, ids::allocate_id, work_events::append_work_event_tx};

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &ReopenTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::ReopenTask(command.clone());
    service
        .store
        .with_immediate_transaction_retry(|transaction| {
            if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? {
                return Ok(replay);
            }
            let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
            if !matches!(
                task.stage_behavior,
                WorkflowStageBehavior::TerminalSuccess | WorkflowStageBehavior::TerminalCancelled
            ) || task.active_gate_id.is_some()
            {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let write = reopen_tx(transaction, service, command, task)?;
            helpers::finish_write_tx(transaction, &envelope, write)
        })
        .await
}

fn reopen_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    command: &ReopenTask,
    task: helpers::TaskState,
) -> Result<helpers::CommandWrite, StoreError> {
    let next_generation = helpers::increment(task.generation, "task.generation")?;
    let next_revision = helpers::increment(task.revision, "task.revision")?;
    let from_stage = task.stage_id.clone();
    let mut next_task = task.clone();
    next_task.generation = next_generation;
    next_task.revision = next_revision;
    next_task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
    next_task.stage_behavior = WorkflowStageBehavior::Dispatch;
    next_task.active_gate_id = None;
    next_task.latest_run_id = None;
    next_task.execution_complexity = command.direction.complexity.or(task.execution_complexity);
    next_task.executor_agent_id = executor_for_reopen(transaction, &task.executor_agent_id)?;

    let message_id =
        noema_tasks::TaskMessageId::new(allocate_id("task_message")).map_err(StoreError::Work)?;
    let direction = reopen_direction(command);
    transaction.execute(
        "INSERT INTO task_messages (message_id, task_id, task_generation, message_kind, body_markdown, author_actor_id) VALUES (?1, ?2, ?3, 'human_change_request', ?4, ?5)",
        params![
            message_id.as_str(),
            task.task_id.as_str(),
            next_generation,
            direction,
            command.meta.actor_id
        ],
    )?;
    let changed = transaction.execute(
        "UPDATE tasks SET generation = ?2, revision = ?3, stage_id = ?4, active_gate_id = NULL, latest_run_id = NULL, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), completed_at = NULL, cancelled_at = NULL, executor_agent_id = ?7, execution_complexity = ?8, current_review_decision = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?5 AND revision = ?6",
        params![
            task.task_id.as_str(),
            next_generation,
            next_revision,
            PERSONAL_QUEUE_STAGE_ID,
            task.generation,
            task.revision,
            next_task.executor_agent_id,
            next_task.execution_complexity.map(|value| value.as_str()),
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }

    let event_context = helpers::event_context(&command.meta);
    append_work_event_tx(
        transaction,
        event_context.task_scope(&next_task, None),
        WorkEventPayload::task_message_appended(
            message_id,
            next_generation,
            TaskMessageKind::HumanChangeRequest,
            None,
        )
        .map_err(StoreError::Work)?,
    )?;
    append_work_event_tx(
        transaction,
        event_context.task_scope(&next_task, None),
        WorkEventPayload::task_reopened(next_revision, next_generation, next_task.stage_id.clone())
            .map_err(StoreError::Work)?,
    )?;
    append_work_event_tx(
        transaction,
        event_context.task_scope(&next_task, None),
        WorkEventPayload::task_queued(next_revision, next_generation, RunKind::Executor)
            .map_err(StoreError::Work)?,
    )?;
    append_work_event_tx(
        transaction,
        event_context.task_scope(&next_task, None),
        WorkEventPayload::task_stage_changed(
            next_revision,
            next_generation,
            from_stage,
            next_task.stage_id.clone(),
            noema_tasks::TaskStageChangeReason::Reopened,
        )
        .map_err(StoreError::Work)?,
    )?;
    let (run_id, event) = helpers::queue_run_tx(
        transaction,
        service.provider_registry.as_ref(),
        &next_task,
        helpers::QueueRun {
            run_kind: RunKind::Executor,
            planner_complexity: None,
            review_round: 1,
            attempt_index: 0,
            parent_run_id: None,
            event: event_context,
        },
    )?;
    Ok(helpers::task_write(event, task.task_id).run(Some(run_id)))
}

fn reopen_direction(command: &ReopenTask) -> String {
    let mut content = command.direction.feedback_markdown.trim().to_string();
    if let Some(request) = command.direction.request_markdown.as_deref() {
        content.push_str("\n\nUpdated request:\n\n");
        content.push_str(request.trim());
    }
    content
}

fn executor_for_reopen(
    transaction: &Transaction<'_>,
    current_agent_id: &str,
) -> Result<String, StoreError> {
    if current_agent_id == noema_tasks::TASK_EXECUTOR_AGENT_ID {
        return Ok(current_agent_id.to_string());
    }
    let configured = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM acp_agents WHERE agent_id = ?1)",
        [current_agent_id],
        |row| row.get::<_, bool>(0),
    )?;
    Ok(if configured {
        current_agent_id.to_string()
    } else {
        noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string()
    })
}
