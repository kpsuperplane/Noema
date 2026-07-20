//! Trusted primary-agent Work command dispatch.
//!
//! This module converts the strict model payload into typed domain commands,
//! deriving all actor/provenance and idempotency metadata from the runtime turn.

use std::str::FromStr;

use noema_store::{
    NoemaStore, ProjectQuery, WorkCommandService, WorkPageSize, WorkTaskQuery, WorkTaskScope,
};
use noema_tasks::{
    AnswerTask, ArchiveProject, CancelTask, CaptureTask, CommandMeta, CreateProject,
    DelegateExecutionIntent, DelegateTask, QueueTask, ReopenProject, ReopenTask,
    RequestTaskChanges, RetryTask, TaskContractAmendment, TaskGateAnswer, TaskGateId, TaskId,
    TaskPrecondition, TaskProvenance, TaskSourceKind, UpdateInboxTask, UpdateProject, WorkCommand,
    WorkflowStageBehavior,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    PROJECT_ARCHIVE_TOOL, PROJECT_CREATE_TOOL, PROJECT_LIST_TOOL, PROJECT_REOPEN_TOOL,
    PROJECT_UPDATE_TOOL, TASK_ACCEPT_TOOL, TASK_ANSWER_TOOL, TASK_CANCEL_TOOL, TASK_CAPTURE_TOOL,
    TASK_DELEGATE_TOOL, TASK_LIST_TOOL, TASK_QUEUE_TOOL, TASK_REOPEN_TOOL,
    TASK_REQUEST_CHANGES_TOOL, TASK_RETRY_TOOL, TASK_UPDATE_TOOL, TaskDelegateRuntimeContext,
    TaskToolResult,
    catalog::{
        CancelArguments, CaptureArguments, DelegateArguments, GateArguments,
        ProjectCreateArguments, ProjectPreconditionArguments, ProjectUpdateArguments,
        RequestChangesArguments, RetryArguments, TaskPreconditionArguments, UpdateArguments,
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
    Ok(json!({
        "tasks": [json!({
            "task_id": task.task_id,
            "title": task.title,
            "description": task.description_markdown,
            "stage_id": task.stage_id,
            "generation": task.generation,
            "revision": task.revision,
            "project_id": task.project_id,
            "attention": detail.attention.map(|attention| format!("{attention:?}").to_ascii_lowercase()),
            "valid_actions": detail.valid_actions.into_iter().map(|action| format!("{action:?}").to_ascii_lowercase()).collect::<Vec<_>>(),
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

fn criteria(
    criteria: Vec<super::catalog::CriterionArguments>,
) -> Result<Vec<noema_tasks::NewTaskValidationCriterion>, String> {
    criteria
        .into_iter()
        .enumerate()
        .map(|(index, criterion)| {
            Ok(noema_tasks::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: criterion_ordinal(index)?,
                description: criterion.description,
                expected_evidence: criterion.expected_evidence,
            })
        })
        .collect()
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
                })
            })
        }
        TASK_DELEGATE_TOOL => {
            execute_command!(service, args, input: DelegateArguments => {
                let project_id = project_id(input.project_id)?;
                let complexity_hint = input.execution_intent.is_none().then_some(input.complexity_hint).flatten();
                let intent = input
                    .execution_intent
                    .map(|intent| -> Result<DelegateExecutionIntent, String> {
                        Ok(DelegateExecutionIntent {
                            request_markdown: intent.request_markdown,
                            criteria: criteria(intent.criteria)?,
                            complexity: intent.complexity,
                            execution_plan_markdown: intent.execution_plan_markdown,
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
                WorkCommand::UpdateInboxTask(UpdateInboxTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    title: input.title,
                    description_markdown: input.description,
                    project_id,
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
        TASK_ACCEPT_TOOL => {
            execute_command!(service, args, input: TaskPreconditionArguments =>
                WorkCommand::AcceptTask(noema_tasks::AcceptTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input)?,
                })
            )
        }
        TASK_REQUEST_CHANGES_TOOL => {
            execute_command!(service, args, input: RequestChangesArguments => {
                let replacement_criteria = input.replacement_criteria.map(criteria).transpose()?;
                WorkCommand::RequestTaskChanges(RequestTaskChanges {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input.precondition)?,
                    amendment: TaskContractAmendment {
                        feedback_markdown: input.feedback_markdown,
                        request_markdown: input.request_markdown,
                        replacement_criteria,
                        complexity: input.complexity,
                    },
                })
            })
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
            execute_command!(service, args, input: TaskPreconditionArguments =>
                WorkCommand::ReopenTask(ReopenTask {
                    meta: meta(call_id.clone()),
                    precondition: task_precondition(&input)?,
                })
            )
        }
        PROJECT_CREATE_TOOL => {
            execute_command!(service, args, input: ProjectCreateArguments =>
                WorkCommand::CreateProject(CreateProject {
                    meta: meta(call_id.clone()),
                    workspace_id,
                    name: input.name,
                    description: input.description,
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
    Ok(command_result_payload(result))
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

fn criterion_ordinal(index: usize) -> Result<u32, String> {
    index
        .checked_add(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or_else(|| "criterion count exceeds the supported bound".to_string())
}

fn command_result_payload(result: noema_tasks::WorkCommandResult) -> Value {
    json!({"task": result.task.map(|task| json!({"task_id":task.task_id,"title":task.title,"stage_id":task.stage_id,"generation":task.generation,"revision":task.revision,"project_id":task.project_id})),"project": result.project.map(|project| json!({"project_id":project.project_id,"name":project.name,"description":project.description,"revision":project.revision,"archived":project.archived_at.is_some()})),"contract_id":result.contract_id,"gate_id":result.gate_id,"run_id":result.run_id,"event_id":result.event_id,"event_sequence":result.event_sequence})
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
    Ok(
        json!({"tasks": connection.edges.into_iter().map(|edge| { let task=edge.node.task; json!({"task_id":task.task_id,"title":task.title,"description":task.description_markdown,"stage_id":task.stage_id,"generation":task.generation,"revision":task.revision,"project_id":task.project_id,"attention":edge.node.attention.map(|attention| format!("{attention:?}").to_ascii_lowercase()),"valid_actions":edge.node.valid_actions.into_iter().map(|action| format!("{action:?}").to_ascii_lowercase()).collect::<Vec<_>>()}) }).collect::<Vec<_>>(),"has_next_page":connection.page_info.has_next_page,"end_cursor":connection.page_info.end_cursor}),
    )
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
    use super::project_id;

    #[test]
    fn blank_optional_project_ids_are_omitted() {
        assert!(project_id(Some(String::new())).unwrap().is_none());
        assert!(project_id(Some(" \t".to_string())).unwrap().is_none());
    }
}
