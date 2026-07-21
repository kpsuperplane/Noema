use noema_tasks::{
    AgentRunRecord, RunKind, RunStatus, TaskExecutionContract, TaskGateRecord, TaskId,
    TaskReviewRecord, TaskSubmissionRecord, WorkflowDefinition, WorkflowStage,
};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};
use rusqlite::Transaction;

use super::history::WorkTaskHistory;
use super::{
    evidence::{load_contract, load_review, load_submission},
    rows::{
        derive_attention_actions, load_active_gate, load_current_run, load_project, load_stage,
        load_task, load_workflow, load_workspace, validate_current_links,
    },
    validate_evidence_links,
};
use crate::{StoreError, WorkTaskDetail};

pub(crate) struct LoadedWorkTask {
    pub(crate) task: noema_tasks::TaskRecord,
    pub(crate) workspace: WorkspaceRecord,
    pub(crate) project: Option<ProjectRecord>,
    pub(crate) stage: WorkflowStage,
    pub(crate) current_contract: Option<TaskExecutionContract>,
    pub(crate) latest_run: Option<AgentRunRecord>,
    pub(crate) current_run: Option<AgentRunRecord>,
    pub(crate) active_gate: Option<TaskGateRecord>,
    pub(crate) latest_submission: Option<TaskSubmissionRecord>,
    pub(crate) completed_submission: Option<TaskSubmissionRecord>,
    pub(crate) latest_review: Option<TaskReviewRecord>,
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
    let current_contract = task
        .current_contract_id
        .as_ref()
        .map(|id| load_contract(transaction, id))
        .transpose()?;
    let expected_criteria = current_contract
        .as_ref()
        .map(|contract| {
            contract
                .criteria
                .iter()
                .map(|criterion| criterion.criterion_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let latest_submission = task
        .latest_submission_id
        .as_deref()
        .map(|id| load_submission(transaction, id, &expected_criteria))
        .transpose()?;
    let completed_submission = task
        .completed_submission_id
        .as_deref()
        .map(|id| load_submission(transaction, id, &expected_criteria))
        .transpose()?;
    let latest_review = task
        .latest_review_id
        .as_deref()
        .map(|id| load_review(transaction, id, &expected_criteria))
        .transpose()?;

    validate_current_links(&task, latest_run.as_ref(), active_gate.as_ref())?;
    validate_evidence_links(
        &task,
        current_contract.as_ref(),
        latest_submission.as_ref(),
        latest_review.as_ref(),
    )?;
    validate_evidence_run_links(
        transaction,
        &task,
        latest_submission.as_ref(),
        latest_review.as_ref(),
    )?;
    validate_contract_links(
        &task,
        current_contract.as_ref(),
        latest_run.as_ref(),
        active_gate.as_ref(),
    )?;

    Ok(Some(LoadedWorkTask {
        task,
        workspace,
        project,
        stage,
        current_contract,
        latest_run,
        current_run,
        active_gate,
        latest_submission,
        completed_submission,
        latest_review,
    }))
}

impl LoadedWorkTask {
    pub(crate) fn into_detail(
        self,
        history: WorkTaskHistory,
        artifacts: Vec<crate::WorkTaskArtifact>,
    ) -> WorkTaskDetail {
        let (attention, valid_actions) =
            derive_attention_actions(self.stage.system_behavior, self.active_gate.as_ref());
        WorkTaskDetail {
            task: self.task,
            workspace: self.workspace,
            project: self.project,
            stage: self.stage,
            current_contract: self.current_contract,
            current_run: self.current_run,
            active_gate: self.active_gate,
            latest_submission: self.latest_submission,
            completed_submission: self.completed_submission,
            latest_review: self.latest_review,
            messages: history.messages,
            runs: history.runs,
            submissions: history.submissions,
            reviews: history.reviews,
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

fn validate_contract_links(
    task: &noema_tasks::TaskRecord,
    contract: Option<&TaskExecutionContract>,
    run: Option<&AgentRunRecord>,
    gate: Option<&TaskGateRecord>,
) -> Result<(), StoreError> {
    let contract_id = contract.map(|contract| &contract.contract_id);
    let run_ok = run.is_none_or(|run| run.contract_id.as_ref() == contract_id);
    let gate_ok = gate.is_none_or(|gate| gate.contract_id.as_ref() == contract_id);
    let scope_ok = contract.is_none_or(|contract| {
        contract.workspace_context.workspace_id == task.workspace_id
            && match (&task.project_id, &contract.project_context) {
                (None, None) => true,
                (Some(task_project), Some(contract_project)) => {
                    task_project == &contract_project.project_id
                }
                _ => false,
            }
    });
    if run_ok && gate_ok && scope_ok {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: format!("task {} crosses a current contract link", task.task_id),
        })
    }
}

fn validate_evidence_run_links(
    transaction: &Transaction<'_>,
    task: &noema_tasks::TaskRecord,
    submission: Option<&TaskSubmissionRecord>,
    review: Option<&TaskReviewRecord>,
) -> Result<(), StoreError> {
    if let Some(submission) = submission {
        let run = load_current_run(transaction, &submission.executor_run_id)?;
        if run.run_kind != RunKind::Executor
            || run.status != RunStatus::Completed
            || run.task_id != task.task_id
            || run.task_generation != task.generation
            || run.contract_id.as_ref() != Some(&submission.contract_id)
            || run.review_round != submission.review_round
        {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "submission {} crosses its executor run fence",
                    submission.submission_id
                ),
            });
        }
    }
    if let Some(review) = review {
        let run = load_current_run(transaction, &review.reviewer_run_id)?;
        let status_ok = match review.overall_verdict {
            noema_tasks::TaskReviewVerdict::NeedsHuman => {
                matches!(
                    run.status,
                    RunStatus::WaitingForApproval | RunStatus::Completed
                )
            }
            noema_tasks::TaskReviewVerdict::Approve
            | noema_tasks::TaskReviewVerdict::RequestChanges => run.status == RunStatus::Completed,
        };
        if run.run_kind != RunKind::Reviewer
            || !status_ok
            || run.task_id != task.task_id
            || run.task_generation != task.generation
            || run.contract_id.as_ref() != Some(&review.contract_id)
            || run.triggering_submission_id.as_deref()
                != Some(review.reviewed_submission_id.as_str())
        {
            return Err(StoreError::InvariantViolation {
                message: format!("review {} crosses its reviewer run fence", review.review_id),
            });
        }
        if submission
            .is_none_or(|submission| submission.submission_id != review.reviewed_submission_id)
        {
            let reviewed_submission =
                load_reviewed_submission_link(transaction, &review.reviewed_submission_id)?;
            if reviewed_submission.0 != task.task_id || reviewed_submission.1 != review.contract_id
            {
                return Err(StoreError::InvariantViolation {
                    message: format!(
                        "review {} crosses its reviewed submission link",
                        review.review_id
                    ),
                });
            }
        }
    }
    Ok(())
}

fn load_reviewed_submission_link(
    transaction: &Transaction<'_>,
    submission_id: &str,
) -> Result<(TaskId, noema_tasks::TaskContractId), StoreError> {
    transaction
        .query_row(
            "SELECT task_id, contract_id FROM task_submissions
             WHERE submission_id = ?1 LIMIT 1",
            [submission_id],
            |row| {
                Ok((
                    TaskId::new(row.get::<_, String>(0)?).map_err(|error| {
                        crate::sqlite::conversion_failure(0, rusqlite::types::Type::Text, error)
                    })?,
                    noema_tasks::TaskContractId::new(row.get::<_, String>(1)?).map_err(
                        |error| {
                            crate::sqlite::conversion_failure(1, rusqlite::types::Type::Text, error)
                        },
                    )?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("review references missing submission: {submission_id}"),
        })
}

use rusqlite::OptionalExtension;
