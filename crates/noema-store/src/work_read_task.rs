use noema_tasks::{AgentRunRecord, TaskGateRecord, TaskId, WorkflowDefinition, WorkflowStage};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};
use rusqlite::Transaction;

use super::history::WorkTaskHistory;
use super::rows::{
    derive_attention_actions, load_active_gate, load_current_run, load_project, load_stage,
    load_task, load_workflow, load_workspace, validate_current_links,
};
use crate::{StoreError, WorkTaskDetail};

pub(crate) struct LoadedWorkTask {
    pub(crate) task: noema_tasks::TaskRecord,
    pub(crate) workspace: WorkspaceRecord,
    pub(crate) project: Option<ProjectRecord>,
    pub(crate) stage: WorkflowStage,
    pub(crate) latest_run: Option<AgentRunRecord>,
    pub(crate) current_run: Option<AgentRunRecord>,
    pub(crate) active_gate: Option<TaskGateRecord>,
}

pub(crate) fn load_task_facts(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<Option<LoadedWorkTask>, StoreError> {
    let Some(task) = load_task(transaction, task_id)? else {
        return Ok(None);
    };
    let workspace = load_workspace(transaction, &task.workspace_id)?;
    let project = task
        .project_id
        .as_ref()
        .map(|id| load_project(transaction, id))
        .transpose()?;
    let workflow = load_workflow(transaction, &task.workflow_id)?;
    let stage = load_stage(transaction, &task.stage_id)?;
    stage
        .belongs_to(&workflow.workflow_id)
        .map_err(StoreError::Work)?;
    validate_scope_links(&task, project.as_ref(), &workflow)?;

    let latest_run = task
        .latest_run_id
        .as_deref()
        .map(|id| load_current_run(transaction, id))
        .transpose()?;
    let current_run = latest_run
        .as_ref()
        .filter(|run| {
            matches!(
                run.status,
                noema_tasks::RunStatus::Queued
                    | noema_tasks::RunStatus::Leased
                    | noema_tasks::RunStatus::Running
            )
        })
        .cloned();
    let active_gate = task
        .active_gate_id
        .as_ref()
        .map(|id| load_active_gate(transaction, id))
        .transpose()?;
    validate_current_links(&task, latest_run.as_ref(), active_gate.as_ref())?;

    Ok(Some(LoadedWorkTask {
        task,
        workspace,
        project,
        stage,
        latest_run,
        current_run,
        active_gate,
    }))
}

impl LoadedWorkTask {
    pub(crate) fn into_detail(
        self,
        history: WorkTaskHistory,
        artifacts: Vec<crate::WorkTaskArtifact>,
    ) -> WorkTaskDetail {
        let (attention, valid_actions) = derive_attention_actions(
            &self.task,
            self.stage.system_behavior,
            self.active_gate.as_ref(),
        );
        WorkTaskDetail {
            task: self.task,
            workspace: self.workspace,
            project: self.project,
            stage: self.stage,
            task_document: String::new(),
            review_document: None,
            working_directory: String::new(),
            current_run: self.current_run,
            active_gate: self.active_gate,
            messages: history.messages,
            runs: history.runs,
            artifacts,
            attention,
            valid_actions,
        }
    }
}

fn validate_scope_links(
    task: &noema_tasks::TaskRecord,
    project: Option<&ProjectRecord>,
    workflow: &WorkflowDefinition,
) -> Result<(), StoreError> {
    if project.is_some_and(|project| project.workspace_id != task.workspace_id)
        || workflow.workspace_id != task.workspace_id
    {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} crosses a workspace scope link", task.task_id),
        });
    }
    Ok(())
}
