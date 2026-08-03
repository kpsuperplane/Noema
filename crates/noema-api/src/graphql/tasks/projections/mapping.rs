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
    Ok(GraphqlTaskCard {
        task_id: value.task.task_id.to_string(),
        workspace: value.workspace.clone().into(),
        project: value.project.clone().map(TryInto::try_into).transpose()?,
        title: value.task.title.clone(),
        description_preview: preview(&value.task.description_markdown, 280),
        stage: value.stage.clone().into(),
        revision: exact_u64(value.task.revision)?,
        generation: exact_u64(value.task.generation)?,
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
        latest_review: value.latest_review.clone().map(Into::into),
        valid_actions: value
            .valid_actions
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
    })
}

/// Build the current task-detail projection. History fields are resolved by
/// context-backed connections on `TaskDetail`.
pub(crate) fn detail_from_store(value: WorkTaskDetail) -> async_graphql::Result<GraphqlTaskDetail> {
    let task_id = value.task.task_id.to_string();
    let summary = WorkTaskSummary {
        task: value.task.clone(),
        workspace: value.workspace.clone(),
        project: value.project.clone(),
        stage: value.stage.clone(),
        current_run: value.current_run.clone(),
        active_gate: value.active_gate.clone(),
        latest_review: value.latest_review.clone(),
        attention: value.attention,
        valid_actions: value.valid_actions.clone(),
    };
    let task = task_card_from_store(&summary)?;
    let attention = attention_projection(
        task,
        value.attention,
        value.active_gate.as_ref(),
        &value.valid_actions,
    )?;
    let current_contract = value.current_contract.map(TryInto::try_into).transpose()?;
    let current_gate = value.active_gate.map(TryInto::try_into).transpose()?;
    let current_run = value.current_run.map(Into::into);
    let latest_submission = value.latest_submission.map(Into::into);
    let completed_result = value.completed_submission.map(Into::into);
    let latest_review = value.latest_review.map(Into::into);
    let messages = value.messages.into_iter().map(Into::into).collect();
    let runs = value
        .runs
        .into_iter()
        .map(TryInto::try_into)
        .collect::<async_graphql::Result<_>>()?;
    let submissions = value.submissions.into_iter().map(Into::into).collect();
    let reviews = value.reviews.into_iter().map(Into::into).collect();
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
        description: value.task.description_markdown,
        stage: value.stage.into(),
        revision: exact_u64(value.task.revision)?,
        generation: exact_u64(value.task.generation)?,
        created_at: value.task.created_at,
        updated_at: value.task.updated_at,
        schedule,
        completed_at: value.task.completed_at,
        source: value.task.provenance.into(),
        current_contract,
        current_run,
        active_gate: current_gate,
        latest_submission,
        completed_result,
        latest_review,
        messages,
        runs,
        submissions,
        reviews,
        artifacts,
        attention,
        valid_actions: value.valid_actions.into_iter().map(Into::into).collect(),
    })
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
) -> async_graphql::Result<GraphqlWorkEventConnection> {
    Ok(GraphqlWorkEventConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                let cursor = edge.cursor.encode();
                let node = GraphqlWorkEvent::try_from(edge.node).map_err(|_| unavailable())?;
                if node.cursor != cursor {
                    return Err(unavailable());
                }
                Ok(GraphqlWorkEventEdge { cursor, node })
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
            .map(|edge| {
                let cursor = edge.cursor;
                let node = (cursor.clone(), edge.node).into();
                GraphqlTaskRunItemEdge { cursor, node }
            })
            .collect(),
        page_info: value.page_info.into(),
    }
}

pub(crate) fn overview_from_store(
    value: WorkOverview,
) -> async_graphql::Result<GraphqlWorkOverview> {
    let workflow = GraphqlWorkflow::from_parts(
        value.default_workflow.workflow,
        value.default_workflow.stages.clone(),
    );
    let board_columns = value
        .board_stage_counts
        .into_iter()
        .map(|count| -> async_graphql::Result<_> {
            Ok(GraphqlWorkStageColumn {
                stage: count.stage.into(),
                task_count: exact_u64(count.task_count)?,
            })
        })
        .collect::<async_graphql::Result<_>>()?;
    Ok(GraphqlWorkOverview {
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
        review: None,
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
