use async_graphql::Result;
use noema_tasks::{
    AnswerTask, ArchiveProject, CancelTask, CaptureTask, ChangeTaskRecurrence, CreateProject,
    NewTaskRecurrence, NewTaskSchedule, ProjectPrecondition, QueueTask, RecurrenceCommandKind,
    RecurrencePrecondition, ReopenProject, ReopenTask, RetryTask, RunScheduledTaskNow,
    RunTaskRecurrenceNow, ScheduleTask, TaskGateId, TaskProvenance, TaskRecurrenceId,
    TaskReopenDirection, TaskSourceKind, UnscheduleTask, UpdateInboxTask, UpdateProject,
    UpdateTaskRecurrence, WorkCommand,
};

use super::*;
use crate::graphql::schema::GraphqlState;
use crate::graphql::tasks::*;

macro_rules! simple_task_mutation {
    ($(#[$meta:meta])* $function:ident, $input:ty, $command:ident, $variant:ident) => {
        $(#[$meta])*
        pub(in crate::graphql) async fn $function(
            state: &GraphqlState,
            principal_subject: &str,
            input: $input,
        ) -> Result<GraphqlTaskCommandPayload> {
            require_owner(principal_subject)?;
            task_simple_command(
                state,
                principal_subject,
                &input.client_mutation_id,
                task_precondition(
                    &input.task_id,
                    input.expected_revision,
                    input.expected_generation,
                )?,
                |meta, precondition| WorkCommand::$variant($command { meta, precondition }),
            )
            .await
        }
    };
}

macro_rules! simple_project_mutation {
    ($(#[$meta:meta])* $function:ident, $input:ty, $command:ident, $variant:ident) => {
        $(#[$meta])*
        pub(in crate::graphql) async fn $function(
            state: &GraphqlState,
            principal_subject: &str,
            input: $input,
        ) -> Result<GraphqlProjectCommandPayload> {
            require_owner(principal_subject)?;
            let client_id = required_client_id(&input.client_mutation_id)?;
            let project_id = parse_project_id(&input.project_id)?;
            require_personal_project(state.store()?, &project_id).await?;
            let command = WorkCommand::$variant($command {
                meta: command_meta(principal_subject, &client_id),
                precondition: ProjectPrecondition {
                    project_id,
                    expected_revision: positive(input.expected_revision, "expectedRevision")?,
                },
            });
            project_payload(client_id, execute_command(state, command).await?)
        }
    };
}

macro_rules! fenced_task_mutation {
    (
        $(#[$meta:meta])*
        $function:ident($input_type:ty, $input:ident) {
            prepare { $($prepare:stmt)* }
            command |$meta_name:ident, $precondition_name:ident| $command:expr
        }
    ) => {
        $(#[$meta])*
        pub(in crate::graphql) async fn $function(
            state: &GraphqlState,
            principal_subject: &str,
            $input: $input_type,
        ) -> Result<GraphqlTaskCommandPayload> {
            require_owner(principal_subject)?;
            $($prepare)*
            task_simple_command(
                state,
                principal_subject,
                &$input.client_mutation_id,
                task_precondition(
                    &$input.task_id,
                    $input.expected_revision,
                    $input.expected_generation,
                )?,
                move |$meta_name, $precondition_name| $command,
            )
            .await
        }
    };
}

/// Create a project through the semantic command service.
pub(in crate::graphql) async fn create_project(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlCreateProjectInput,
) -> Result<GraphqlProjectCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let command = WorkCommand::CreateProject(CreateProject {
        meta: command_meta(principal_subject, &client_id),
        workspace_id,
        name: input.name,
        description: input.description,
        folder: input.folder,
        project_document_markdown: None,
    });
    let result = execute_command(state, command).await?;
    let project = result.result.project.ok_or_else(unavailable)?;
    Ok(GraphqlProjectCommandPayload {
        project: project.try_into()?,
        event_cursor: event_cursor(result.result.event_sequence)?,
        client_mutation_id: client_id,
    })
}

/// Update a project through the semantic command service.
pub(in crate::graphql) async fn update_project(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlUpdateProjectInput,
) -> Result<GraphqlProjectCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let project_id = parse_project_id(&input.project_id)?;
    require_personal_project(state.store()?, &project_id).await?;
    let folder = match (input.folder, input.clear_folder.unwrap_or(false)) {
        (Some(_), true) => return Err(invalid_input_error("folder and clearFolder")),
        (Some(value), false) => Some(Some(value)),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let command = WorkCommand::UpdateProject(UpdateProject {
        meta: command_meta(principal_subject, &client_id),
        precondition: ProjectPrecondition {
            project_id,
            expected_revision: positive(input.expected_revision, "expectedRevision")?,
        },
        name: input.name,
        description: input.description,
        folder,
        project_document_markdown: None,
        expected_project_document_digest: None,
    });
    project_payload(client_id, execute_command(state, command).await?)
}

simple_project_mutation!(
    /// Archive a project through the semantic command service.
    archive_project,
    GraphqlArchiveProjectInput,
    ArchiveProject,
    ArchiveProject
);

simple_project_mutation!(
    /// Reopen a project through the semantic command service.
    reopen_project,
    GraphqlReopenProjectInput,
    ReopenProject,
    ReopenProject
);

/// Capture an Inbox task through the semantic command service.
pub(in crate::graphql) async fn capture_task(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlCaptureTaskInput,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let project_id = input
        .project_id
        .as_deref()
        .map(parse_project_id)
        .transpose()?;
    if let Some(project_id) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let schedule = input.schedule.map(schedule_input).transpose()?;
    let command = WorkCommand::CaptureTask(CaptureTask {
        meta: command_meta(principal_subject, &client_id),
        workspace_id: workspace_id.clone(),
        title: input.title,
        task_document_markdown: input.task_document,
        project_id,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::WorkUi,
            created_by_actor_id: actor_id_for_principal(principal_subject),
            ..Default::default()
        },
        schedule,
        executor_agent_id: input.executor_agent_id,
        cwd_override: input.cwd_override,
    });
    task_payload(client_id, execute_command(state, command).await?)
}

fn schedule_input(input: GraphqlNewTaskScheduleInput) -> Result<NewTaskSchedule> {
    Ok(NewTaskSchedule {
        scheduled_for: noema_tasks::parse_utc_instant(&input.scheduled_for, "scheduledFor")
            .map_err(|_| invalid_input_error("scheduledFor"))?,
        time_zone: input.time_zone,
        missed_run_policy: input.missed_run_policy.map(Into::into).unwrap_or_default(),
        recurrence: input
            .recurrence
            .map(|recurrence| -> Result<_> {
                Ok(NewTaskRecurrence {
                    starts_at: noema_tasks::parse_utc_instant(&recurrence.starts_at, "startsAt")
                        .map_err(|_| invalid_input_error("startsAt"))?,
                    cron_expression: recurrence.cron_expression,
                    overlap_policy: recurrence
                        .overlap_policy
                        .map(Into::into)
                        .unwrap_or_default(),
                })
            })
            .transpose()?,
    })
}

pub(in crate::graphql) async fn set_task_schedule(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlScheduleTaskInput,
    reschedule: bool,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let precondition = task_precondition(
        &input.task_id,
        input.expected_revision,
        input.expected_generation,
    )?;
    require_personal_task(state.store()?, &precondition.task_id).await?;
    let schedule = schedule_input(input.schedule)?;
    let meta = command_meta(principal_subject, &client_id);
    let command = WorkCommand::ScheduleTask(ScheduleTask {
        meta,
        precondition,
        schedule,
        requires_existing: reschedule,
    });
    task_payload(client_id, execute_command(state, command).await?)
}

simple_task_mutation!(
    /// Remove future execution from an Inbox task.
    unschedule_task,
    GraphqlUnscheduleTaskInput,
    UnscheduleTask,
    UnscheduleTask
);

simple_task_mutation!(
    /// Start an already scheduled Inbox task immediately.
    run_scheduled_task_now,
    GraphqlRunScheduledTaskNowInput,
    RunScheduledTaskNow,
    RunScheduledTaskNow
);

pub(in crate::graphql) async fn update_task_recurrence(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlUpdateTaskRecurrenceInput,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let project_id = match (input.project_id, input.clear_project.unwrap_or(false)) {
        (Some(_), true) => return Err(invalid_input_error("projectId and clearProject")),
        (Some(value), false) => Some(Some(parse_project_id(&value)?)),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let recurrence_id = TaskRecurrenceId::new(input.recurrence_id)
        .map_err(|_| invalid_input_error("recurrenceId"))?;
    let command = WorkCommand::UpdateTaskRecurrence(UpdateTaskRecurrence {
        meta: command_meta(principal_subject, &client_id),
        precondition: RecurrencePrecondition {
            recurrence_id,
            expected_revision: positive(input.expected_revision, "expectedRevision")?,
        },
        title: input.title,
        task_document_markdown: input.task_document,
        expected_task_document_digest: input.expected_task_document_digest,
        project_id,
        starts_at: input
            .starts_at
            .map(|value| noema_tasks::parse_utc_instant(&value, "startsAt"))
            .transpose()
            .map_err(|_| invalid_input_error("startsAt"))?,
        cron_expression: input.cron_expression,
        time_zone: input.time_zone,
        missed_run_policy: input.missed_run_policy.map(Into::into),
        overlap_policy: input.overlap_policy.map(Into::into),
    });
    task_payload(client_id, execute_command(state, command).await?)
}

pub(in crate::graphql) async fn change_task_recurrence(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskRecurrenceCommandInput,
    action: RecurrenceCommandKind,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let recurrence_id = TaskRecurrenceId::new(input.recurrence_id)
        .map_err(|_| invalid_input_error("recurrenceId"))?;
    let command = WorkCommand::ChangeTaskRecurrence(ChangeTaskRecurrence {
        meta: command_meta(principal_subject, &client_id),
        precondition: RecurrencePrecondition {
            recurrence_id,
            expected_revision: positive(input.expected_revision, "expectedRevision")?,
        },
        action,
    });
    task_payload(client_id, execute_command(state, command).await?)
}

pub(in crate::graphql) async fn run_task_recurrence_now(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskRecurrenceCommandInput,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    let recurrence_id = TaskRecurrenceId::new(input.recurrence_id)
        .map_err(|_| invalid_input_error("recurrenceId"))?;
    let command = WorkCommand::RunTaskRecurrenceNow(RunTaskRecurrenceNow {
        meta: command_meta(principal_subject, &client_id),
        precondition: RecurrencePrecondition {
            recurrence_id,
            expected_revision: positive(input.expected_revision, "expectedRevision")?,
        },
    });
    task_payload(client_id, execute_command(state, command).await?)
}

/// Update Inbox capture fields through the semantic command service.
pub(in crate::graphql) async fn update_inbox_task(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlUpdateInboxTaskInput,
) -> Result<GraphqlTaskCommandPayload> {
    require_owner(principal_subject)?;
    let client_id = required_client_id(&input.client_mutation_id)?;
    if input.title.is_none()
        && input.task_document.is_none()
        && input.project_id.is_none()
        && !input.clear_project.unwrap_or(false)
        && input.executor_agent_id.is_none()
        && input.cwd_override.is_none()
        && !input.clear_cwd_override.unwrap_or(false)
    {
        return Err(invalid_input_error("updateInboxTask"));
    }
    let project_id = match (input.project_id, input.clear_project.unwrap_or(false)) {
        (Some(_), true) => return Err(invalid_input_error("projectId and clearProject")),
        (Some(id), false) => Some(Some(parse_project_id(&id)?)),
        (None, true) => Some(None),
        (None, false) => None,
    };
    if let Some(Some(project_id)) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    let cwd_override = match (
        input.cwd_override,
        input.clear_cwd_override.unwrap_or(false),
    ) {
        (Some(_), true) => {
            return Err(invalid_input_error("cwdOverride and clearCwdOverride"));
        }
        (Some(value), false) => Some(Some(value)),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let precondition = task_precondition(
        &input.task_id,
        input.expected_revision,
        input.expected_generation,
    )?;
    require_personal_task(state.store()?, &precondition.task_id).await?;
    let command = WorkCommand::UpdateInboxTask(UpdateInboxTask {
        meta: command_meta(principal_subject, &client_id),
        precondition,
        title: input.title,
        task_document_markdown: input.task_document,
        expected_task_document_digest: input.expected_task_document_digest,
        project_id,
        executor_agent_id: input.executor_agent_id,
        cwd_override,
    });
    task_payload(client_id, execute_command(state, command).await?)
}

simple_task_mutation!(
    /// Queue an Inbox task through the semantic command service.
    queue_task,
    GraphqlQueueTaskInput,
    QueueTask,
    QueueTask
);

fenced_task_mutation! {
    /// Resolve a human gate through the semantic command service.
    answer_task(GraphqlAnswerTaskInput, input) {
        prepare {
            let gate_id = TaskGateId::new(input.gate_id)
                .map_err(|_| invalid_input_error("gateId"))?
        }
        command |meta, precondition| WorkCommand::AnswerTask(AnswerTask {
            meta,
            precondition,
            gate_id,
            answer: noema_tasks::TaskGateAnswer {
                message_markdown: input.answer_markdown,
                approval_decision: input.approval_decision.map(Into::into),
            },
        })
    }
}

fenced_task_mutation! {
    /// Retry a Recovery gate through the semantic command service.
    retry_task(GraphqlRetryTaskInput, input) {
        prepare {
            let gate_id = TaskGateId::new(input.gate_id)
                .map_err(|_| invalid_input_error("gateId"))?
        }
        command |meta, precondition| WorkCommand::RetryTask(RetryTask {
            meta,
            precondition,
            gate_id,
            note: input.retry_note,
        })
    }
}

fenced_task_mutation! {
    /// Reopen a completed task with new direction.
    reopen_task(GraphqlReopenTaskInput, input) {
        prepare {
            let direction = TaskReopenDirection {
                feedback_markdown: input.feedback_markdown,
                request_markdown: input.request_markdown,
                complexity: input.complexity.map(Into::into),
            }
        }
        command |meta, precondition| WorkCommand::ReopenTask(ReopenTask { meta, precondition, direction })
    }
}

fenced_task_mutation! {
    /// Cancel a task through the semantic command service.
    cancel_task(GraphqlCancelTaskInput, input) {
        prepare {}
        command |meta, precondition| WorkCommand::CancelTask(CancelTask { meta, precondition, reason: input.reason })
    }
}
