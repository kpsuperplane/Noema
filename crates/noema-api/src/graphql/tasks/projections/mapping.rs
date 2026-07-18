use noema_store::{
    ProjectConnection, WorkContractConnection, WorkEventConnection, WorkGateConnection,
    WorkMessageConnection, WorkOverview, WorkReviewConnection, WorkRunConnection,
    WorkRunItemConnection, WorkSubmissionConnection, WorkTaskArtifactConnection,
    WorkTaskConnection, WorkTaskDetail, WorkTaskSummary,
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
        value.latest_review.as_ref(),
        &value.valid_actions,
    )?;
    Ok(GraphqlTaskSummary {
        task_id: task.task_id,
        workspace: task.workspace,
        project: task.project,
        title: task.title,
        description_preview: task.description_preview,
        stage: task.stage,
        revision: task.revision,
        generation: task.generation,
        created_at: task.created_at,
        updated_at: task.updated_at,
        completed_at: task.completed_at,
        current_run: task.current_run,
        active_gate: task.active_gate,
        latest_review: task.latest_review,
        attention,
        valid_actions: task.valid_actions,
    })
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
        value.latest_review.as_ref(),
        &value.valid_actions,
    )?;
    let current_contract = value.current_contract.map(TryInto::try_into).transpose()?;
    let current_gate = value.active_gate.map(TryInto::try_into).transpose()?;
    let current_run = value.current_run.map(Into::into);
    let latest_submission = value.latest_submission.map(Into::into);
    let accepted_result = value.accepted_submission.map(Into::into);
    let latest_review = value.latest_review.map(Into::into);
    let workflow = GraphqlWorkflow::from_parts(value.workflow, value.workflow_stages);
    Ok(GraphqlTaskDetail {
        task_id,
        workspace: value.workspace.into(),
        project: value.project.map(TryInto::try_into).transpose()?,
        title: value.task.title,
        description_preview: preview(&value.task.description_markdown, 280),
        description: value.task.description_markdown,
        stage: value.stage.into(),
        revision: exact_u64(value.task.revision)?,
        generation: exact_u64(value.task.generation)?,
        created_at: value.task.created_at,
        updated_at: value.task.updated_at,
        completed_at: value.task.completed_at,
        source: GraphqlTaskSource {
            source_kind: value.task.provenance.source_kind.into(),
            conversation_id: value.task.provenance.conversation_id,
            turn_id: value.task.provenance.turn_id,
            item_id: value.task.provenance.item_id,
        },
        workflow,
        current_contract,
        current_run,
        active_gate: current_gate,
        latest_submission,
        accepted_result,
        latest_review,
        attention,
        valid_actions: value.valid_actions.into_iter().map(Into::into).collect(),
    })
}

/// Map a Store project connection to GraphQL.
pub(crate) fn project_connection(
    value: ProjectConnection,
) -> async_graphql::Result<GraphqlProjectConnection> {
    Ok(GraphqlProjectConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlProjectEdge {
                    cursor: edge.cursor,
                    node: edge.node.try_into()?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

/// Map a Store task connection to GraphQL.
pub(crate) fn task_connection(
    value: WorkTaskConnection,
) -> async_graphql::Result<GraphqlTaskConnection> {
    Ok(GraphqlTaskConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskEdge {
                    cursor: edge.cursor,
                    node: summary_from_store(edge.node)?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
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

pub(crate) fn contract_connection(
    value: WorkContractConnection,
) -> async_graphql::Result<GraphqlTaskExecutionContractConnection> {
    Ok(GraphqlTaskExecutionContractConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskExecutionContractEdge {
                    cursor: edge.cursor,
                    node: edge.node.try_into()?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

pub(crate) fn gate_connection(
    value: WorkGateConnection,
) -> async_graphql::Result<GraphqlTaskGateConnection> {
    Ok(GraphqlTaskGateConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskGateEdge {
                    cursor: edge.cursor,
                    node: edge.node.try_into()?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

pub(crate) fn message_connection(
    value: WorkMessageConnection,
) -> async_graphql::Result<GraphqlTaskMessageConnection> {
    Ok(GraphqlTaskMessageConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskMessageEdge {
                    cursor: edge.cursor,
                    node: GraphqlTaskMessage {
                        message_id: edge.node.message_id.into_string(),
                        task_generation: exact_u64(edge.node.task_generation)?,
                        kind: edge.node.kind.into(),
                        body_markdown: edge.node.body_markdown,
                        author: edge.node.author_actor_id,
                        gate_id: edge.node.gate_id.map(|id| id.into_string()),
                        contract_id: edge.node.contract_id.map(|id| id.into_string()),
                        approval_decision: edge.node.approval_decision.map(Into::into),
                        consumed_by_run_id: edge.node.consumed_by_run_id,
                        consumed_at: edge.node.consumed_at,
                        created_at: edge.node.created_at,
                    },
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

pub(crate) fn run_connection(
    value: WorkRunConnection,
) -> async_graphql::Result<GraphqlTaskRunConnection> {
    Ok(GraphqlTaskRunConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskRunEdge {
                    cursor: edge.cursor,
                    node: edge.node.try_into()?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

pub(crate) fn submission_connection(
    value: WorkSubmissionConnection,
) -> GraphqlTaskSubmissionConnection {
    GraphqlTaskSubmissionConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| GraphqlTaskSubmissionEdge {
                cursor: edge.cursor,
                node: edge.node.into(),
            })
            .collect(),
        page_info: value.page_info.into(),
    }
}

pub(crate) fn review_connection(value: WorkReviewConnection) -> GraphqlTaskReviewConnection {
    GraphqlTaskReviewConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| GraphqlTaskReviewEdge {
                cursor: edge.cursor,
                node: edge.node.into(),
            })
            .collect(),
        page_info: value.page_info.into(),
    }
}

pub(crate) fn artifact_connection(
    value: WorkTaskArtifactConnection,
) -> async_graphql::Result<GraphqlTaskArtifactConnection> {
    Ok(GraphqlTaskArtifactConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| -> async_graphql::Result<_> {
                Ok(GraphqlTaskArtifactEdge {
                    cursor: edge.cursor,
                    node: crate::graphql::artifacts::graphql_artifact_from_current(
                        edge.node.artifact,
                        edge.node.current_version,
                    )?,
                })
            })
            .collect::<async_graphql::Result<_>>()?,
        page_info: value.page_info.into(),
    })
}

pub(crate) fn run_item_connection(value: WorkRunItemConnection) -> GraphqlTaskRunItemConnection {
    GraphqlTaskRunItemConnection {
        edges: value
            .edges
            .into_iter()
            .map(|edge| {
                let cursor = edge.cursor;
                let node = GraphqlTaskRunItem::from_edge(cursor.clone(), edge.node);
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
    let active_columns = value
        .active_stage_counts
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
        active_columns,
        recent_tasks: task_connection(value.recent_tasks)?,
        needs_you_count: exact_u64(value.needs_you_count)?,
    })
}

fn attention_projection(
    task: GraphqlTaskCard,
    attention: Option<noema_store::WorkTaskAttention>,
    gate: Option<&noema_tasks::TaskGateRecord>,
    review: Option<&noema_tasks::TaskReviewRecord>,
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
        noema_store::WorkTaskAttention::ReadyForAcceptance => (
            "Review ready",
            review.ok_or_else(unavailable)?.overall_feedback.clone(),
        ),
    };
    Ok(Some(GraphqlTaskAttention {
        kind: attention.into(),
        title: title.to_string(),
        summary: preview(&summary, 400),
        gate: gate.cloned().map(TryInto::try_into).transpose()?,
        review: review.cloned().map(Into::into),
        task,
        valid_actions: actions.iter().copied().map(Into::into).collect(),
    }))
}

pub(super) fn preview(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub(super) fn exact_u64(value: u64) -> async_graphql::Result<i64> {
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
