//! Workflow and one-transaction board-bootstrap reads.

use noema_tasks::{WorkflowDefinition, WorkflowId};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{Row, Transaction, params, types::Type};

use super::{
    list::{PreparedQuery, load_connection},
    rows::{decode_stage_record, load_project, load_workspace},
};
use crate::{
    NoemaStore, StoreError, WorkOverview, WorkOverviewQuery, WorkStageTaskCount, WorkTaskQuery,
    WorkTaskScope, WorkWorkflowWithStages, sqlite::conversion_failure,
    work_row::invalid as invalid_sql,
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
            load_workflows(&transaction, &workspace_id)
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
            let workflows = load_workflows(&transaction, &query.workspace_id)?;
            let default_workflow = one_default_workflow(workflows)?;
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

fn load_workflows(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
) -> Result<Vec<WorkWorkflowWithStages>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT workflow.workflow_id, workflow.workspace_id, workflow.name, workflow.revision,
                workflow.is_default, workflow.created_at, workflow.updated_at,
                stage.stage_id, stage.workflow_id, stage.stable_key, stage.display_name,
                stage.ordinal, stage.system_behavior, stage.board_visible
         FROM workflow_definitions workflow
         JOIN workflow_stages stage ON stage.workflow_id = workflow.workflow_id
         WHERE workflow.workspace_id = ?1
         ORDER BY workflow.is_default DESC, workflow.created_at, workflow.workflow_id,
                  stage.ordinal, stage.stage_id",
    )?;
    let rows = statement.query_map([workspace_id.as_str()], decode_workflow_stage)?;
    let mut workflows = Vec::<WorkWorkflowWithStages>::new();
    for row in rows {
        let (workflow, stage) = row?;
        if let Some(current) = workflows
            .last_mut()
            .filter(|current| current.workflow.workflow_id == workflow.workflow_id)
        {
            if current.workflow != workflow {
                return Err(invariant(
                    "workflow join produced inconsistent definition rows",
                ));
            }
            current.stages.push(stage);
        } else {
            workflow.validate().map_err(StoreError::Work)?;
            workflows.push(WorkWorkflowWithStages {
                workflow,
                stages: vec![stage],
            });
        }
    }
    for workflow in &workflows {
        if workflow.stages.is_empty()
            || workflow
                .stages
                .iter()
                .any(|stage| stage.workflow_id != workflow.workflow.workflow_id)
        {
            return Err(invariant("workflow has missing or foreign stages"));
        }
    }
    Ok(workflows)
}

fn decode_workflow_stage(
    row: &Row<'_>,
) -> rusqlite::Result<(WorkflowDefinition, noema_tasks::WorkflowStage)> {
    let revision = u64::try_from(row.get::<_, i64>(3)?)
        .map_err(|error| conversion_failure(3, Type::Integer, error))?;
    if revision == 0 {
        return Err(invalid_sql(3, "workflow revision must be positive"));
    }
    let workflow = WorkflowDefinition {
        workflow_id: WorkflowId::new(row.get::<_, String>(0)?)
            .map_err(|error| conversion_failure(0, Type::Text, error))?,
        workspace_id: WorkspaceId::new(row.get::<_, String>(1)?)
            .map_err(|error| conversion_failure(1, Type::Text, error))?,
        name: row.get(2)?,
        revision,
        is_default: match row.get::<_, i64>(4)? {
            0 => false,
            1 => true,
            _ => return Err(invalid_sql(4, "workflow default flag is not boolean")),
        },
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    };
    Ok((workflow, decode_stage_record(row, 7)?))
}

fn one_default_workflow(
    workflows: Vec<WorkWorkflowWithStages>,
) -> Result<WorkWorkflowWithStages, StoreError> {
    let mut defaults = workflows
        .into_iter()
        .filter(|workflow| workflow.workflow.is_default);
    let default = defaults
        .next()
        .ok_or_else(|| invariant("workspace has no default workflow"))?;
    if defaults.next().is_some() {
        return Err(invariant("workspace has multiple default workflows"));
    }
    Ok(default)
}

fn load_stage_counts(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
    workflow: &WorkWorkflowWithStages,
) -> Result<Vec<WorkStageTaskCount>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT stage_id, COUNT(*) FROM tasks
         WHERE workspace_id = ?1 AND workflow_id = ?2
           AND (?3 IS NULL OR project_id = ?3)
           AND cancelled_at IS NULL
         GROUP BY stage_id",
    )?;
    let rows = statement.query_map(
        params![
            workspace_id.as_str(),
            workflow.workflow.workflow_id.as_str(),
            project_id.map(ProjectId::as_str),
        ],
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
         JOIN workflow_stages stage
           ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
         WHERE task.workspace_id = ?1 AND (?2 IS NULL OR task.project_id = ?2)
           AND task.completed_at IS NULL AND task.cancelled_at IS NULL
           AND (
             (stage.system_behavior = 'human_gate' AND EXISTS (
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
