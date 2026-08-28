use std::{
    collections::VecDeque,
    path::{Component, Path},
};

use async_graphql::Result;
use noema_store::{
    NoemaStore, ProjectQuery, WorkOverviewQuery, WorkRunItemCursor, WorkRunItemOwnerScope,
    WorkRunItemQuery, WorkTaskCursor, WorkTaskQuery,
};
use noema_tasks::{TaskId, WorkflowStageId};

use super::*;
use crate::graphql::schema::GraphqlState;
use crate::graphql::tasks::*;

const TASK_WORKSPACE_FILE_LIMIT: usize = 500;

pub(in crate::graphql) async fn task_schedule_preview(
    principal_subject: &str,
    input: GraphqlTaskSchedulePreviewInput,
) -> Result<GraphqlTaskSchedulePreview> {
    require_owner(principal_subject)?;
    let starts_at = noema_tasks::parse_utc_instant(&input.starts_at, "startsAt")
        .map_err(|_| invalid_input_error("startsAt"))?;
    let occurrences = if let Some(cron) = input.cron_expression {
        noema_tasks::recurrence_preview(&cron, &input.time_zone, starts_at)
            .map_err(|_| invalid_input_error("schedule"))?
    } else {
        noema_tasks::recurrence_preview("0 0 * * *", &input.time_zone, starts_at)
            .map_err(|_| invalid_input_error("timeZone"))?;
        vec![starts_at]
    };
    Ok(GraphqlTaskSchedulePreview {
        resolved_start: instant(starts_at)?,
        occurrences: occurrences
            .into_iter()
            .map(instant)
            .collect::<Result<_>>()?,
    })
}

pub(in crate::graphql) async fn task_recurrence(
    state: &GraphqlState,
    principal_subject: &str,
    recurrence_id: String,
    first: Option<i32>,
) -> Result<GraphqlTaskRecurrence> {
    require_owner(principal_subject)?;
    let recurrence_id = noema_tasks::TaskRecurrenceId::new(recurrence_id)
        .map_err(|_| invalid_input_error("recurrenceId"))?;
    let recurrence = state
        .store()?
        .get_task_recurrence(&recurrence_id)
        .await
        .map_err(task_error)?
        .ok_or_else(unavailable)?;
    require_personal_workspace(&recurrence.workspace_id)?;
    let first = usize::try_from(first.unwrap_or(30).clamp(1, 100)).map_err(|_| unavailable())?;
    let occurrences = state
        .store()?
        .list_task_recurrence_occurrences(&recurrence_id, first)
        .await
        .map_err(task_error)?
        .into_iter()
        .map(|value| -> Result<_> {
            Ok(GraphqlRecurrenceOccurrence {
                recurrence_revision: exact_u64(value.recurrence_revision)?,
                scheduled_for: instant(value.scheduled_for)?,
                local_slot: value.local_slot,
                trigger: value.trigger.into(),
                resolution: value.resolution.into(),
                task_id: value.task_id.map(|id| id.to_string()),
                created_at: value.created_at,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let document = state
        .store()?
        .read_recurrence_document(&recurrence_id)
        .await
        .map_err(|_| unavailable())?;
    Ok(GraphqlTaskRecurrence {
        recurrence_id: recurrence.recurrence_id.to_string(),
        title: recurrence.title,
        task_document: document.content,
        task_document_digest: document.digest,
        starts_at: instant(recurrence.starts_at)?,
        cron_expression: recurrence.cron_expression,
        time_zone: recurrence.time_zone,
        missed_run_policy: recurrence.missed_run_policy.into(),
        overlap_policy: recurrence.overlap_policy.into(),
        lifecycle: recurrence.lifecycle.into(),
        revision: exact_u64(recurrence.revision)?,
        next_run_at: recurrence.next_run_at.map(instant).transpose()?,
        pending_coalesced_at: recurrence.pending_coalesced_at.map(instant).transpose()?,
        occurrences,
    })
}

/// Resolve current recurring authorities for one Tasks list scope.
pub(in crate::graphql) async fn task_recurrences(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
    first: Option<i32>,
) -> Result<Vec<GraphqlTaskRecurrenceSummary>> {
    require_owner(principal_subject)?;
    let workspace_id = parse_workspace_id(&workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let project_id = project_id.as_deref().map(parse_project_id).transpose()?;
    if let Some(project_id) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    state
        .store()?
        .list_task_recurrences(&workspace_id, project_id.as_ref(), page_size(first)?)
        .await
        .map_err(task_error)?
        .into_iter()
        .map(|recurrence| {
            Ok(GraphqlTaskRecurrenceSummary {
                recurrence_id: recurrence.recurrence_id.into_string(),
                title: recurrence.title,
                cron_expression: recurrence.cron_expression,
                lifecycle: recurrence.lifecycle.into(),
                next_run_at: recurrence.next_run_at.map(instant).transpose()?,
                updated_at: recurrence.updated_at,
            })
        })
        .collect()
}

/// Resolve one owner-authorized task detail.
pub(in crate::graphql) async fn task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<GraphqlTaskDetail> {
    require_owner(principal_subject)?;
    let task_id = parse_task_id(&task_id)?;
    let store = state.store()?;
    let detail = store
        .get_work_task(&task_id)
        .await
        .map_err(task_error)?
        .ok_or_else(unavailable)?;
    require_personal_workspace(&detail.workspace.workspace_id)?;
    let (workspace_files, workspace_files_truncated) =
        task_workspace_files(store, &task_id).await?;
    let mut detail = detail_from_store(detail)?;
    detail.workspace_files = workspace_files;
    detail.workspace_files_truncated = workspace_files_truncated;
    Ok(detail)
}

/// Resolve one owner-authorized bounded UTF-8 Task workspace file.
pub(in crate::graphql) async fn task_workspace_file(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
    path: String,
) -> Result<GraphqlTaskWorkspaceFileText> {
    require_owner(principal_subject)?;
    let task_id = parse_task_id(&task_id)?;
    if path.is_empty()
        || Path::new(&path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(unavailable());
    }
    require_personal_task(state.store()?, &task_id).await?;
    let content = state
        .store()?
        .read_task_file(&task_id, &path)
        .await
        .map_err(|_| unavailable())?;
    Ok(GraphqlTaskWorkspaceFileText { path, content })
}

async fn task_workspace_files(
    store: &NoemaStore,
    task_id: &TaskId,
) -> Result<(Vec<GraphqlTaskWorkspaceFile>, bool)> {
    let mut directories = VecDeque::from([".".to_string()]);
    let mut files = Vec::new();
    while let Some(directory) = directories.pop_front() {
        for entry in store
            .list_task_files(task_id, &directory)
            .await
            .map_err(|_| unavailable())?
        {
            if files.len() == TASK_WORKSPACE_FILE_LIMIT {
                return Ok((files, true));
            }
            if entry.is_directory {
                directories.push_back(entry.path.clone());
            }
            files.push(GraphqlTaskWorkspaceFile {
                path: entry.path,
                is_directory: entry.is_directory,
                size_bytes: entry.size_bytes.map(exact_u64).transpose()?,
            });
        }
    }
    Ok((files, false))
}

/// Resolve one owner-authorized task card for an embedded intervention.
pub(in crate::graphql) async fn task_card(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<GraphqlTaskCard> {
    require_owner(principal_subject)?;
    let task_id = parse_task_id(&task_id)?;
    let detail = state
        .store()?
        .get_work_task(&task_id)
        .await
        .map_err(task_error)?
        .ok_or_else(unavailable)?;
    require_personal_workspace(&detail.workspace.workspace_id)?;
    task_card_from_detail(&detail)
}

/// Resolve one owner-authorized task summary for a transcript reference.
pub(in crate::graphql) async fn task_summary(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<Option<GraphqlTaskSummary>> {
    require_owner(principal_subject)?;
    let task_id = parse_task_id(&task_id)?;
    let Some(summary) = state
        .store()?
        .get_work_task_summary(&task_id)
        .await
        .map_err(task_error)?
    else {
        return Ok(None);
    };
    require_personal_workspace(&summary.workspace.workspace_id)?;
    summary_from_store(summary).map(Some)
}

/// Resolve a bounded project connection.
pub(in crate::graphql) async fn projects(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    include_archived: bool,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlProjectConnection> {
    require_owner(principal_subject)?;
    let workspace_id = parse_workspace_id(&workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let page_size = page_size(first)?;
    let after = after
        .as_deref()
        .map(noema_store::ProjectCursor::decode)
        .transpose()
        .map_err(cursor_error)?;
    state
        .store()?
        .list_work_projects(ProjectQuery {
            workspace_id,
            include_archived,
            first: page_size,
            after,
        })
        .await
        .map_err(task_error)
        .and_then(project_connection)
}

/// Read one exact project document.
pub(in crate::graphql) async fn project_document(
    state: &GraphqlState,
    principal_subject: &str,
    project_id: String,
) -> Result<GraphqlProjectDocument> {
    require_owner(principal_subject)?;
    let project_id = parse_project_id(&project_id)?;
    require_personal_project(state.store()?, &project_id).await?;
    let document = state
        .store()?
        .read_project_document(&project_id)
        .await
        .map_err(|_| unavailable())?;
    Ok(GraphqlProjectDocument {
        project_id: project_id.into_string(),
        content: document.content,
        digest: document.digest,
    })
}

/// Resolve the transactionally coherent board bootstrap.
pub(in crate::graphql) async fn tasks_overview(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
) -> Result<GraphqlTaskOverview> {
    require_owner(principal_subject)?;
    let workspace_id = parse_workspace_id(&workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let project_id = project_id.as_deref().map(parse_project_id).transpose()?;
    if let Some(project_id) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    state
        .store()?
        .work_overview(WorkOverviewQuery {
            workspace_id,
            project_id,
            first: page_size(None)?,
        })
        .await
        .map_err(task_error)
        .and_then(overview_from_store)
}

/// Resolve a bounded board/list task connection using Store batch hydration.
pub(in crate::graphql) async fn task_list(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskListInput,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlTaskConnection> {
    require_owner(principal_subject)?;
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let project_id = input
        .project_id
        .as_deref()
        .map(parse_project_id)
        .transpose()?;
    if let Some(project_id) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    let stage_ids = input
        .stage_ids
        .unwrap_or_default()
        .into_iter()
        .map(|id| WorkflowStageId::new(id).map_err(|_| invalid_input_error("stage id")))
        .collect::<Result<Vec<_>>>()?;
    let stage_behaviors = input
        .stage_behaviors
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();
    validate_scope_filters(
        state.store()?,
        &workspace_id,
        input.scope,
        &stage_ids,
        &stage_behaviors,
    )
    .await?;
    let after = after
        .as_deref()
        .map(WorkTaskCursor::decode)
        .transpose()
        .map_err(cursor_error)?;
    state
        .store()?
        .list_work_tasks(WorkTaskQuery {
            workspace_id,
            project_id,
            stage_ids,
            stage_behaviors,
            attention_only: input.attention_only,
            scope: input.scope.into(),
            first: page_size(first)?,
            after,
        })
        .await
        .map_err(task_error)
        .and_then(task_connection)
}

/// Resolve derived Needs You cards from Store attention projections.
pub(in crate::graphql) async fn needs_you(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlTaskAttentionConnection> {
    require_owner(principal_subject)?;
    let input = GraphqlTaskListInput {
        workspace_id,
        project_id,
        stage_ids: None,
        stage_behaviors: None,
        attention_only: true,
        scope: GraphqlTaskScope::Active,
    };
    let tasks = task_list(state, principal_subject, input, first, after).await?;
    let edges = tasks
        .edges
        .into_iter()
        .map(|edge| {
            Ok(GraphqlTaskAttentionEdge {
                cursor: edge.cursor,
                node: edge.node.attention.ok_or_else(unavailable)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(GraphqlTaskAttentionConnection {
        edges,
        page_info: tasks.page_info,
    })
}

/// Resolve Done and Cancelled task history with a stable kind filter.
#[allow(
    clippy::too_many_arguments,
    reason = "mirrors the GraphQL field contract"
)]
pub(in crate::graphql) async fn task_history(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
    kind: GraphqlTerminalTaskKind,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlTaskConnection> {
    require_owner(principal_subject)?;
    let stage_behaviors = match kind {
        GraphqlTerminalTaskKind::Completed => {
            vec![noema_tasks::WorkflowStageBehavior::TerminalSuccess]
        }
        GraphqlTerminalTaskKind::Cancelled => {
            vec![noema_tasks::WorkflowStageBehavior::TerminalCancelled]
        }
        GraphqlTerminalTaskKind::All => Vec::new(),
    };
    task_list(
        state,
        principal_subject,
        GraphqlTaskListInput {
            workspace_id,
            project_id,
            stage_ids: None,
            stage_behaviors: Some(stage_behaviors.into_iter().map(Into::into).collect()),
            attention_only: false,
            scope: GraphqlTaskScope::Terminal,
        },
        first,
        after,
    )
    .await
}

/// Return a bounded transcript page for one owner-authorized run.
pub(in crate::graphql) async fn task_run_items(
    state: &GraphqlState,
    principal_subject: &str,
    run_id: String,
    after: Option<String>,
    first: Option<i32>,
) -> Result<GraphqlTaskRunItemConnection> {
    require_owner(principal_subject)?;
    let store = state.store()?;
    let before = after
        .as_deref()
        .map(WorkRunItemCursor::decode)
        .transpose()
        .map_err(cursor_error)?;
    store
        .list_work_run_items(WorkRunItemQuery {
            owner: WorkRunItemOwnerScope {
                workspace_id: parse_workspace_id("workspace:personal")?,
                task_id: None,
            },
            run_id: run_id.trim().to_string(),
            first: page_size(first)?,
            before,
        })
        .await
        .map(run_item_connection)
        .map_err(task_error)
}
