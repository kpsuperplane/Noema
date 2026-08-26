//! Workflow and one-transaction board-bootstrap reads.

use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{Transaction, params};

use super::{
    list::{PreparedQuery, load_connection},
    rows::{load_project, load_workspace},
};
use crate::{
    NoemaStore, StoreError, WorkOverview, WorkOverviewQuery, WorkStageTaskCount, WorkTaskQuery,
    WorkTaskScope, WorkWorkflowWithStages,
};

impl NoemaStore {
    /// List a workspace's workflow definitions with stages in ordinal order.
    ///
    /// # Errors
    ///
    /// Returns an error when workflow or stage rows cannot be decoded.
    pub async fn list_work_workflows(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<WorkWorkflowWithStages>, StoreError> {
        let workspace_id = workspace_id.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            load_workspace(&transaction, &workspace_id)?;
            Ok(vec![personal_workflow()])
        })
        .await
    }

    /// Assemble a coherent board bootstrap in one SQLite read transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or any failed projection read.
    pub async fn work_overview(
        &self,
        query: WorkOverviewQuery,
    ) -> Result<WorkOverview, StoreError> {
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let workspace = load_workspace(&transaction, &query.workspace_id)?;
            if let Some(project_id) = query.project_id.as_ref() {
                let project = load_project(&transaction, project_id)?;
                if project.workspace_id != query.workspace_id {
                    return Err(StoreError::InvariantViolation {
                        message: format!("project {project_id} is outside overview workspace"),
                    });
                }
            }
            let default_workflow = personal_workflow();
            let board_stage_counts = load_stage_counts(
                &transaction,
                &query.workspace_id,
                query.project_id.as_ref(),
                &default_workflow,
            )?;
            let needs_you_count =
                load_needs_you_count(&transaction, &query.workspace_id, query.project_id.as_ref())?;
            let recent_tasks = load_connection(
                &transaction,
                PreparedQuery::new(WorkTaskQuery {
                    workspace_id: query.workspace_id,
                    project_id: query.project_id,
                    stage_ids: Vec::new(),
                    stage_behaviors: Vec::new(),
                    attention_only: false,
                    scope: WorkTaskScope::Active,
                    first: query.first,
                    after: None,
                })?,
            )?;
            Ok(WorkOverview {
                workspace,
                default_workflow,
                board_stage_counts,
                recent_tasks,
                needs_you_count,
            })
        })
        .await
    }
}

fn personal_workflow() -> WorkWorkflowWithStages {
    WorkWorkflowWithStages {
        workflow: noema_tasks::personal_workflow(),
        stages: noema_tasks::personal_stages(),
    }
}

fn load_stage_counts(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
    workflow: &WorkWorkflowWithStages,
) -> Result<Vec<WorkStageTaskCount>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT stage_id, COUNT(*) FROM tasks
         WHERE workspace_id = ?1
           AND (?2 IS NULL OR project_id = ?2)
           AND cancelled_at IS NULL
         GROUP BY stage_id",
    )?;
    let rows = statement.query_map(
        params![workspace_id.as_str(), project_id.map(ProjectId::as_str),],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let counts = rows.collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    workflow
        .stages
        .iter()
        .filter(|stage| stage.board_visible)
        .map(|stage| {
            let raw = counts.get(stage.stage_id.as_str()).copied().unwrap_or(0);
            Ok(WorkStageTaskCount {
                stage: stage.clone(),
                task_count: u64::try_from(raw).map_err(|_| invariant("stage count is negative"))?,
            })
        })
        .collect()
}

fn load_needs_you_count(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
) -> Result<u64, StoreError> {
    let count = transaction.query_row(
        "SELECT COUNT(*)
         FROM tasks task
         WHERE task.workspace_id = ?1 AND (?2 IS NULL OR task.project_id = ?2)
           AND task.completed_at IS NULL AND task.cancelled_at IS NULL
           AND (
             (task.stage_id = 'stage:personal:waiting' AND EXISTS (
               SELECT 1 FROM task_gates gate
               WHERE gate.gate_id = task.active_gate_id AND gate.task_id = task.task_id
                 AND gate.task_generation = task.generation AND gate.gate_state = 'open'
             ))
           )",
        params![workspace_id.as_str(), project_id.map(ProjectId::as_str)],
        |row| row.get::<_, i64>(0),
    )?;
    u64::try_from(count).map_err(|_| invariant("needs-you count is negative"))
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
