use noema_store::{
    ProjectConnection, WorkConnection, WorkEdge, WorkEventConnection, WorkOverview,
    WorkRunItemConnection, WorkTaskConnection, WorkTaskDetail, WorkTaskSummary,
};

use crate::graphql::tasks::unavailable;

use super::*;

/// Build a bounded task summary from a Store projection.
pub(crate) fn summary_from_store(
    value: WorkTaskSummary,
) -> async_graphql::Result<GraphqlTaskSummary> {
    let task = task_card_from_store(&value)?;
    let attention = attention_projection(
        task.clone(),
        value.attention,
        value.active_gate.as_ref(),
        &value.valid_actions,
    )?;
    Ok(GraphqlTaskSummary { task, attention })
}

fn task_card_from_store(value: &WorkTaskSummary) -> async_graphql::Result<GraphqlTaskCard> {
    let (effective_cwd, effective_cwd_source) = task_cwd(
        value.task.cwd_override.as_deref(),
        value
            .project
            .as_ref()
            .and_then(|project| project.folder.as_deref()),
        &value.task.task_directory,
    );
    Ok(GraphqlTaskCard {
        task_id: value.task.task_id.to_string(),
        workspace: value.workspace.clone().into(),
        project: value.project.clone().map(TryInto::try_into).transpose()?,
        title: value.task.title.clone(),
        task_document_preview: value.task_document_preview.clone(),
        stage: value.stage.clone().into(),
        revision: exact_u64(value.task.revision)?,
        generation: exact_u64(value.task.generation)?,
        executor_agent_id: value.task.executor_agent_id.clone(),
        executor_backend: task_executor_backend(&value.task.executor_agent_id),
        cwd_override: value.task.cwd_override.clone(),
        effective_cwd,
        effective_cwd_source,
        created_at: value.task.created_at.clone(),
        updated_at: value.task.updated_at.clone(),
        schedule: task_schedule(&value.task)?,
        completed_at: value.task.completed_at.clone(),
        current_run: value.current_run.clone().map(Into::into),
        active_gate: value
            .active_gate
            .clone()
            .map(TryInto::try_into)
            .transpose()?,
        valid_actions: value
            .valid_actions
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
    })
}

pub(crate) fn task_card_from_detail(
    value: &WorkTaskDetail,
) -> async_graphql::Result<GraphqlTaskCard> {
    task_card_from_store(&WorkTaskSummary {
        task: value.task.clone(),
        task_document_preview: preview(&value.task_document, 280),
        workspace: value.workspace.clone(),
        project: value.project.clone(),
        stage: value.stage.clone(),
        current_run: value.current_run.clone(),
        active_gate: value.active_gate.clone(),
        attention: value.attention,
        valid_actions: value.valid_actions.clone(),
    })
}

/// Build the current task-detail projection. History fields are resolved by
/// context-backed connections on `TaskDetail`.
pub(crate) fn detail_from_store(value: WorkTaskDetail) -> async_graphql::Result<GraphqlTaskDetail> {
    let task_id = value.task.task_id.to_string();
    let task = task_card_from_detail(&value)?;
    let attention = attention_projection(
        task,
        value.attention,
        value.active_gate.as_ref(),
        &value.valid_actions,
    )?;
    let effective_cwd = Some(value.working_directory.clone());
    let effective_cwd_source = if value.task.cwd_override.is_some() {
        "task"
    } else if value.project.is_some() {
        "project"
    } else {
        "default"
    }
    .to_string();
    let current_gate = value.active_gate.map(TryInto::try_into).transpose()?;
    let current_run = value.current_run.map(Into::into);
    let messages = value.messages.into_iter().map(Into::into).collect();
    let runs = value
        .runs
        .into_iter()
        .map(TryInto::try_into)
        .collect::<async_graphql::Result<_>>()?;
    let artifacts = value
        .artifacts
        .into_iter()
        .map(|artifact| {
            crate::graphql::artifacts::graphql_artifact_from_current(
                artifact.artifact,
                artifact.current_version,
            )
        })
        .collect::<async_graphql::Result<_>>()?;
    let schedule = task_schedule(&value.task)?;
    Ok(GraphqlTaskDetail {
        task_id,
        project: value.project.map(TryInto::try_into).transpose()?,
        title: value.task.title,
        task_document: value.task_document,
        task_document_digest: value.task_document_digest,
        result_document: value.result_document,
        review_document: value.review_document,
        stage: value.stage.into(),
        revision: exact_u64(value.task.revision)?,
        generation: exact_u64(value.task.generation)?,
        executor_agent_id: value.task.executor_agent_id.clone(),
        executor_backend: task_executor_backend(&value.task.executor_agent_id),
        cwd_override: value.task.cwd_override.clone(),
        effective_cwd,
        effective_cwd_source,
        created_at: value.task.created_at,
        updated_at: value.task.updated_at,
        schedule,
        completed_at: value.task.completed_at,
        source: value.task.provenance.into(),
        current_run,
        active_gate: current_gate,
        messages,
        runs,
        contributor_instance_names: value.contributor_instance_names,
        artifacts,
        attention,
        valid_actions: value.valid_actions.into_iter().map(Into::into).collect(),
    })
}

fn task_executor_backend(agent_id: &str) -> String {
    if agent_id == noema_tasks::TASK_EXECUTOR_AGENT_ID {
        "provider".to_string()
    } else {
        "acp".to_string()
    }
}

fn task_cwd(
    task_override: Option<&str>,
    project_folder: Option<&str>,
    task_directory: &str,
) -> (Option<String>, String) {
    if let Some(value) = task_override {
        (
            Some(
                std::path::Path::new(value)
                    .join(task_directory)
                    .to_string_lossy()
                    .into_owned(),
            ),
            "task".to_string(),
        )
    } else if let Some(value) = project_folder {
        (
            Some(
                std::path::Path::new(value)
                    .join(task_directory)
                    .to_string_lossy()
                    .into_owned(),
            ),
            "project".to_string(),
        )
    } else {
        (None, "default".to_string())
    }
}

fn task_schedule(
    value: &noema_tasks::TaskRecord,
) -> async_graphql::Result<Option<GraphqlTaskSchedule>> {
    let Some(scheduled_for) = value.scheduled_for else {
        return Ok(None);
    };
    Ok(Some(GraphqlTaskSchedule {
        scheduled_for: instant(scheduled_for)?,
        time_zone: value.schedule_time_zone.clone().ok_or_else(unavailable)?,
        missed_run_policy: value.missed_run_policy.ok_or_else(unavailable)?.into(),
        recurrence_id: value.recurrence_id.as_ref().map(ToString::to_string),
        recurrence_revision: value.recurrence_revision.map(exact_u64).transpose()?,
        recurrence_scheduled_for: value.recurrence_scheduled_for.map(instant).transpose()?,
    }))
}

pub(in crate::graphql) fn instant(value: i64) -> async_graphql::Result<String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(value, 0)
        .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .ok_or_else(unavailable)
}

/// Map a Store project connection to GraphQL.
pub(crate) fn project_connection(
    value: ProjectConnection,
) -> async_graphql::Result<GraphqlProjectConnection> {
    let (edges, page_info) = map_connection(value, |edge| {
        Ok(GraphqlProjectEdge {
            cursor: edge.cursor,
            node: edge.node.try_into()?,
        })
    })?;
    Ok(GraphqlProjectConnection { edges, page_info })
}

/// Map a Store task connection to GraphQL.
pub(crate) fn task_connection(
    value: WorkTaskConnection,
) -> async_graphql::Result<GraphqlTaskConnection> {
    let (edges, page_info) = map_connection(value, |edge| {
        Ok(GraphqlTaskEdge {
            cursor: edge.cursor,
            node: summary_from_store(edge.node)?,
        })
    })?;
    Ok(GraphqlTaskConnection { edges, page_info })
}

/// Map a Store event connection to GraphQL.
pub(crate) fn event_connection(
    value: WorkEventConnection,
) -> async_graphql::Result<GraphqlTaskEventConnection> {
    Ok(GraphqlTaskEventConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                let cursor = edge.cursor.encode();
                let node = GraphqlTaskEvent::try_from(edge.node).map_err(|_| unavailable())?;
                if node.cursor != cursor {
                    return Err(unavailable());
                }
                Ok(GraphqlTaskEventEdge { cursor, node })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

fn map_connection<N, E>(
    value: WorkConnection<String, N>,
    map: impl FnMut(WorkEdge<String, N>) -> async_graphql::Result<E>,
) -> async_graphql::Result<(Vec<E>, GraphqlPageInfo)> {
    Ok((
        value
            .edges
            .into_iter()
            .map(map)
            .collect::<async_graphql::Result<_>>()?,
        value.page_info.into(),
    ))
}

pub(crate) fn run_item_connection(value: WorkRunItemConnection) -> GraphqlTaskRunItemConnection {
    GraphqlTaskRunItemConnection {
        edges: value
            .edges
            .into_iter()
            .map(|mut edge| {
                add_tool_marker(&mut edge.node);
                let cursor = edge.cursor;
                let node = (cursor.clone(), edge.node).into();
                GraphqlTaskRunItemEdge { cursor, node }
            })
            .collect(),
        page_info: value.page_info.into(),
    }
}

fn add_tool_marker(item: &mut noema_tasks::AgentRunItemRecord) {
    let action_kind = match item.kind {
        noema_tasks::AgentRunItemKind::ToolCall => "tool_call",
        noema_tasks::AgentRunItemKind::ToolResult => "tool_result",
        _ => return,
    };
    let mut action = item.payload.clone();
    let Some(action_object) = action.as_object_mut() else {
        return;
    };
    if !action_object.contains_key("name")
        && let Some(name) = item.content_text.as_deref()
    {
        action_object.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }
    let Some(marker) =
        noema_runtime::tool_marker_for_action(action_kind, item.status.as_str(), &action)
    else {
        return;
    };
    if let Some(payload) = item.payload.as_object_mut()
        && let Some(display) = payload
            .entry("display")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
    {
        display.insert("marker".to_string(), marker);
    }
}

pub(crate) fn overview_from_store(
    value: WorkOverview,
) -> async_graphql::Result<GraphqlTaskOverview> {
    let workflow = GraphqlWorkflow::from_parts(
        value.default_workflow.workflow,
        value.default_workflow.stages.clone(),
    );
    let board_columns = value
        .board_stage_counts
        .into_iter()
        .map(|count| -> async_graphql::Result<_> {
            Ok(GraphqlTaskStageColumn {
                stage: count.stage.into(),
                task_count: exact_u64(count.task_count)?,
            })
        })
        .collect::<async_graphql::Result<_>>()?;
    Ok(GraphqlTaskOverview {
        workspace: value.workspace.into(),
        workflow,
        board_columns,
        recent_tasks: task_connection(value.recent_tasks)?,
        needs_you_count: exact_u64(value.needs_you_count)?,
    })
}

fn attention_projection(
    task: GraphqlTaskCard,
    attention: Option<noema_store::WorkTaskAttention>,
    gate: Option<&noema_tasks::TaskGateRecord>,
    actions: &[noema_store::WorkTaskValidAction],
) -> async_graphql::Result<Option<GraphqlTaskAttention>> {
    let Some(attention) = attention else {
        return Ok(None);
    };
    let (title, summary) = match attention {
        noema_store::WorkTaskAttention::Clarification => (
            "Clarification required",
            gate.ok_or_else(unavailable)?.prompt_markdown.clone(),
        ),
        noema_store::WorkTaskAttention::Approval => (
            "Approval required",
            gate.ok_or_else(unavailable)?.prompt_markdown.clone(),
        ),
        noema_store::WorkTaskAttention::Recovery => (
            "Recovery decision required",
            gate.ok_or_else(unavailable)?.prompt_markdown.clone(),
        ),
    };
    Ok(Some(GraphqlTaskAttention {
        kind: attention.into(),
        title: title.to_string(),
        summary: preview(&summary, 400),
        gate: gate.cloned().map(TryInto::try_into).transpose()?,
        task,
        valid_actions: actions.iter().copied().map(Into::into).collect(),
    }))
}

pub(super) fn preview(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub(in crate::graphql) fn exact_u64(value: u64) -> async_graphql::Result<i64> {
    i64::try_from(value).map_err(|_| unavailable())
}

#[cfg(test)]
mod tests {
    use super::exact_u64;

    #[test]
    fn authoritative_u64_projection_fails_instead_of_saturating() {
        assert_eq!(exact_u64(i64::MAX as u64).expect("i64 maximum"), i64::MAX);
        assert!(exact_u64(i64::MAX as u64 + 1).is_err());
        assert!(exact_u64(u64::MAX).is_err());
    }
}
