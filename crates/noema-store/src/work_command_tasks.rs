//! Task capture/queue/delegation command transactions.

use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{
    AcpExecutorSnapshot, CaptureTask, ContractOrigin, DelegateTask, NewTaskValidationCriterion,
    PERSONAL_INBOX_STAGE_ID, PERSONAL_QUEUE_STAGE_ID, QueueTask, RecurrenceLifecycle,
    RunScheduledTaskNow, RunTaskRecurrenceNow, ScheduleTask, TaskComplexity, TaskContractId,
    TaskExecutorBackend, TaskExecutorSelection, TaskSourceKind, UnscheduleTask, UpdateInboxTask,
    UpdateTaskRecurrence, WorkCommand, WorkDomainError, WorkEventPayload, WorkflowStageBehavior,
    WorkflowStageId,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use std::path::Path;

use super::{WorkCommandService, helpers};
use crate::{
    StoreError,
    authorization_context::{
        bounded_authorization_context_json, conversation_authorization_context,
    },
    ids::allocate_id,
    tasks::provider_selection::{pool_selection_tx, reviewer_preference_tx},
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    match command {
        WorkCommand::CaptureTask(value) => capture(service, value).await,
        WorkCommand::UpdateInboxTask(value) => update_inbox(service, value).await,
        WorkCommand::QueueTask(value) => queue(service, value).await,
        WorkCommand::ScheduleTask(value) => set_schedule(service, value).await,
        WorkCommand::UnscheduleTask(value) => unschedule(service, value).await,
        WorkCommand::RunScheduledTaskNow(value) => run_scheduled_now(service, value).await,
        WorkCommand::UpdateTaskRecurrence(value) => update_recurrence(service, value).await,
        WorkCommand::ChangeTaskRecurrence(value) => match value.action {
            noema_tasks::RecurrenceCommandKind::Pause => {
                recurrence_lifecycle(
                    service,
                    &value.meta,
                    &value.precondition,
                    RecurrenceLifecycle::Paused,
                    command.clone(),
                )
                .await
            }
            noema_tasks::RecurrenceCommandKind::Resume => {
                recurrence_lifecycle(
                    service,
                    &value.meta,
                    &value.precondition,
                    RecurrenceLifecycle::Active,
                    command.clone(),
                )
                .await
            }
            noema_tasks::RecurrenceCommandKind::SkipNext => {
                skip_recurrence_next(service, &value.meta, &value.precondition, command.clone())
                    .await
            }
            noema_tasks::RecurrenceCommandKind::End => {
                recurrence_lifecycle(
                    service,
                    &value.meta,
                    &value.precondition,
                    RecurrenceLifecycle::Ended,
                    command.clone(),
                )
                .await
            }
        },
        WorkCommand::RunTaskRecurrenceNow(value) => run_recurrence_now(service, value).await,
        WorkCommand::DelegateTask(value) => delegate(service, value).await,
        _ => Err(StoreError::InvariantViolation {
            message: "task writer received a project command".to_string(),
        }),
    }
}

async fn capture(
    service: &WorkCommandService,
    command: &CaptureTask,
) -> Result<helpers::CommandWrite, StoreError> {
    if command.provenance.source_kind == TaskSourceKind::ChatDelegate {
        return Err(invalid_provenance(
            "capture cannot use delegated provenance",
        ));
    }
    if command.provenance.source_kind == TaskSourceKind::WorkUi
        && (!command.meta.actor_id.starts_with("actor:human:")
            || command.provenance.created_by_actor_id != command.meta.actor_id)
    {
        return Err(invalid_provenance(
            "work UI authority requires the matching authenticated human actor",
        ));
    }
    let task_id = noema_tasks::TaskId::new(allocate_id("task")).map_err(StoreError::Work)?;
    let workspace_id = command.workspace_id.clone();
    let envelope = WorkCommand::CaptureTask(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        if let Some(replay) = helpers::lookup_capture_source_receipt_tx(transaction, command)? {
            return Ok(helpers::CommandTransactionOutcome::Replay(replay));
        }
        require_workspace(transaction, &workspace_id)?;
        validate_project_target(transaction, &workspace_id, command.project_id.as_ref())?;
        let executor_agent_id = command
            .executor_agent_id
            .as_deref()
            .unwrap_or(noema_tasks::TASK_EXECUTOR_AGENT_ID);
        validate_executor_agent(transaction, executor_agent_id)?;
        let authorization_context = task_authorization_context(
            transaction,
            &command.provenance,
            &command.title,
            &command.description_markdown,
        )?;
        transaction.execute(
            r#"INSERT INTO tasks (
                     task_id, workspace_id, project_id, workflow_id, stage_id,
                     title, description_markdown, executor_agent_id, cwd_override,
                     authorization_context_json, source_kind,
                     source_conversation_id, source_turn_id, source_item_id,
                     source_tool_call_id, created_by_actor_id, scheduled_for, schedule_time_zone,
                     missed_run_policy
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                             ?15, ?16, ?17, ?18, ?19)"#,
            params![
                task_id.as_str(),
                workspace_id.as_str(),
                command.project_id.as_ref().map(ProjectId::as_str),
                noema_tasks::PERSONAL_WORKFLOW_ID,
                PERSONAL_INBOX_STAGE_ID,
                command.title,
                command.description_markdown,
                executor_agent_id,
                command.cwd_override,
                authorization_context,
                command.provenance.source_kind.as_str(),
                command.provenance.conversation_id,
                command.provenance.turn_id,
                command.provenance.item_id,
                command.provenance.source_tool_call_id,
                command.provenance.created_by_actor_id,
                command.schedule.as_ref().map(|value| value.scheduled_for),
                command
                    .schedule
                    .as_ref()
                    .map(|value| value.time_zone.as_str()),
                command
                    .schedule
                    .as_ref()
                    .map(|value| value.missed_run_policy.as_str()),
            ],
        )?;
        if let Some(schedule) = command.schedule.as_ref() {
            create_task_recurrence_tx(transaction, &task_id, schedule)?;
        }
        let payload = WorkEventPayload::task_captured(
            1,
            1,
            WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?,
            command.provenance.source_kind,
        )
        .map_err(StoreError::Work)?;
        let mut event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).scope(
                &workspace_id,
                command.project_id.as_ref(),
                Some(&task_id),
                None,
            ),
            payload,
        )?;
        if let Some(notification_event) = enqueue_work_notification_tx(
            transaction,
            &event,
            noema_tasks::NotificationKind::TaskCreated,
            &serde_json::json!({"task_id": task_id.as_str(), "title": command.title}),
        )? {
            event = notification_event;
        }
        Ok(helpers::task_write(event, task_id.clone()).into())
    })
    .await
}

async fn update_inbox(
    service: &WorkCommandService,
    command: &UpdateInboxTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::UpdateInboxTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
            let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
            if task.stage_behavior != WorkflowStageBehavior::Intake {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let project_id = match &command.project_id {
                None => task.project_id.as_ref(),
                Some(None) => None,
                Some(Some(project_id)) => Some(project_id),
            };
            validate_project_target(transaction, &task.workspace_id, project_id)?;
            let executor_agent_id = command
                .executor_agent_id
                .as_deref()
                .unwrap_or(&task.executor_agent_id);
            validate_executor_agent(transaction, executor_agent_id)?;
            let cwd_override = match &command.cwd_override {
                None => task.cwd_override.as_deref(),
                Some(cwd) => cwd.as_deref(),
            };
            let title = command.title.as_deref().unwrap_or(&task.title);
            let description = command
                .description_markdown
                .as_deref()
                .unwrap_or(&task.description_markdown);
            let authorization_context = (command.meta.actor_id.starts_with("actor:human:")
                && (command.title.is_some() || command.description_markdown.is_some()))
            .then(|| {
                bounded_authorization_context_json(
                    &noema_tasks::TaskAuthorizationContext::ManualTaskBody {
                        title: title.to_string(),
                        description_markdown: description.to_string(),
                    },
                )
            })
            .transpose()?;
            let revision = helpers::increment(task.revision, "task.revision")?;
            transaction.execute(
                "UPDATE tasks SET title = ?2, description_markdown = ?3, project_id = ?4, authorization_context_json = COALESCE(?5, authorization_context_json), executor_agent_id = ?6, cwd_override = ?7, revision = ?8, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?9 AND generation = ?10",
                params![
                    task_id.as_str(),
                    title,
                    description,
                    project_id.map(ProjectId::as_str),
                    authorization_context,
                    executor_agent_id,
                    cwd_override,
                    revision,
                    task.revision,
                    task.generation,
                ],
            )?;
            let mut changed = Vec::new();
            if command.title.is_some() {
                changed.push(noema_tasks::TaskChangedField::Title);
            }
            if command.description_markdown.is_some() {
                changed.push(noema_tasks::TaskChangedField::Description);
            }
            if command.project_id.is_some() {
                changed.push(noema_tasks::TaskChangedField::Project);
            }
            if command.executor_agent_id.is_some() {
                changed.push(noema_tasks::TaskChangedField::Executor);
            }
            if command.cwd_override.is_some() {
                changed.push(noema_tasks::TaskChangedField::WorkingDirectory);
            }
            let payload = WorkEventPayload::task_updated(revision, task.generation, changed)
                .map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &task.workspace_id,
                    project_id,
                    Some(&task_id),
                    None,
                ),
                payload,
            )?;
            Ok(helpers::task_write(event, task_id.clone()).into())
        })
        .await
}

async fn set_schedule(
    service: &WorkCommandService,
    command: &ScheduleTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::ScheduleTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        if task.stage_behavior != WorkflowStageBehavior::Intake {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let existing = task_schedule_recurrence_tx(transaction, &task_id)?;
        if command.requires_existing != existing.0.is_some() {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        if let Some(recurrence_id) = existing.1.as_deref() {
            transaction.execute(
                "DELETE FROM task_recurrence_occurrences WHERE recurrence_id = ?1",
                [recurrence_id],
            )?;
            transaction.execute(
                "DELETE FROM task_recurrences WHERE recurrence_id = ?1",
                [recurrence_id],
            )?;
        }
        let schedule = &command.schedule;
        let revision = helpers::increment(task.revision, "task.revision")?;
        transaction.execute(
            "UPDATE tasks SET scheduled_for = ?2, schedule_time_zone = ?3, missed_run_policy = ?4, recurrence_id = NULL, recurrence_revision = NULL, recurrence_scheduled_for = NULL, revision = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?6 AND generation = ?7",
            params![task_id.as_str(), schedule.scheduled_for, schedule.time_zone,
                schedule.missed_run_policy.as_str(), revision, task.revision, task.generation],
        )?;
        create_task_recurrence_tx(transaction, &task_id, schedule)?;
        schedule_task_write(transaction, &command.meta, &task, revision)
    }).await
}

fn create_task_recurrence_tx(
    transaction: &Transaction<'_>,
    task_id: &noema_tasks::TaskId,
    schedule: &noema_tasks::NewTaskSchedule,
) -> Result<(), StoreError> {
    let Some(recurrence) = schedule.recurrence.as_ref() else {
        return Ok(());
    };
    let task = helpers::load_task_state_tx(transaction, task_id)?;
    let authorization: String = transaction.query_row(
        "SELECT authorization_context_json FROM tasks WHERE task_id = ?1",
        [task_id.as_str()],
        |row| row.get(0),
    )?;
    let recurrence_id =
        noema_tasks::TaskRecurrenceId::new(allocate_id("recurrence")).map_err(StoreError::Work)?;
    let next = noema_tasks::next_recurrence_at_or_after(
        &recurrence.cron_expression,
        &schedule.time_zone,
        schedule.scheduled_for.saturating_add(1),
    )
    .map_err(StoreError::Work)?;
    transaction.execute(
        "INSERT INTO task_recurrences (recurrence_id, workspace_id, project_id, title, description_markdown, authorization_context_json, executor_agent_id, cwd_override, starts_at, cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle, next_run_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'active', ?14)",
        params![recurrence_id.as_str(), task.workspace_id.as_str(), task.project_id.as_ref().map(ProjectId::as_str),
            task.title, task.description_markdown, authorization, task.executor_agent_id, task.cwd_override,
            recurrence.starts_at, recurrence.cron_expression, schedule.time_zone,
            schedule.missed_run_policy.as_str(), recurrence.overlap_policy.as_str(), next],
    )?;
    transaction.execute(
        "UPDATE tasks SET recurrence_id = ?2, recurrence_revision = 1, recurrence_scheduled_for = scheduled_for WHERE task_id = ?1",
        params![task_id.as_str(), recurrence_id.as_str()],
    )?;
    transaction.execute(
        "INSERT INTO task_recurrence_occurrences (occurrence_id, recurrence_id, recurrence_revision, scheduled_for, local_slot, resolution, task_id) VALUES (?1, ?2, 1, ?3, ?4, 'materialized', ?5)",
        params![allocate_id("occurrence"), recurrence_id.as_str(), schedule.scheduled_for,
            noema_tasks::recurrence_local_slot(schedule.scheduled_for, &schedule.time_zone).map_err(StoreError::Work)?, task_id.as_str()],
    )?;
    Ok(())
}

async fn unschedule(
    service: &WorkCommandService,
    command: &UnscheduleTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::UnscheduleTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        if task.stage_behavior != WorkflowStageBehavior::Intake {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let (scheduled_for, recurrence_id) = task_schedule_recurrence_tx(transaction, &task_id)?;
        if scheduled_for.is_none() {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        if let Some(recurrence_id) = recurrence_id {
            transaction.execute("DELETE FROM task_recurrence_occurrences WHERE recurrence_id = ?1", [&recurrence_id])?;
            transaction.execute("DELETE FROM task_recurrences WHERE recurrence_id = ?1", [&recurrence_id])?;
        }
        let revision = helpers::increment(task.revision, "task.revision")?;
        transaction.execute(
            "UPDATE tasks SET scheduled_for = NULL, schedule_time_zone = NULL, missed_run_policy = NULL, recurrence_id = NULL, recurrence_revision = NULL, recurrence_scheduled_for = NULL, revision = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?3 AND generation = ?4",
            params![task_id.as_str(), revision, task.revision, task.generation],
        )?;
        schedule_task_write(transaction, &command.meta, &task, revision)
    }).await
}

fn task_schedule_recurrence_tx(
    transaction: &Transaction<'_>,
    task_id: &noema_tasks::TaskId,
) -> Result<(Option<i64>, Option<String>), StoreError> {
    Ok(transaction.query_row(
        "SELECT scheduled_for, recurrence_id FROM tasks WHERE task_id = ?1",
        [task_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

fn schedule_task_write(
    transaction: &Transaction<'_>,
    meta: &noema_tasks::CommandMeta,
    task: &helpers::TaskState,
    revision: u64,
) -> Result<helpers::CommandTransactionOutcome, StoreError> {
    let payload = WorkEventPayload::task_updated(
        revision,
        task.generation,
        vec![noema_tasks::TaskChangedField::Schedule],
    )
    .map_err(StoreError::Work)?;
    let event = append_work_event_tx(
        transaction,
        helpers::event_context(meta).scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            None,
        ),
        payload,
    )?;
    Ok(helpers::task_write(event, task.task_id.clone()).into())
}

struct RecurrenceState {
    workspace_id: WorkspaceId,
    project_id: Option<ProjectId>,
    title: String,
    description: String,
    authorization_context: String,
    starts_at: i64,
    cron: String,
    time_zone: String,
    missed: noema_tasks::MissedRunPolicy,
    overlap: noema_tasks::OverlapPolicy,
    lifecycle: RecurrenceLifecycle,
    revision: u64,
    next_run_at: Option<i64>,
}

fn load_recurrence_tx(
    transaction: &Transaction<'_>,
    precondition: &noema_tasks::RecurrencePrecondition,
) -> Result<RecurrenceState, StoreError> {
    let row = transaction.query_row(
        "SELECT workspace_id, project_id, title, description_markdown, authorization_context_json, starts_at, cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle, revision, next_run_at FROM task_recurrences WHERE recurrence_id = ?1",
        [precondition.recurrence_id.as_str()],
        |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?,
            row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
            row.get::<_, String>(9)?, row.get::<_, String>(10)?, row.get::<_, i64>(11)?,
            row.get::<_, Option<i64>>(12)?,
        )),
    ).optional()?.ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    let revision = u64::try_from(row.11).map_err(|_| StoreError::InvariantViolation {
        message: "recurrence revision overflow".to_string(),
    })?;
    if revision != precondition.expected_revision {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    Ok(RecurrenceState {
        workspace_id: WorkspaceId::new(row.0).map_err(StoreError::Workspace)?,
        project_id: row
            .1
            .map(ProjectId::new)
            .transpose()
            .map_err(StoreError::Workspace)?,
        title: row.2,
        description: row.3,
        authorization_context: row.4,
        starts_at: row.5,
        cron: row.6,
        time_zone: row.7,
        missed: row.8.parse().map_err(StoreError::Work)?,
        overlap: row.9.parse().map_err(StoreError::Work)?,
        lifecycle: row.10.parse().map_err(StoreError::Work)?,
        revision,
        next_run_at: row.12,
    })
}

async fn update_recurrence(
    service: &WorkCommandService,
    command: &UpdateTaskRecurrence,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::UpdateTaskRecurrence(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let state = load_recurrence_tx(transaction, &command.precondition)?;
        if state.lifecycle == RecurrenceLifecycle::Ended {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let project_id = match &command.project_id {
            None => state.project_id.as_ref(),
            Some(None) => None,
            Some(Some(value)) => Some(value),
        };
        validate_project_target(transaction, &state.workspace_id, project_id)?;
        let title = command.title.as_deref().unwrap_or(&state.title);
        let description = command.description_markdown.as_deref().unwrap_or(&state.description);
        let starts_at = command.starts_at.unwrap_or(state.starts_at);
        let cron = command.cron_expression.as_deref().unwrap_or(&state.cron);
        let time_zone = command.time_zone.as_deref().unwrap_or(&state.time_zone);
        let missed = command.missed_run_policy.unwrap_or(state.missed);
        let overlap = command.overlap_policy.unwrap_or(state.overlap);
        let timing_changed = command.starts_at.is_some() || command.cron_expression.is_some() || command.time_zone.is_some();
        let next_run_at = if timing_changed {
            Some(noema_tasks::next_recurrence_at_or_after(cron, time_zone, starts_at.max(unix_now()))
                .map_err(StoreError::Work)?)
        } else { state.next_run_at };
        let authorization_context = if command.meta.actor_id.starts_with("actor:human:")
            && (command.title.is_some() || command.description_markdown.is_some())
        {
            bounded_authorization_context_json(&noema_tasks::TaskAuthorizationContext::ManualTaskBody {
                title: title.to_string(), description_markdown: description.to_string(),
            })?
        } else { state.authorization_context };
        let revision = helpers::increment(state.revision, "recurrence.revision")?;
        transaction.execute(
            "UPDATE task_recurrences SET project_id = ?2, title = ?3, description_markdown = ?4, authorization_context_json = ?5, starts_at = ?6, cron_expression = ?7, time_zone = ?8, missed_run_policy = ?9, overlap_policy = ?10, next_run_at = ?11, revision = ?12, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE recurrence_id = ?1 AND revision = ?13",
            params![command.precondition.recurrence_id.as_str(), project_id.map(ProjectId::as_str), title,
                description, authorization_context, starts_at, cron, time_zone, missed.as_str(),
                overlap.as_str(), next_run_at, revision, state.revision],
        )?;
        recurrence_command_write(transaction, &command.meta, &command.precondition.recurrence_id, &state.workspace_id, project_id, revision, "updated")
    }).await
}

async fn recurrence_lifecycle(
    service: &WorkCommandService,
    meta: &noema_tasks::CommandMeta,
    precondition: &noema_tasks::RecurrencePrecondition,
    lifecycle: RecurrenceLifecycle,
    envelope: WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let state = load_recurrence_tx(transaction, precondition)?;
        let valid = matches!((state.lifecycle, lifecycle),
            (RecurrenceLifecycle::Active, RecurrenceLifecycle::Paused | RecurrenceLifecycle::Ended)
            | (RecurrenceLifecycle::Paused, RecurrenceLifecycle::Active | RecurrenceLifecycle::Ended));
        if !valid { return Err(StoreError::Work(WorkDomainError::InvalidTransition)); }
        let next_run_at = match lifecycle {
            RecurrenceLifecycle::Active => state.next_run_at,
            RecurrenceLifecycle::Paused => state.next_run_at,
            RecurrenceLifecycle::Ended => None,
        };
        if lifecycle == RecurrenceLifecycle::Active
            && state.missed == noema_tasks::MissedRunPolicy::Skip
            && next_run_at.is_some_and(|due| due < unix_now())
        {
            let skipped = next_run_at.expect("checked missed slot");
            record_skipped_recurrence_tx(transaction, precondition, &state, skipped)?;
        }
        let next_run_at = if lifecycle == RecurrenceLifecycle::Active
            && state.missed == noema_tasks::MissedRunPolicy::Skip
            && next_run_at.is_some_and(|due| due < unix_now())
        {
            Some(noema_tasks::next_recurrence_at_or_after(&state.cron, &state.time_zone, unix_now().saturating_add(1))
                .map_err(StoreError::Work)?)
        } else { next_run_at };
        let revision = helpers::increment(state.revision, "recurrence.revision")?;
        transaction.execute(
            "UPDATE task_recurrences SET lifecycle = ?2, next_run_at = ?3, revision = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE recurrence_id = ?1 AND revision = ?5",
            params![precondition.recurrence_id.as_str(), lifecycle.as_str(), next_run_at, revision, state.revision],
        )?;
        recurrence_command_write(transaction, meta, &precondition.recurrence_id, &state.workspace_id, state.project_id.as_ref(), revision, lifecycle.as_str())
    }).await
}

async fn skip_recurrence_next(
    service: &WorkCommandService,
    meta: &noema_tasks::CommandMeta,
    precondition: &noema_tasks::RecurrencePrecondition,
    envelope: WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let state = load_recurrence_tx(transaction, precondition)?;
        if state.lifecycle != RecurrenceLifecycle::Active {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let skipped = state.next_run_at.ok_or(StoreError::Work(WorkDomainError::InvalidTransition))?;
        record_skipped_recurrence_tx(transaction, precondition, &state, skipped)?;
        let next = noema_tasks::next_recurrence_at_or_after(&state.cron, &state.time_zone, skipped.saturating_add(1))
            .map_err(StoreError::Work)?;
        let revision = helpers::increment(state.revision, "recurrence.revision")?;
        transaction.execute(
            "UPDATE task_recurrences SET next_run_at = ?2, revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE recurrence_id = ?1 AND revision = ?4",
            params![precondition.recurrence_id.as_str(), next, revision, state.revision],
        )?;
        recurrence_command_write(transaction, meta, &precondition.recurrence_id, &state.workspace_id, state.project_id.as_ref(), revision, "skipped_next")
    }).await
}

fn record_skipped_recurrence_tx(
    transaction: &Transaction<'_>,
    precondition: &noema_tasks::RecurrencePrecondition,
    state: &RecurrenceState,
    skipped: i64,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT OR IGNORE INTO task_recurrence_occurrences (occurrence_id, recurrence_id, recurrence_revision, scheduled_for, local_slot, resolution) VALUES (?1, ?2, ?3, ?4, ?5, 'skipped')",
        params![allocate_id("occurrence"), precondition.recurrence_id.as_str(), state.revision,
            skipped, noema_tasks::recurrence_local_slot(skipped, &state.time_zone).map_err(StoreError::Work)?],
    )?;
    Ok(())
}

fn recurrence_command_write(
    transaction: &Transaction<'_>,
    meta: &noema_tasks::CommandMeta,
    recurrence_id: &noema_tasks::TaskRecurrenceId,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
    revision: u64,
    reason: &str,
) -> Result<helpers::CommandTransactionOutcome, StoreError> {
    let task_id = transaction.query_row(
        "SELECT task_id FROM tasks WHERE recurrence_id = ?1 ORDER BY recurrence_scheduled_for DESC LIMIT 1",
        [recurrence_id.as_str()],
        |row| row.get::<_, String>(0),
    )?;
    let task_id = noema_tasks::TaskId::new(task_id).map_err(StoreError::Work)?;
    let payload = WorkEventPayload::recurrence_changed(
        recurrence_id.to_string(),
        revision,
        reason.to_string(),
    )
    .map_err(StoreError::Work)?;
    let event = append_work_event_tx(
        transaction,
        helpers::event_context(meta).scope(workspace_id, project_id, Some(&task_id), None),
        payload,
    )?;
    Ok(helpers::task_write(event, task_id).into())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

async fn queue(
    service: &WorkCommandService,
    command: &QueueTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::QueueTask(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        queue_task_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            &command.meta,
            false,
        )
    })
    .await
}

pub(crate) fn queue_task_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    mut task: helpers::TaskState,
    meta: &noema_tasks::CommandMeta,
    allow_scheduled: bool,
) -> Result<helpers::CommandTransactionOutcome, StoreError> {
    if task.stage_behavior != WorkflowStageBehavior::Intake {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let is_scheduled = transaction.query_row(
        "SELECT scheduled_for IS NOT NULL FROM tasks WHERE task_id = ?1",
        [task.task_id.as_str()],
        |row| row.get::<_, bool>(0),
    )?;
    if is_scheduled && !allow_scheduled {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    validate_project_target(transaction, &task.workspace_id, task.project_id.as_ref())?;
    let revision = helpers::increment(task.revision, "task.revision")?;
    transaction.execute(
        "UPDATE tasks SET stage_id = ?2, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?4 AND generation = ?5",
        params![task.task_id.as_str(), PERSONAL_QUEUE_STAGE_ID, revision, task.revision, task.generation],
    )?;
    task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::Dispatch;
    task.revision = revision;
    let queued = WorkEventPayload::task_queued(
        revision,
        task.generation,
        None,
        noema_tasks::RunKind::Planner,
    )
    .map_err(StoreError::Work)?;
    append_work_event_tx(
        transaction,
        helpers::event_context(meta).task_scope(&task, None),
        queued,
    )?;
    let changed = WorkEventPayload::task_stage_changed(
        revision,
        task.generation,
        WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?,
        task.stage_id.clone(),
        noema_tasks::TaskStageChangeReason::Queued,
    )
    .map_err(StoreError::Work)?;
    append_work_event_tx(
        transaction,
        helpers::event_context(meta).task_scope(&task, None),
        changed,
    )?;
    let (run_id, run_event) = helpers::queue_run_tx(
        transaction,
        registry,
        &task,
        helpers::QueueRun {
            run_kind: noema_tasks::RunKind::Planner,
            contract_id: None,
            planner_complexity: None,
            review_round: 0,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
            event: helpers::event_context(meta),
        },
    )?;
    Ok(helpers::task_write(run_event, task.task_id.clone())
        .run(Some(run_id))
        .into())
}

async fn run_scheduled_now(
    service: &WorkCommandService,
    command: &RunScheduledTaskNow,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::RunScheduledTaskNow(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        let scheduled = transaction.query_row(
            "SELECT scheduled_for IS NOT NULL FROM tasks WHERE task_id = ?1",
            [task.task_id.as_str()],
            |row| row.get::<_, bool>(0),
        )?;
        if !scheduled {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        transaction.execute(
            "UPDATE task_recurrence_occurrences SET trigger_kind = 'manual' WHERE task_id = ?1",
            [task.task_id.as_str()],
        )?;
        queue_task_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            &command.meta,
            true,
        )
    })
    .await
}

async fn run_recurrence_now(
    service: &WorkCommandService,
    command: &RunTaskRecurrenceNow,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::RunTaskRecurrenceNow(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let recurrence = load_recurrence_tx(transaction, &command.precondition)?;
        if recurrence.lifecycle == RecurrenceLifecycle::Ended
            || super::schedules::recurrence_has_nonterminal_task_tx(
                transaction,
                command.precondition.recurrence_id.as_str(),
            )?
        {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        super::schedules::materialize_manual_occurrence_tx(
            service,
            transaction,
            command.precondition.recurrence_id.as_str(),
            unix_now(),
            &command.meta,
        )
    })
    .await
}

async fn delegate(
    service: &WorkCommandService,
    command: &DelegateTask,
) -> Result<helpers::CommandWrite, StoreError> {
    if command.provenance.source_kind != TaskSourceKind::ChatDelegate {
        return Err(invalid_provenance(
            "delegate requires chat-delegate provenance",
        ));
    }
    if command.meta.idempotency_key.is_none() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "task.delegate.meta.idempotency_key",
            message: "Delegate requires an idempotency key".to_string(),
        }));
    }
    let task_id = noema_tasks::TaskId::new(allocate_id("task")).map_err(StoreError::Work)?;
    let workspace_id = command.workspace_id.clone();
    let envelope = WorkCommand::DelegateTask(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        if let Some(replay) = helpers::lookup_delegate_source_receipt_tx(transaction, command)? {
            return Ok(helpers::CommandTransactionOutcome::Replay(replay));
        }
        require_workspace(transaction, &workspace_id)?;
        validate_project_target(transaction, &workspace_id, command.project_id.as_ref())?;
        let executor_agent_id = command
            .executor_agent_id
            .as_deref()
            .unwrap_or(noema_tasks::TASK_EXECUTOR_AGENT_ID);
        validate_executor_agent(transaction, executor_agent_id)?;
        let authorization_context = task_authorization_context(
            transaction,
            &command.provenance,
            &command.title,
            &command.description_markdown,
        )?;
        transaction.execute(
            r#"INSERT INTO tasks (
                     task_id, workspace_id, project_id, workflow_id, stage_id,
                     title, description_markdown, executor_agent_id, cwd_override,
                     authorization_context_json, source_kind,
                     source_conversation_id, source_turn_id, source_item_id,
                     source_tool_call_id, created_by_actor_id, queued_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                             strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"#,
            params![
                task_id.as_str(),
                workspace_id.as_str(),
                command.project_id.as_ref().map(ProjectId::as_str),
                noema_tasks::PERSONAL_WORKFLOW_ID,
                PERSONAL_QUEUE_STAGE_ID,
                command.title,
                command.description_markdown,
                executor_agent_id,
                command.cwd_override,
                authorization_context,
                TaskSourceKind::ChatDelegate.as_str(),
                command.provenance.conversation_id,
                command.provenance.turn_id,
                command.provenance.item_id,
                command.provenance.source_tool_call_id,
                command.provenance.created_by_actor_id,
            ],
        )?;
        let task = helpers::load_task_state_tx(transaction, &task_id)?;
        let captured = WorkEventPayload::task_captured(
            1,
            1,
            WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?,
            TaskSourceKind::ChatDelegate,
        )
        .map_err(StoreError::Work)?;
        let captured_event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).task_scope(&task, None),
            captured,
        )?;
        let _notification_event = enqueue_work_notification_tx(
            transaction,
            &captured_event,
            noema_tasks::NotificationKind::TaskCreated,
            &serde_json::json!({"task_id": task_id.as_str(), "title": command.title}),
        )?;
        let contract = if let Some(intent) = &command.execution_intent {
            let (contract_id, _contract_event) = create_contract_tx(
                transaction,
                service.provider_registry.as_ref(),
                &service.store.default_task_cwd(task.task_id.as_str()),
                &task,
                CreateContract {
                    origin: ContractOrigin::Delegated,
                    request_markdown: &intent.request_markdown,
                    execution_plan_markdown: intent.execution_plan_markdown.as_deref(),
                    criteria: &intent.criteria,
                    complexity: intent.complexity,
                    supersedes_contract_id: None,
                    event: helpers::event_context(&command.meta),
                },
            )?;
            Some(contract_id)
        } else {
            None
        };
        let next_kind = if contract.is_some() {
            noema_tasks::RunKind::Executor
        } else {
            noema_tasks::RunKind::Planner
        };
        let queued = WorkEventPayload::task_queued(1, 1, contract.clone(), next_kind)
            .map_err(StoreError::Work)?;
        let _queued_event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).task_scope(&task, None),
            queued,
        )?;
        let (run_id, run_event) = helpers::queue_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            &task,
            helpers::QueueRun {
                run_kind: next_kind,
                contract_id: contract.as_ref(),
                planner_complexity: command.complexity_hint,
                review_round: u32::from(contract.is_some()),
                attempt_index: 0,
                parent_run_id: None,
                triggering_submission_id: None,
                triggering_review_id: None,
                event: helpers::event_context(&command.meta),
            },
        )?;
        Ok(helpers::task_write(run_event, task_id.clone())
            .contract(contract)
            .run(Some(run_id))
            .into())
    })
    .await
}

fn task_authorization_context(
    transaction: &Transaction<'_>,
    provenance: &noema_tasks::TaskProvenance,
    title: &str,
    description_markdown: &str,
) -> Result<String, StoreError> {
    let context = match provenance.source_kind {
        TaskSourceKind::ChatCapture | TaskSourceKind::ChatDelegate => {
            let (Some(conversation_id), Some(turn_id), Some(item_id)) = (
                provenance.conversation_id.as_deref(),
                provenance.turn_id.as_deref(),
                provenance.item_id.as_deref(),
            ) else {
                return Err(invalid_provenance(
                    "chat-created tasks require conversation, turn, and human item provenance",
                ));
            };
            conversation_authorization_context(transaction, conversation_id, turn_id, item_id)?
        }
        TaskSourceKind::WorkUi => noema_tasks::TaskAuthorizationContext::ManualTaskBody {
            title: title.to_string(),
            description_markdown: description_markdown.to_string(),
        },
        TaskSourceKind::System => noema_tasks::TaskAuthorizationContext::None,
    };
    bounded_authorization_context_json(&context)
}

fn invalid_provenance(message: &str) -> StoreError {
    StoreError::Work(WorkDomainError::InvalidInput {
        field: "task.provenance",
        message: message.to_string(),
    })
}

fn require_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
) -> Result<(), StoreError> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND is_personal = 1 AND archived_at IS NULL)",
        [workspace_id.as_str()],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::WorkUnavailable))
    }
}

fn validate_project_target(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
) -> Result<(), StoreError> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let row = transaction
        .query_row(
            "SELECT workspace_id, archived_at FROM projects WHERE project_id = ?1",
            [project_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let Some((workspace, archived_at)) = row else {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    };
    if workspace != workspace_id.as_str() {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    }
    if archived_at.is_some() {
        return Err(StoreError::Work(WorkDomainError::ProjectArchived));
    }
    Ok(())
}

fn validate_executor_agent(
    transaction: &Transaction<'_>,
    agent_id: &str,
) -> Result<(), StoreError> {
    if agent_id == noema_tasks::TASK_EXECUTOR_AGENT_ID {
        return Ok(());
    }
    let enabled = transaction
        .query_row(
            "SELECT enabled FROM acp_agents WHERE agent_id = ?1 LIMIT 1",
            [agent_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?;
    match enabled {
        Some(true) => Ok(()),
        Some(false) | None => Err(StoreError::Work(WorkDomainError::ConfigurationUnavailable)),
    }
}

/// Create one immutable execution contract and its exact criteria rows.
pub(crate) struct CreateContract<'a> {
    pub origin: ContractOrigin,
    pub request_markdown: &'a str,
    pub execution_plan_markdown: Option<&'a str>,
    pub criteria: &'a [NewTaskValidationCriterion],
    pub complexity: TaskComplexity,
    pub supersedes_contract_id: Option<&'a TaskContractId>,
    pub event: helpers::CommandEventContext<'a>,
}

pub(crate) fn create_contract_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    default_task_cwd: &Path,
    task: &helpers::TaskState,
    request: CreateContract<'_>,
) -> Result<(TaskContractId, noema_tasks::WorkEventRecord), StoreError> {
    let executor = select_pool(transaction, request.complexity)?;
    let reviewer = reviewer_preference_tx(transaction)?;
    let _executor_ready = crate::provider_selections::prove_selection_ready(&executor, registry)?;
    let _reviewer_ready = crate::provider_selections::prove_selection_ready(&reviewer, registry)?;
    let policy = helpers::load_policy_tx(transaction)?;
    let contract_id = TaskContractId::new(allocate_id("contract")).map_err(StoreError::Work)?;
    let version: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM task_execution_contracts WHERE task_id = ?1",
        [task.task_id.as_str()],
        |row| row.get(0),
    )?;
    if request.origin == ContractOrigin::Planned && request.execution_plan_markdown.is_none() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "contract.execution_plan_markdown",
            message: "planned contract requires a plan".to_string(),
        }));
    }
    let (workspace_name, workspace_description) = transaction.query_row(
        "SELECT name, description FROM workspaces WHERE workspace_id = ?1",
        [task.workspace_id.as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    let project_context = task
        .project_id
        .as_ref()
        .map(|project_id| {
            transaction.query_row(
                "SELECT name, description, folder FROM projects WHERE project_id = ?1",
                [project_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
        })
        .transpose()?;
    let executor_selection = if task.executor_agent_id == noema_tasks::TASK_EXECUTOR_AGENT_ID {
        TaskExecutorSelection::provider()
    } else {
        let snapshot = transaction
            .query_row(
                "SELECT command, arguments_json, connection_revision, enabled FROM acp_agents WHERE agent_id = ?1 LIMIT 1",
                [task.executor_agent_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
        if !snapshot.3 {
            return Err(StoreError::Work(WorkDomainError::ConfigurationUnavailable));
        }
        TaskExecutorSelection {
            agent_id: task.executor_agent_id.clone(),
            backend: TaskExecutorBackend::Acp,
            acp: Some(AcpExecutorSnapshot {
                connection_revision: u64::try_from(snapshot.2).map_err(|_| {
                    StoreError::InvariantViolation {
                        message: "ACP connection revision overflow".to_string(),
                    }
                })?,
                command: snapshot.0,
                arguments: serde_json::from_str(&snapshot.1)?,
            }),
        }
    };
    let effective_cwd = task
        .cwd_override
        .clone()
        .or_else(|| project_context.as_ref().and_then(|value| value.2.clone()))
        .unwrap_or_else(|| default_task_cwd.to_string_lossy().into_owned());
    if task.cwd_override.is_none()
        && project_context
            .as_ref()
            .and_then(|value| value.2.as_ref())
            .is_none()
    {
        std::fs::create_dir_all(default_task_cwd).map_err(StoreError::PreparePath)?;
    }
    let acp_connection_revision = executor_selection
        .acp
        .as_ref()
        .map(|snapshot| snapshot.connection_revision);
    let acp_launch_json = executor_selection
        .acp
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    transaction.execute(
        r#"INSERT INTO task_execution_contracts (
             contract_id, task_id, version, task_generation, supersedes_contract_id, origin,
             request_markdown, execution_plan_markdown, complexity,
             executor_provider_kind, executor_provider_account_id,
             executor_provider_instance_key, executor_selection_mode,
             executor_model_profile, executor_reasoning_effort, executor_fast_mode,
             executor_selection_source, reviewer_provider_kind,
             reviewer_provider_account_id, reviewer_provider_instance_key,
             reviewer_selection_mode, reviewer_model_profile,
             reviewer_reasoning_effort, reviewer_fast_mode, reviewer_selection_source,
             max_provider_continuations, max_tool_calls, max_active_minutes,
             progress_audit_interval, max_automatic_retries, max_review_rounds,
             workspace_id_snapshot, workspace_name_snapshot,
             workspace_description_snapshot, project_id_snapshot,
             project_name_snapshot, project_description_snapshot,
             project_folder_snapshot, executor_backend_kind, executor_agent_id,
             executor_acp_connection_revision, executor_acp_launch_json, effective_cwd,
             created_by_actor_id
           ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                     ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33,
                     ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44)"#,
        params![
            contract_id.as_str(),
            task.task_id.as_str(),
            version,
            task.generation,
            request.supersedes_contract_id.map(ToString::to_string),
            request.origin.as_str(),
            request.request_markdown,
            request.execution_plan_markdown,
            request.complexity.as_str(),
            executor.provider_kind,
            executor.provider_account_id,
            executor
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string),
            executor.selection_mode.as_str(),
            executor.model_profile,
            executor
                .reasoning_effort
                .map(ReasoningEffort::as_persistence_str),
            executor.fast_mode,
            executor.selection_source,
            reviewer.provider_kind,
            reviewer.provider_account_id,
            reviewer
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string),
            reviewer.selection_mode.as_str(),
            reviewer.model_profile,
            reviewer
                .reasoning_effort
                .map(ReasoningEffort::as_persistence_str),
            reviewer.fast_mode,
            reviewer.selection_source,
            policy.max_provider_continuations,
            policy.max_tool_calls,
            policy.max_active_minutes,
            policy.progress_audit_interval,
            policy.max_automatic_retries,
            policy.max_review_rounds,
            task.workspace_id.as_str(),
            workspace_name,
            workspace_description,
            task.project_id.as_ref().map(ProjectId::as_str),
            project_context.as_ref().map(|value| value.0.as_str()),
            project_context.as_ref().map(|value| value.1.as_str()),
            project_context
                .as_ref()
                .and_then(|value| value.2.as_deref()),
            executor_selection.backend.as_str(),
            executor_selection.agent_id,
            acp_connection_revision,
            acp_launch_json,
            effective_cwd,
            request.event.actor_id,
        ],
    )?;
    for criterion in request.criteria {
        let criterion_id = if let Some(criterion_id) = &criterion.criterion_id {
            if !criterion_id.starts_with("criterion:") {
                return Err(StoreError::Work(WorkDomainError::InvalidInput {
                    field: "contract.criteria.criterion_id",
                    message: "criterion id must use the criterion: namespace".to_string(),
                }));
            }
            criterion_id.clone()
        } else {
            allocate_id("criterion")
        };
        transaction.execute(
            "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description, expected_evidence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![criterion_id, contract_id.as_str(), criterion.ordinal, criterion.description, criterion.expected_evidence],
        )?;
    }
    transaction.execute(
        "UPDATE tasks SET current_contract_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1",
        params![task.task_id.as_str(), contract_id.as_str()],
    )?;
    let payload = WorkEventPayload::contract_created(
        contract_id.clone(),
        u32::try_from(version).map_err(|_| StoreError::InvariantViolation {
            message: "contract version overflow".to_string(),
        })?,
        task.generation,
        request.origin,
        request.complexity,
        u32::try_from(request.criteria.len()).map_err(|_| StoreError::InvariantViolation {
            message: "criteria count overflow".to_string(),
        })?,
        request.supersedes_contract_id.cloned(),
    )
    .map_err(StoreError::Work)?;
    let event = append_work_event_tx(
        transaction,
        request.event.scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            None,
        ),
        payload,
    )?;
    Ok((contract_id, event))
}

fn select_pool(
    transaction: &Transaction<'_>,
    complexity: TaskComplexity,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let complexity_str = complexity.as_str();
    let id: String = transaction.query_row(
        "SELECT pool_entry_id FROM task_model_pool_entries WHERE complexity = ?1 AND enabled = 1 ORDER BY sort_order, label, pool_entry_id LIMIT 1",
        [complexity_str],
        |row| row.get(0),
    )?;
    pool_selection_tx(transaction, complexity, &id)
}
