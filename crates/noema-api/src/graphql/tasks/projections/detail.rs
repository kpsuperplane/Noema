//! Context-backed fields for bounded task-detail history connections.

use async_graphql::{Context, Object, Result};
use noema_store::{
    WorkContractCursor, WorkContractHistoryQuery, WorkEventBeforeQuery, WorkEventCursor,
    WorkGateCursor, WorkGateHistoryQuery, WorkMessageCursor, WorkMessageHistoryQuery, WorkPageSize,
    WorkReviewCursor, WorkReviewHistoryQuery, WorkRunCursor, WorkRunHistoryQuery,
    WorkSubmissionCursor, WorkSubmissionHistoryQuery, WorkTaskArtifactCursor,
    WorkTaskArtifactQuery,
};

use crate::graphql::schema::GraphqlState;
use crate::graphql::tasks::*;

#[Object(name = "TaskDetail")]
impl GraphqlTaskDetail {
    async fn task_id(&self) -> &str {
        &self.task_id
    }

    async fn workspace(&self) -> &GraphqlWorkspace {
        &self.workspace
    }

    async fn project(&self) -> Option<&GraphqlProject> {
        self.project.as_ref()
    }

    async fn title(&self) -> &str {
        &self.title
    }

    async fn description(&self) -> &str {
        &self.description
    }

    async fn description_preview(&self) -> &str {
        &self.description_preview
    }

    async fn stage(&self) -> &GraphqlWorkflowStage {
        &self.stage
    }

    async fn revision(&self) -> i64 {
        self.revision
    }

    async fn generation(&self) -> i64 {
        self.generation
    }

    async fn created_at(&self) -> &str {
        &self.created_at
    }

    async fn updated_at(&self) -> &str {
        &self.updated_at
    }

    async fn completed_at(&self) -> Option<&str> {
        self.completed_at.as_deref()
    }

    async fn source(&self) -> &GraphqlTaskSource {
        &self.source
    }

    async fn workflow(&self) -> &GraphqlWorkflow {
        &self.workflow
    }

    async fn current_contract(&self) -> Option<&GraphqlTaskExecutionContract> {
        self.current_contract.as_ref()
    }

    async fn current_run(&self) -> Option<&GraphqlCurrentRunSummary> {
        self.current_run.as_ref()
    }

    async fn active_gate(&self) -> Option<&GraphqlTaskGate> {
        self.active_gate.as_ref()
    }

    async fn latest_submission(&self) -> Option<&GraphqlTaskSubmission> {
        self.latest_submission.as_ref()
    }

    async fn accepted_result(&self) -> Option<&GraphqlTaskSubmission> {
        self.accepted_result.as_ref()
    }

    async fn latest_review(&self) -> Option<&GraphqlTaskReviewSummary> {
        self.latest_review.as_ref()
    }

    async fn attention(&self) -> Option<&GraphqlTaskAttention> {
        self.attention.as_ref()
    }

    async fn valid_actions(&self) -> &[GraphqlValidTaskAction] {
        &self.valid_actions
    }

    /// Return immutable contract history, newest first.
    async fn contracts(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskExecutionContractConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkContractCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_contracts(WorkContractHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map_err(work_error)
            .and_then(contract_connection)
    }

    /// Return immutable gate history, newest first.
    async fn gates(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskGateConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkGateCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_gates(WorkGateHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map_err(work_error)
            .and_then(gate_connection)
    }

    /// Return immutable human-message history, newest first.
    async fn messages(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskMessageConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkMessageCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_messages(WorkMessageHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map_err(work_error)
            .and_then(message_connection)
    }

    /// Return immutable run history, newest first.
    async fn runs(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskRunConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkRunCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_runs(WorkRunHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map_err(work_error)
            .and_then(run_connection)
    }

    /// Return immutable submission history, newest first.
    async fn submissions(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskSubmissionConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkSubmissionCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_submissions(WorkSubmissionHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map(submission_connection)
            .map_err(work_error)
    }

    /// Return immutable review history, newest first.
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskReviewConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkReviewCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_reviews(WorkReviewHistoryQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map(review_connection)
            .map_err(work_error)
    }

    /// Return immutable task-owned artifact evidence, newest first.
    async fn artifacts(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskArtifactConnection> {
        let task_id = parse_task_id(&self.task_id)?;
        let after = decode_history_cursor(after, WorkTaskArtifactCursor::decode)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_task_artifacts(WorkTaskArtifactQuery {
                task_id,
                first: history_page_size(first)?,
                after,
            })
            .await
            .map_err(work_error)
            .and_then(artifact_connection)
    }

    /// Return durable task activity, newest first.
    async fn activity(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlWorkEventConnection> {
        let workspace_id = parse_workspace_id(&self.workspace.workspace_id)?;
        let task_id = parse_task_id(&self.task_id)?;
        let before = after
            .as_deref()
            .map(WorkEventCursor::decode)
            .transpose()
            .map_err(cursor_error)?;
        ctx.data_unchecked::<GraphqlState>()
            .store()?
            .list_work_events_before(WorkEventBeforeQuery {
                workspace_id,
                project_id: None,
                task_id: Some(task_id),
                run_id: None,
                before,
                first: history_page_size(first)?,
            })
            .await
            .map_err(work_error)
            .and_then(event_connection)
    }
}

fn history_page_size(first: Option<i32>) -> Result<WorkPageSize> {
    let first = u32::try_from(first.unwrap_or(20))
        .map_err(|_| cursor_error(noema_store::WorkCursorError))?;
    WorkPageSize::new(first).map_err(cursor_error)
}

fn decode_history_cursor<T>(
    value: Option<String>,
    decode: impl FnOnce(&str) -> std::result::Result<T, noema_store::WorkCursorError>,
) -> Result<Option<T>> {
    value
        .as_deref()
        .map(decode)
        .transpose()
        .map_err(cursor_error)
}
