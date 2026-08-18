//! Trusted primary-agent Work command dispatch.
//!
//! This module converts the strict model payload into typed domain commands,
//! deriving all actor/provenance and idempotency metadata from the runtime turn.

use std::str::FromStr;

use noema_store::{
    NoemaStore, ProjectQuery, WorkCommandService, WorkPageSize, WorkTaskQuery, WorkTaskScope,
    WorkTaskValidAction,
};
use noema_tasks::{
    AnswerTask, ArchiveProject, CancelTask, CaptureTask, ChangeTaskRecurrence, CommandMeta,
    CreateProject, DelegateExecutionIntent, DelegateTask, NewTaskRecurrence, NewTaskSchedule,
    QueueTask, RecurrenceCommandKind, RecurrencePrecondition, ReopenProject, ReopenTask, RetryTask,
    RunScheduledTaskNow, RunTaskRecurrenceNow, ScheduleTask, TaskGateAnswer, TaskGateId,
    TaskGateRecord, TaskId, TaskPrecondition, TaskProvenance, TaskRecurrenceId,
    TaskRecurrenceRecord, TaskReopenDirection, TaskSourceKind, UnscheduleTask, UpdateInboxTask,
    UpdateProject, UpdateTaskRecurrence, WorkCommand, WorkflowStageBehavior,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    PROJECT_ARCHIVE_TOOL, PROJECT_CREATE_TOOL, PROJECT_LIST_TOOL, PROJECT_REOPEN_TOOL,
    PROJECT_UPDATE_TOOL, TASK_ANSWER_TOOL, TASK_CANCEL_TOOL, TASK_CAPTURE_TOOL, TASK_DELEGATE_TOOL,
    TASK_FILE_DELETE_TOOL, TASK_FILE_LIST_TOOL, TASK_FILE_READ_TOOL, TASK_FILE_WRITE_TOOL,
    TASK_LIST_TOOL, TASK_QUEUE_TOOL, TASK_RECURRENCE_END_TOOL, TASK_RECURRENCE_PAUSE_TOOL,
    TASK_RECURRENCE_RESUME_TOOL, TASK_RECURRENCE_SKIP_NEXT_TOOL, TASK_RECURRENCE_UPDATE_TOOL,
    TASK_REOPEN_TOOL, TASK_RESCHEDULE_TOOL, TASK_RETRY_TOOL, TASK_RUN_RECURRENCE_NOW_TOOL,
    TASK_RUN_SCHEDULED_NOW_TOOL, TASK_SCHEDULE_TOOL, TASK_UNSCHEDULE_TOOL, TASK_UPDATE_TOOL,
    TaskDelegateRuntimeContext, TaskToolResult,
    catalog::{
        CancelArguments, CaptureArguments, DelegateArguments, DelegateProjectArguments,
        GateArguments, ProjectCreateArguments, ProjectPreconditionArguments,
        ProjectUpdateArguments, RecurrencePreconditionArguments, RecurrenceUpdateArguments,
        ReopenArguments, RetryArguments, ScheduleArguments, ScheduleFieldsArguments,
        TaskPreconditionArguments, UpdateArguments,
    },
};

macro_rules! execute_command {
    ($service:expr, $args:expr, $input:ident: $ty:ty => $command:expr) => {{
        let $input: $ty = parse_arguments($args)?;
        $service
            .execute($command)
            .await
            .map_err(|error| error.to_string())?
    }};
}

/// Execute a primary task/project tool through the semantic command service.
pub(crate) async fn execute_primary_task_tool(
    store: &NoemaStore,
    provider_registry: &noema_providers::ProviderRegistryHandle,
    context: &TaskDelegateRuntimeContext,
    name: &str,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = execute_primary_inner(
        store,
        provider_registry,
        context,
        name,
        call_id.clone(),
        payload,
    )
    .await;
    task_tool_result(name, call_id, result)
}

/// Inspect only the task attached to the active background run.
pub(crate) async fn execute_scoped_task_list_tool(
    store: &NoemaStore,
    task_id: &str,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = execute_scoped_task_list_inner(store, task_id, payload).await;
    task_tool_result(TASK_LIST_TOOL, call_id, result)
}

/// Execute one file operation for the Task attached to the active run.
pub(crate) async fn execute_scoped_task_file_tool(
    store: &NoemaStore,
    task_id: &str,
    role: crate::agent_execution::ExecutionRole,
    name: &str,
    call_id: Option<String>,
    payload: &Value,
) -> TaskToolResult {
    let result = execute_scoped_task_file_inner(store, task_id, role, name, payload).await;
    task_tool_result(name, call_id, result)
}

async fn execute_scoped_task_file_inner(
    store: &NoemaStore,
    task_id: &str,
    role: crate::agent_execution::ExecutionRole,
    name: &str,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    let task_id = TaskId::new(task_id.trim().to_string()).map_err(|error| error.to_string())?;
    let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
    match name {
        TASK_FILE_LIST_TOOL => {
            let entries = store
                .list_task_files(&task_id, path)
                .await
                .map_err(|error| error.to_string())?;
            Ok(json!({"entries": entries}))
        }
        TASK_FILE_READ_TOOL => {
            require_path(&arguments)?;
            let content = store
                .read_task_file(&task_id, path)
                .await
                .map_err(|error| error.to_string())?;
            Ok(json!({"path": path, "content": content}))
        }
        TASK_FILE_WRITE_TOOL => {
            require_file_writer(role)?;
            require_path(&arguments)?;
            let content = arguments
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "task.files.write requires content".to_string())?;
            store
                .write_task_file(&task_id, path, content)
                .await
                .map_err(|error| error.to_string())?;
            Ok(json!({"path": path}))
        }
        TASK_FILE_DELETE_TOOL => {
            require_file_writer(role)?;
            require_path(&arguments)?;
            store
                .delete_task_file(&task_id, path)
                .await
                .map_err(|error| error.to_string())?;
            Ok(json!({"path": path}))
        }
        _ => Err("unknown Task file operation".to_string()),
    }
}

fn require_path(arguments: &Value) -> Result<(), String> {
    if arguments
        .get("path")
        .and_then(Value::as_str)
        .is_some_and(|path| !path.trim().is_empty())
    {
        Ok(())
    } else {
        Err("Task file path is required".to_string())
    }
}

fn require_file_writer(role: crate::agent_execution::ExecutionRole) -> Result<(), String> {
    if matches!(
        role,
        crate::agent_execution::ExecutionRole::TaskPlanner
            | crate::agent_execution::ExecutionRole::TaskExecutor
    ) {
        Ok(())
    } else {
        Err("This Task role cannot change Task files".to_string())
    }
}

async fn execute_scoped_task_list_inner(
    store: &NoemaStore,
    task_id: &str,
    payload: &Value,
) -> Result<Value, String> {
    let args = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    if !args.is_object() || args.as_object().is_some_and(|object| !object.is_empty()) {
        return Err(
            "task.list does not accept model-supplied scope filters during a background run"
                .to_string(),
        );
    }
    let task_id = TaskId::new(task_id.trim().to_string()).map_err(|error| error.to_string())?;
    let detail = store
        .get_work_task(&task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "current task is unavailable".to_string())?;
    let task = detail.task;
    let recurrence_authority =
        current_recurrence_authority(store, task.recurrence_id.as_ref()).await?;
    Ok(json!({
        "tasks": [json!({
            "task_id": task.task_id,
            "title": task.title,
            "description": task.description_markdown,
            "stage_id": task.stage_id,
            "generation": task.generation,
            "revision": task.revision,
            "project_id": task.project_id,
            "scheduled_for": task.scheduled_for,
            "schedule_time_zone": task.schedule_time_zone,
            "missed_run_policy": task.missed_run_policy,
            "recurrence_id": task.recurrence_id,
            "recurrence_revision": task.recurrence_revision,
            "recurrence_scheduled_for": task.recurrence_scheduled_for,
            "recurrence_authority": recurrence_authority,
            "active_gate": active_gate_payload(detail.active_gate.as_ref()),
            "attention": detail.attention.map(|attention| format!("{attention:?}").to_ascii_lowercase()),
            "valid_actions": detail.valid_actions.into_iter().map(serialized_action).collect::<Vec<_>>(),
        })],
        "has_next_page": false,
        "end_cursor": Value::Null,
    }))
}

fn task_tool_result(
    name: &str,
    call_id: Option<String>,
    result: Result<Value, String>,
) -> TaskToolResult {
    match result {
        Ok(payload) => TaskToolResult {
            call_id,
            name: name.to_string(),
            success: true,
            payload,
        },
        Err(error) => TaskToolResult {
            call_id,
            name: name.to_string(),
            success: false,
            payload: json!({"code": "invalid_input", "message": error}),
        },
    }
}

fn parse_arguments<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}

async fn execute_primary_inner(
    store: &NoemaStore,
    provider_registry: &noema_providers::ProviderRegistryHandle,
    context: &TaskDelegateRuntimeContext,
    name: &str,
    call_id: Option<String>,
    payload: &Value,
) -> Result<Value, String> {
    let args = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    let service = WorkCommandService::new(store.clone(), provider_registry.clone());
    let meta = |idempotency: Option<String>| CommandMeta {
        actor_id: work_actor_id(&context.agent_id),
        causation_id: None,
        correlation_id: format!("correlation:turn:{}", context.turn_id),
        idempotency_key: idempotency,
    };
    let workspace_id =
        WorkspaceId::new(context.workspace_id.clone()).map_err(|error| error.to_string())?;
    let result = match name {
        TASK_CAPTURE_TOOL => {
            execute_command!(service, args, input: CaptureArguments => {
                let project_id = project_id(input.project_id)?;
                WorkCommand::CaptureTask(CaptureTask {
                    meta: meta(call_id.clone()),
                    workspace_id,
                    title: input.title,
                    description_markdown: input.description,
                    project_id,
                    provenance: provenance(context, TaskSourceKind::ChatCapture, call_id.clone()),
                    schedule: input.schedule.map(|value| schedule(value, context)).transpose()?,
                    executor_agent_id: input.executor_agent_id,
                    cwd_override: input.cwd_override,
                })
            })
        }
        TASK_DELEGATE_TOOL => {
            execute_command!(service, args, input: DelegateArguments => {
                let project_id = match input.project {
                    DelegateProjectArguments::None => None,
                    DelegateProjectArguments::Existing { project_id: id } => project_id(Some(id))?,
                };
                let complexity_hint = input.execution_intent.is_none().then_some(input.complexity_hint).flatten();
                let intent = input
                    .execution_intent
                    .map(|intent| -> Result<DelegateExecutionIntent, String> {
                        Ok(DelegateExecutionIntent {
                            request_markdown: intent.request_markdown,
                            complexity: intent.complexity,
                        })
                    })
                    .transpose()?;
                WorkCommand::DelegateTask(DelegateTask {
                    meta: meta(call_id.clone()),
                    workspace_id,
                    title: input.title,
                    description_markdown: input.description,
                    project_id,
                    provenance: provenance(context, TaskSourceKind::ChatDelegate, call_id.clone()),
                    complexity_hint,
                    execution_intent: intent,
                    executor_agent_id: input.executor_agent_id,
                    cwd_override: input.cwd_override,
                })
            })
        }
        TASK_UPDATE_TOOL => {
            execute_command!(service, args, input: UpdateArguments => {
                let project_id = if input.clear_project {
                    Some(None)
                } else {
                    project_id(input.project_id)?.map(Some)
                };
                if input.clear_cwd_override && input.cwd_override.is_some() {
                    return Err("cwd_override and clear_cwd_override are mutually exclusive".to_string());
                }
                let cwd_override = if input.clear_cwd_override {
                    Some(None)
                } else {
                    input.cwd_override.map(Some)
                };
                WorkCommand::UpdateInboxTask(UpdateInboxTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    title: input.title,
                    description_markdown: input.description,
                    project_id,
                    executor_agent_id: input.executor_agent_id,
                    cwd_override,
                })
            })
        }
        TASK_QUEUE_TOOL => {
            execute_command!(service, args, input: TaskPreconditionArguments =>
                WorkCommand::QueueTask(QueueTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input)?,
                })
            )
        }
        TASK_SCHEDULE_TOOL | TASK_RESCHEDULE_TOOL => {
            execute_command!(service, args, input: ScheduleArguments => {
                let precondition = task_precondition(&input.precondition)?;
                let schedule = schedule(input.schedule, context)?;
                WorkCommand::ScheduleTask(ScheduleTask {
                    meta: meta(call_id.clone()), precondition, schedule,
                    requires_existing: name == TASK_RESCHEDULE_TOOL,
                })
            })
        }
        TASK_UNSCHEDULE_TOOL => {
            execute_command!(service, args, input: TaskPreconditionArguments =>
                WorkCommand::UnscheduleTask(UnscheduleTask { meta: meta(call_id.clone()), precondition: task_precondition(&input)? })
            )
        }
        TASK_RUN_SCHEDULED_NOW_TOOL => {
            execute_command!(service, args, input: TaskPreconditionArguments =>
                WorkCommand::RunScheduledTaskNow(RunScheduledTaskNow { meta: meta(call_id.clone()), precondition: task_precondition(&input)? })
            )
        }
        TASK_RECURRENCE_UPDATE_TOOL => {
            execute_command!(service, args, input: RecurrenceUpdateArguments => {
                let project_id = if input.clear_project { Some(None) } else { project_id(input.project_id)?.map(Some) };
                WorkCommand::UpdateTaskRecurrence(UpdateTaskRecurrence {
                    meta: meta(call_id.clone()), precondition: recurrence_precondition(input.precondition)?,
                    title: input.title, description_markdown: input.description, project_id,
                    starts_at: input.starts_at.map(|value| noema_tasks::parse_utc_instant(&value, "starts_at")).transpose().map_err(|error| error.to_string())?,
                    cron_expression: input.cron_expression, time_zone: input.time_zone,
                    missed_run_policy: input.missed_run_policy, overlap_policy: input.overlap_policy,
                })
            })
        }
        TASK_RECURRENCE_PAUSE_TOOL
        | TASK_RECURRENCE_RESUME_TOOL
        | TASK_RECURRENCE_SKIP_NEXT_TOOL
        | TASK_RECURRENCE_END_TOOL => {
            execute_command!(service, args, input: RecurrencePreconditionArguments => {
                let precondition = recurrence_precondition(input)?;
                let action = match name {
                    TASK_RECURRENCE_PAUSE_TOOL => RecurrenceCommandKind::Pause,
                    TASK_RECURRENCE_RESUME_TOOL => RecurrenceCommandKind::Resume,
                    TASK_RECURRENCE_SKIP_NEXT_TOOL => RecurrenceCommandKind::SkipNext,
                    _ => RecurrenceCommandKind::End,
                };
                WorkCommand::ChangeTaskRecurrence(ChangeTaskRecurrence { meta: meta(call_id.clone()), precondition, action })
            })
        }
        TASK_RUN_RECURRENCE_NOW_TOOL => {
            execute_command!(service, args, input: RecurrencePreconditionArguments =>
                WorkCommand::RunTaskRecurrenceNow(RunTaskRecurrenceNow {
                    meta: meta(call_id.clone()), precondition: recurrence_precondition(input)?,
                })
            )
        }
        TASK_ANSWER_TOOL => {
            execute_command!(service, args, input: GateArguments =>
                WorkCommand::AnswerTask(AnswerTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    gate_id: TaskGateId::new(input.gate_id).map_err(|error| error.to_string())?,
                    answer: TaskGateAnswer {
                        message_markdown: input.answer_markdown,
                        approval_decision: input.approval_decision,
                    },
                })
            )
        }
        TASK_RETRY_TOOL => {
            execute_command!(service, args, input: RetryArguments =>
                WorkCommand::RetryTask(RetryTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    gate_id: TaskGateId::new(input.gate_id).map_err(|error| error.to_string())?,
                    note: input.retry_note,
                })
            )
        }
        TASK_CANCEL_TOOL => {
            execute_command!(service, args, input: CancelArguments =>
                WorkCommand::CancelTask(CancelTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    reason: input.reason,
                })
            )
        }
        TASK_REOPEN_TOOL => {
            execute_command!(service, args, input: ReopenArguments => {
                WorkCommand::ReopenTask(ReopenTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    direction: TaskReopenDirection {
                        feedback_markdown: input.feedback_markdown,
                        request_markdown: input.request_markdown,
                        complexity: input.complexity,
                    },
                })
            })
        }
        PROJECT_CREATE_TOOL => {
            execute_command!(service, args, input: ProjectCreateArguments =>
                WorkCommand::CreateProject(CreateProject {
                    meta: meta(call_id.clone()),
                    workspace_id,
                    name: input.name,
                    description: input.description,
                    folder: input.folder,
                })
            )
        }
        PROJECT_UPDATE_TOOL => {
            execute_command!(service, args, input: ProjectUpdateArguments =>
                WorkCommand::UpdateProject(UpdateProject {
                    meta: meta(call_id.clone()),
                    precondition: project_precondition(input.precondition)?,
                    name: input.name,
                    description: input.description,
                    folder: if input.clear_folder { Some(None) } else { input.folder.map(Some) },
                })
            )
        }
        PROJECT_ARCHIVE_TOOL => {
            execute_command!(service, args, input: ProjectPreconditionArguments =>
                WorkCommand::ArchiveProject(ArchiveProject {
                    meta: meta(call_id.clone()),
                    precondition: project_precondition(input)?,
                })
            )
        }
        PROJECT_REOPEN_TOOL => {
            execute_command!(service, args, input: ProjectPreconditionArguments =>
                WorkCommand::ReopenProject(ReopenProject {
                    meta: meta(call_id.clone()),
                    precondition: project_precondition(input)?,
                })
            )
        }
        TASK_LIST_TOOL => {
            return list_tasks(store, &context.owner_human_id, &workspace_id, &args).await;
        }
        PROJECT_LIST_TOOL => return list_projects(store, &workspace_id, &args).await,
        _ => return Err("unknown primary Work tool".to_string()),
    };
    command_result_payload(store, result).await
}

fn provenance(
    context: &TaskDelegateRuntimeContext,
    source_kind: TaskSourceKind,
    call_id: Option<String>,
) -> TaskProvenance {
    TaskProvenance {
        source_kind,
        conversation_id: Some(context.conversation_id.clone()),
        turn_id: Some(context.turn_id.clone()),
        item_id: Some(context.user_item_id.clone()),
        source_tool_call_id: call_id,
        created_by_actor_id: work_actor_id(&context.agent_id),
    }
}

fn work_actor_id(agent_id: &str) -> String {
    if agent_id.starts_with("actor:") {
        agent_id.to_string()
    } else {
        format!("actor:{agent_id}")
    }
}

fn task_precondition(input: &TaskPreconditionArguments) -> Result<TaskPrecondition, String> {
    Ok(TaskPrecondition {
        task_id: TaskId::new(input.task_id.trim().to_string())
            .map_err(|error| error.to_string())?,
        expected_revision: input.expected_revision,
        expected_generation: input.expected_generation,
    })
}

fn recurrence_precondition(
    input: RecurrencePreconditionArguments,
) -> Result<RecurrencePrecondition, String> {
    Ok(RecurrencePrecondition {
        recurrence_id: TaskRecurrenceId::new(input.recurrence_id)
            .map_err(|error| error.to_string())?,
        expected_revision: input.expected_revision,
    })
}

fn schedule(
    input: ScheduleFieldsArguments,
    context: &TaskDelegateRuntimeContext,
) -> Result<NewTaskSchedule, String> {
    Ok(NewTaskSchedule {
        scheduled_for: noema_tasks::parse_utc_instant(&input.scheduled_for, "scheduled_for")
            .map_err(|error| error.to_string())?,
        time_zone: input
            .time_zone
            .unwrap_or_else(|| context.client_time_zone.clone()),
        missed_run_policy: input.missed_run_policy.unwrap_or_default(),
        recurrence: input
            .recurrence
            .map(|value| -> Result<_, String> {
                Ok(NewTaskRecurrence {
                    starts_at: noema_tasks::parse_utc_instant(&value.starts_at, "starts_at")
                        .map_err(|error| error.to_string())?,
                    cron_expression: value.cron_expression,
                    overlap_policy: value.overlap_policy.unwrap_or_default(),
                })
            })
            .transpose()?,
    })
}

fn project_id(id: Option<String>) -> Result<Option<ProjectId>, String> {
    id.filter(|id| !id.trim().is_empty())
        .map(|id| ProjectId::new(id).map_err(|error| error.to_string()))
        .transpose()
}

fn project_precondition(
    input: ProjectPreconditionArguments,
) -> Result<noema_tasks::ProjectPrecondition, String> {
    Ok(noema_tasks::ProjectPrecondition {
        project_id: ProjectId::new(input.project_id).map_err(|error| error.to_string())?,
        expected_revision: input.expected_revision,
    })
}

async fn command_result_payload(
    store: &NoemaStore,
    result: noema_tasks::WorkCommandResult,
) -> Result<Value, String> {
    let recurrence_id = result
        .task
        .as_ref()
        .and_then(|task| task.recurrence_id.as_ref());
    let recurrence_authority = current_recurrence_authority(store, recurrence_id).await?;
    Ok(
        json!({"task": result.task.map(|task| json!({"task_id":task.task_id,"title":task.title,"stage_id":task.stage_id,"generation":task.generation,"revision":task.revision,"project_id":task.project_id,"scheduled_for":task.scheduled_for,"schedule_time_zone":task.schedule_time_zone,"recurrence_id":task.recurrence_id,"recurrence_revision":task.recurrence_revision})),"recurrence_authority":recurrence_authority,"project": result.project.map(|project| json!({"project_id":project.project_id,"name":project.name,"description":project.description,"revision":project.revision,"archived":project.archived_at.is_some()})),"gate_id":result.gate_id,"run_id":result.run_id,"event_id":result.event_id,"event_sequence":result.event_sequence}),
    )
}

fn active_gate_payload(gate: Option<&TaskGateRecord>) -> Value {
    gate.map_or(Value::Null, |gate| {
        json!({
            "gate_id": gate.gate_id,
            "kind": gate.kind,
            "prompt": gate.prompt_markdown,
            "context": gate.context_markdown,
            "suggested_answers": gate.suggested_answers,
        })
    })
}

fn recurrence_authority_payload(recurrence: &TaskRecurrenceRecord) -> Value {
    json!({
        "recurrence_id": recurrence.recurrence_id,
        "title": recurrence.title,
        "description": recurrence.description_markdown,
        "project_id": recurrence.project_id,
        "starts_at": recurrence.starts_at,
        "cron_expression": recurrence.cron_expression,
        "time_zone": recurrence.time_zone,
        "missed_run_policy": recurrence.missed_run_policy,
        "overlap_policy": recurrence.overlap_policy,
        "lifecycle": recurrence.lifecycle,
        "revision": recurrence.revision,
        "next_run_at": recurrence.next_run_at,
        "pending_coalesced_at": recurrence.pending_coalesced_at,
    })
}

async fn current_recurrence_authority(
    store: &NoemaStore,
    recurrence_id: Option<&TaskRecurrenceId>,
) -> Result<Value, String> {
    let Some(recurrence_id) = recurrence_id else {
        return Ok(Value::Null);
    };
    store
        .get_task_recurrence(recurrence_id)
        .await
        .map_err(|error| error.to_string())?
        .as_ref()
        .map_or(Ok(Value::Null), |recurrence| {
            Ok(recurrence_authority_payload(recurrence))
        })
}

async fn list_tasks(
    store: &NoemaStore,
    _owner: &str,
    workspace_id: &WorkspaceId,
    args: &Value,
) -> Result<Value, String> {
    let project_id = project_id(
        args.get("project_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    )?;
    let behavior = args
        .get("stage_behavior")
        .and_then(Value::as_str)
        .map(WorkflowStageBehavior::from_str)
        .transpose()
        .map_err(|error| error.to_string())?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 100) as u32;
    let query = WorkTaskQuery {
        workspace_id: workspace_id.clone(),
        project_id,
        stage_ids: Vec::new(),
        stage_behaviors: behavior.into_iter().collect(),
        text: None,
        attention_only: args
            .get("attention_only")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        scope: WorkTaskScope::All,
        first: WorkPageSize::new(limit).map_err(|_| "invalid limit".to_string())?,
        after: None,
    };
    let connection = store
        .list_work_tasks(query)
        .await
        .map_err(|error| error.to_string())?;
    let mut tasks = Vec::with_capacity(connection.edges.len());
    for edge in connection.edges {
        let summary = edge.node;
        let task = summary.task;
        let recurrence_authority =
            current_recurrence_authority(store, task.recurrence_id.as_ref()).await?;
        tasks.push(json!({
            "task_id": task.task_id, "title": task.title,
            "description": task.description_markdown, "stage_id": task.stage_id,
            "generation": task.generation, "revision": task.revision,
            "project_id": task.project_id, "scheduled_for": task.scheduled_for,
            "schedule_time_zone": task.schedule_time_zone,
            "missed_run_policy": task.missed_run_policy, "recurrence_id": task.recurrence_id,
            "recurrence_revision": task.recurrence_revision,
            "recurrence_scheduled_for": task.recurrence_scheduled_for,
            "recurrence_authority": recurrence_authority,
            "active_gate": active_gate_payload(summary.active_gate.as_ref()),
            "attention": summary.attention.map(|attention| format!("{attention:?}").to_ascii_lowercase()),
            "valid_actions": summary.valid_actions.into_iter().map(serialized_action).collect::<Vec<_>>(),
        }));
    }
    Ok(json!({
        "tasks": tasks,
        "has_next_page": connection.page_info.has_next_page,
        "end_cursor": connection.page_info.end_cursor,
    }))
}

fn serialized_action(action: WorkTaskValidAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

async fn list_projects(
    store: &NoemaStore,
    workspace_id: &WorkspaceId,
    args: &Value,
) -> Result<Value, String> {
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 100) as u32;
    let connection = store
        .list_work_projects(ProjectQuery {
            workspace_id: workspace_id.clone(),
            include_archived: args
                .get("include_archived")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            first: WorkPageSize::new(limit).map_err(|_| "invalid limit".to_string())?,
            after: None,
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(
        json!({"projects": connection.edges.into_iter().map(|edge| json!({"project_id":edge.node.project_id,"name":edge.node.name,"description":edge.node.description,"revision":edge.node.revision,"archived":edge.node.archived_at.is_some()})).collect::<Vec<_>>(),"has_next_page":connection.page_info.has_next_page,"end_cursor":connection.page_info.end_cursor}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_id(value: &str) -> TaskId {
        TaskId::new(value.to_string()).expect("task id")
    }

    #[test]
    fn blank_optional_project_ids_are_omitted() {
        assert!(project_id(Some(String::new())).unwrap().is_none());
        assert!(project_id(Some(" \t".to_string())).unwrap().is_none());
    }

    #[test]
    fn active_gate_projection_exposes_exact_answer_authority() {
        let gate = TaskGateRecord {
            gate_id: TaskGateId::new("gate:current".to_string()).expect("gate id"),
            task_id: task_id("task:current"),
            task_generation: 3,
            kind: noema_tasks::TaskGateKind::Clarification,
            state: noema_tasks::TaskGateState::Open,
            recovery_reason: None,
            retry_run_kind: None,
            prompt_markdown: "Which date range?".to_string(),
            context_markdown: "The current occurrence needs one range.".to_string(),
            suggested_answers: vec!["Previous 24 hours".to_string()],
            opened_by_actor_id: "actor:runtime:worker".to_string(),
            originating_run_id: None,
            resolved_by_actor_id: None,
            resolution_message_id: None,
            opened_at: "2026-08-11T14:00:00Z".to_string(),
            resolved_at: None,
        };

        assert_eq!(
            active_gate_payload(Some(&gate)),
            json!({
                "gate_id": "gate:current",
                "kind": "clarification",
                "prompt": "Which date range?",
                "context": "The current occurrence needs one range.",
                "suggested_answers": ["Previous 24 hours"],
            })
        );
    }

    #[test]
    fn recurrence_projection_exposes_current_future_authority() {
        let recurrence = TaskRecurrenceRecord {
            recurrence_id: TaskRecurrenceId::new("recurrence:current".to_string())
                .expect("recurrence id"),
            workspace_id: WorkspaceId::new("workspace:personal".to_string()).expect("workspace id"),
            project_id: None,
            title: "Daily briefing".to_string(),
            description_markdown: "Use the previous 24 hours.".to_string(),
            authorization_context: noema_tasks::TaskAuthorizationContext::None,
            starts_at: 1_786_456_800,
            cron_expression: "0 7 * * *".to_string(),
            time_zone: "America/Los_Angeles".to_string(),
            missed_run_policy: noema_tasks::MissedRunPolicy::RunOnce,
            overlap_policy: noema_tasks::OverlapPolicy::Skip,
            lifecycle: noema_tasks::RecurrenceLifecycle::Active,
            revision: 3,
            next_run_at: Some(1_786_543_200),
            pending_coalesced_at: None,
            created_at: "2026-08-01T00:00:00Z".to_string(),
            updated_at: "2026-08-11T14:00:00Z".to_string(),
        };

        let payload = recurrence_authority_payload(&recurrence);
        assert_eq!(payload["recurrence_id"], "recurrence:current");
        assert_eq!(payload["description"], "Use the previous 24 hours.");
        assert_eq!(payload["revision"], 3);
        assert_eq!(payload["cron_expression"], "0 7 * * *");
        assert_eq!(payload["time_zone"], "America/Los_Angeles");
        assert_eq!(payload["lifecycle"], "active");
    }
}
