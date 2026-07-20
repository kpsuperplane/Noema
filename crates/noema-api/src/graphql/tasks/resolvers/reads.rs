use async_graphql::Result;
use noema_store::{
    ProjectQuery, WorkEventBeforeQuery, WorkEventCursor, WorkOverviewQuery, WorkRunItemCursor,
    WorkRunItemOwnerScope, WorkRunItemQuery, WorkTaskCursor, WorkTaskQuery,
};
use noema_tasks::WorkflowStageId;

use super::*;
use crate::graphql::schema::GraphqlState;
use crate::graphql::tasks::*;

/// Resolve one owner-authorized Work task detail.
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
        .map_err(work_error)?
        .ok_or_else(unavailable)?;
    require_personal_workspace(&detail.workspace.workspace_id)?;
    detail_from_store(detail)
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
        .map_err(work_error)
        .and_then(project_connection)
}

/// Resolve the transactionally coherent board bootstrap.
pub(in crate::graphql) async fn work_overview(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
) -> Result<GraphqlWorkOverview> {
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
        .map_err(work_error)
        .and_then(overview_from_store)
}

/// Resolve a bounded board/list task connection using Store batch hydration.
pub(in crate::graphql) async fn work_tasks(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlWorkTasksInput,
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
            text: input.text,
            attention_only: input.attention_only,
            scope: input.scope.into(),
            first: page_size(first)?,
            after,
        })
        .await
        .map_err(work_error)
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
    let input = GraphqlWorkTasksInput {
        workspace_id,
        project_id,
        stage_ids: None,
        stage_behaviors: None,
        text: None,
        attention_only: true,
        scope: GraphqlWorkTaskScope::Active,
    };
    let tasks = work_tasks(state, principal_subject, input, first, after).await?;
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

/// Resolve newest-first activity for a workspace/project/task scope.
pub(in crate::graphql) async fn work_activity(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
    task_id: Option<String>,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlWorkEventConnection> {
    require_owner(principal_subject)?;
    let workspace_id = parse_workspace_id(&workspace_id)?;
    require_personal_workspace(&workspace_id)?;
    let project_id = project_id.as_deref().map(parse_project_id).transpose()?;
    let task_id = task_id.as_deref().map(parse_task_id).transpose()?;
    if let Some(project_id) = project_id.as_ref() {
        require_personal_project(state.store()?, project_id).await?;
    }
    if let Some(task_id) = task_id.as_ref() {
        let detail = state
            .store()?
            .get_work_task(task_id)
            .await
            .map_err(work_error)?
            .ok_or_else(unavailable)?;
        if detail.workspace.workspace_id != workspace_id {
            return Err(unavailable());
        }
        require_personal_workspace(&detail.workspace.workspace_id)?;
    }
    let before = after
        .as_deref()
        .map(WorkEventCursor::decode)
        .transpose()
        .map_err(cursor_error)?;
    state
        .store()?
        .list_work_events_before(WorkEventBeforeQuery {
            workspace_id,
            project_id,
            task_id,
            run_id: None,
            before,
            first: page_size(first)?,
        })
        .await
        .map_err(work_error)
        .and_then(event_connection)
}

/// Resolve archived task history with a stable kind filter.
#[allow(
    clippy::too_many_arguments,
    reason = "mirrors the GraphQL field contract"
)]
pub(in crate::graphql) async fn archive_tasks(
    state: &GraphqlState,
    principal_subject: &str,
    workspace_id: String,
    project_id: Option<String>,
    kind: GraphqlTerminalTaskKind,
    text: Option<String>,
    first: Option<i32>,
    after: Option<String>,
) -> Result<GraphqlTaskConnection> {
    require_owner(principal_subject)?;
    let stage_behaviors = match kind {
        GraphqlTerminalTaskKind::Accepted => {
            vec![noema_tasks::WorkflowStageBehavior::TerminalSuccess]
        }
        GraphqlTerminalTaskKind::Cancelled => {
            vec![noema_tasks::WorkflowStageBehavior::TerminalCancelled]
        }
        GraphqlTerminalTaskKind::All => Vec::new(),
    };
    work_tasks(
        state,
        principal_subject,
        GraphqlWorkTasksInput {
            workspace_id,
            project_id,
            stage_ids: None,
            stage_behaviors: Some(stage_behaviors.into_iter().map(Into::into).collect()),
            text,
            attention_only: false,
            scope: GraphqlWorkTaskScope::Terminal,
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
        .map_err(work_error)
}
