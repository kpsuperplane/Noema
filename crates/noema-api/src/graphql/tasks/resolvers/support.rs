use async_graphql::{Error, ErrorExtensions, Result};
use noema_store::{StoreError, WorkCommandService, WorkEventCursor, WorkPageSize};
use noema_tasks::{CommandMeta, TaskId, TaskPrecondition, WorkCommand};
use noema_workspaces::{ProjectId, WorkspaceId};

use crate::graphql::schema::GraphqlState;
use crate::graphql::tasks::*;

pub(crate) async fn task_simple_command<F>(
    state: &GraphqlState,
    principal_subject: &str,
    client_mutation_id: &str,
    precondition: TaskPrecondition,
    build: F,
) -> Result<GraphqlTaskCommandPayload>
where
    F: FnOnce(CommandMeta, TaskPrecondition) -> WorkCommand,
{
    require_owner(principal_subject)?;
    let client_id = required_client_id(client_mutation_id)?;
    require_personal_task(state.store()?, &precondition.task_id).await?;
    let command = build(command_meta(principal_subject, &client_id), precondition);
    task_payload(client_id, execute_command(state, command).await?)
}

pub(crate) async fn execute_command(
    state: &GraphqlState,
    command: WorkCommand,
) -> Result<noema_store::CommittedWorkCommandResult> {
    let store = state.store()?.clone();
    let registry = state.provider_registry()?.clone();
    let task_id = command_task_id(&command);
    let service = WorkCommandService::new(store, registry);
    let committed = service
        .execute_committed(command)
        .await
        .map_err(work_error)?;
    let result = &committed.result;
    if let Some(workspace_id) = result
        .task
        .as_ref()
        .map(|task| task.workspace_id.to_string())
        .or_else(|| {
            result
                .project
                .as_ref()
                .map(|project| project.workspace_id.to_string())
        })
    {
        state
            .subscriptions()
            .publish_work(noema_runtime::WorkRuntimeEvent::Committed {
                workspace_id,
                task_id: result.task.as_ref().map(|task| task.task_id.to_string()),
            });
    }
    if let Some(task_id) = task_id.or_else(|| result.task.as_ref().map(|task| task.task_id.clone()))
    {
        state
            .subscriptions()
            .publish_task(noema_runtime::TaskRuntimeEvent::Changed {
                task_id: task_id.to_string(),
                run_id: None,
            });
    }
    Ok(committed)
}

pub(crate) fn command_task_id(command: &WorkCommand) -> Option<TaskId> {
    match command {
        WorkCommand::UpdateInboxTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::QueueTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::ScheduleTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::UnscheduleTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::AnswerTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::RetryTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::CancelTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::ReopenTask(value) => Some(value.precondition.task_id.clone()),
        WorkCommand::CaptureTask(_)
        | WorkCommand::CreateProject(_)
        | WorkCommand::UpdateProject(_)
        | WorkCommand::ArchiveProject(_)
        | WorkCommand::ReopenProject(_)
        | WorkCommand::DelegateTask(_)
        | WorkCommand::UpdateTaskRecurrence(_)
        | WorkCommand::ChangeTaskRecurrence(_) => None,
    }
}

pub(crate) fn task_payload(
    client_mutation_id: String,
    result: noema_store::CommittedWorkCommandResult,
) -> Result<GraphqlTaskCommandPayload> {
    let detail = result.task_detail.ok_or_else(unavailable)?;
    Ok(GraphqlTaskCommandPayload {
        task: detail_from_store(detail)?,
        event_cursor: event_cursor(result.result.event_sequence)?,
        client_mutation_id,
    })
}

pub(crate) fn project_payload(
    client_mutation_id: String,
    result: noema_store::CommittedWorkCommandResult,
) -> Result<GraphqlProjectCommandPayload> {
    let project = result.result.project.ok_or_else(unavailable)?;
    Ok(GraphqlProjectCommandPayload {
        project: project.try_into()?,
        event_cursor: event_cursor(result.result.event_sequence)?,
        client_mutation_id,
    })
}

pub(crate) async fn validate_scope_filters(
    store: &noema_store::NoemaStore,
    workspace_id: &WorkspaceId,
    scope: GraphqlWorkTaskScope,
    stage_ids: &[noema_tasks::WorkflowStageId],
    behaviors: &[noema_tasks::WorkflowStageBehavior],
) -> Result<()> {
    let mut selected_behaviors = behaviors.to_vec();
    let mut stage_behaviors = Vec::new();
    if !stage_ids.is_empty() {
        let workflows = store
            .list_work_workflows(workspace_id)
            .await
            .map_err(work_error)?;
        let stages = workflows
            .into_iter()
            .flat_map(|workflow| workflow.stages)
            .collect::<Vec<_>>();
        for stage_id in stage_ids {
            let stage = stages
                .iter()
                .find(|stage| stage.stage_id == *stage_id)
                .ok_or_else(|| invalid_input_error("stageId"))?;
            stage_behaviors.push((stage_id, stage.system_behavior));
            selected_behaviors.push(stage.system_behavior);
        }
    }
    selected_behaviors.sort_by_key(|behavior| behavior.as_str());
    selected_behaviors.dedup();
    let has_terminal = selected_behaviors
        .iter()
        .any(|behavior| behavior.is_terminal());
    let has_active = selected_behaviors
        .iter()
        .any(|behavior| !behavior.is_terminal());
    if (scope == GraphqlWorkTaskScope::Active && has_terminal)
        || (scope == GraphqlWorkTaskScope::Terminal && has_active)
    {
        return Err(work_error(noema_store::StoreError::Work(
            noema_tasks::WorkDomainError::WorkflowMismatch,
        )));
    }
    if !behaviors.is_empty() && !stage_ids.is_empty() {
        let stage_matches_behavior = stage_behaviors
            .iter()
            .any(|(_, behavior)| behaviors.contains(behavior));
        if !stage_matches_behavior {
            return Err(work_error(noema_store::StoreError::Work(
                noema_tasks::WorkDomainError::WorkflowMismatch,
            )));
        }
    }
    Ok(())
}

pub(crate) fn task_precondition(
    task_id: &str,
    revision: i64,
    generation: i64,
) -> Result<TaskPrecondition> {
    Ok(TaskPrecondition {
        task_id: parse_task_id(task_id)?,
        expected_revision: positive(revision, "expectedRevision")?,
        expected_generation: positive(generation, "expectedGeneration")?,
    })
}

pub(crate) fn command_meta(actor_id: &str, client_mutation_id: &str) -> CommandMeta {
    CommandMeta {
        actor_id: actor_id_for_principal(actor_id),
        causation_id: None,
        correlation_id: format!("correlation:graphql:{client_mutation_id}"),
        idempotency_key: Some(client_mutation_id.to_string()),
    }
}

pub(crate) fn actor_id_for_principal(principal_subject: &str) -> String {
    if principal_subject.starts_with("actor:") {
        principal_subject.to_string()
    } else {
        format!("actor:{principal_subject}")
    }
}

pub(crate) fn required_client_id(value: &str) -> Result<String> {
    if value.is_empty() || value.trim() != value {
        Err(invalid_input_error("clientMutationId"))
    } else {
        Ok(value.to_string())
    }
}

pub(crate) fn positive(value: i64, field: &'static str) -> Result<u64> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_input_error(field))
}

pub(crate) fn page_size(value: Option<i32>) -> Result<WorkPageSize> {
    let value = u32::try_from(value.unwrap_or(50))
        .map_err(|_| cursor_error(noema_store::WorkCursorError))?;
    WorkPageSize::new(value).map_err(cursor_error)
}

pub(crate) fn event_cursor(sequence: u64) -> Result<String> {
    WorkEventCursor::new(sequence)
        .map(|cursor| cursor.encode())
        .map_err(cursor_error)
}

pub(crate) fn parse_task_id(value: &str) -> Result<TaskId> {
    TaskId::new(value.trim()).map_err(|_| invalid_input_error("taskId"))
}

pub(crate) fn parse_project_id(value: &str) -> Result<ProjectId> {
    ProjectId::new(value.trim()).map_err(|_| invalid_input_error("projectId"))
}

pub(crate) fn parse_workspace_id(value: &str) -> Result<WorkspaceId> {
    WorkspaceId::new(value.trim()).map_err(|_| invalid_input_error("workspaceId"))
}

/// The first Work release exposes only the seeded Personal workspace. Keep
/// this gate in the API layer until membership-aware workspace reads exist in
/// the Store surface; callers still carry an opaque workspace id.
pub(crate) fn require_personal_workspace(workspace_id: &WorkspaceId) -> Result<()> {
    if workspace_id.as_str() == "workspace:personal" {
        Ok(())
    } else {
        Err(unavailable())
    }
}

/// Authorize an existing project without exposing a foreign or missing id.
/// The Store lookup is exact, workspace-scoped, and includes archived rows so
/// authorization remains one bounded read regardless of project count.
pub(crate) async fn require_personal_project(
    store: &noema_store::NoemaStore,
    project_id: &ProjectId,
) -> Result<()> {
    let workspace_id =
        WorkspaceId::new("workspace:personal").expect("the seeded Personal workspace id is valid");
    if store
        .get_work_project(&workspace_id, project_id)
        .await
        .map_err(work_error)?
        .is_some()
    {
        Ok(())
    } else {
        Err(unavailable())
    }
}

/// Authorize an existing task before a semantic mutation is built.
pub(crate) async fn require_personal_task(
    store: &noema_store::NoemaStore,
    task_id: &TaskId,
) -> Result<()> {
    let detail = store
        .get_work_task(task_id)
        .await
        .map_err(work_error)?
        .ok_or_else(unavailable)?;
    require_personal_workspace(&detail.workspace.workspace_id)
}

pub(crate) fn require_owner(principal_subject: &str) -> Result<()> {
    if principal_subject == "human:local" {
        Ok(())
    } else {
        Err(unavailable())
    }
}

pub(crate) fn unavailable() -> Error {
    safe_error("work_unavailable", "work is unavailable")
}

pub(crate) fn invalid_input_error(field: &str) -> Error {
    safe_error("invalid_input", format!("invalid {field}"))
}

pub(crate) fn cursor_error(_: noema_store::WorkCursorError) -> Error {
    safe_error("invalid_cursor", "invalid work cursor")
}

pub(crate) fn work_error(error: StoreError) -> Error {
    match error {
        StoreError::Work(noema_tasks::WorkDomainError::InvalidInput { message, .. })
            if message == "invalid_cursor" =>
        {
            safe_error("invalid_cursor", "invalid work cursor")
        }
        StoreError::Work(error) => safe_error(error.code(), safe_message(error.code())),
        StoreError::InvariantViolation { .. } => unavailable(),
        StoreError::Sqlite(_) | StoreError::Json(_) => unavailable(),
        _ => unavailable(),
    }
}

pub(crate) fn safe_error(code: &'static str, message: impl Into<String>) -> Error {
    Error::new(message).extend_with(|_, extensions| extensions.set("code", code))
}

pub(crate) fn safe_message(code: &str) -> &'static str {
    match code {
        "stale_revision" => "the authoritative project or task revision is stale",
        "stale_generation" => "the authoritative task generation is stale",
        "invalid_transition" => "the requested work action is not valid now",
        "workflow_mismatch" => "the requested workflow filter is inconsistent",
        "project_archived" => "the selected project is archived",
        "contract_required" => "a complete task contract is required",
        "contract_immutable" => "task contracts are immutable",
        "gate_required" => "the current open gate is required",
        "gate_unresolved" => "the task gate must be resolved first",
        "review_not_approved" => "the latest review does not permit this action",
        "review_limit_reached" => "the review limit has been reached",
        "configuration_unavailable" => "task execution configuration is unavailable",
        "run_fenced" => "the run is fenced",
        "idempotency_conflict" => "the idempotency key conflicts with a prior command",
        "invalid_input" => "the work input is invalid",
        _ => "work is unavailable",
    }
}
