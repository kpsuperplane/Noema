//! Read-consistent, bounded execution context for a supervised Work run.

use std::collections::HashSet;

use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, ProjectContextSnapshot, RunKind, RunStatus,
    TaskExecutionContract, TaskGateRecord, TaskMessageKind, TaskMessageRecord, TaskRecord,
    TaskReviewRecord, TaskSubmissionRecord, WorkspaceContextSnapshot,
};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};
use rusqlite::{Row, Transaction, params};

use crate::{
    NoemaStore, StoreError,
    work_reads::rows::{
        decode_gate, load_active_gate, load_project, load_stage, load_task, load_workflow,
        load_workspace, validate_current_links,
    },
    work_reads::{
        evidence::{load_contract, load_review, load_submission},
        history::decode_message,
    },
    work_run_context_records::{
        WORK_RUN_CONTEXT_MAX_GATES, WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN,
        WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS, WORK_RUN_CONTEXT_MAX_MESSAGES, WorkRunExecutionContext,
    },
    work_runs::rows::load_run_tx,
};

const MAX_CONTEXT_TEXT_BYTES: usize = 64 * 1024;
pub(super) const MAX_CONTEXT_PAYLOAD_BYTES: usize = 128 * 1024;

impl NoemaStore {
    /// Load one run envelope and its bounded, role-relevant durable evidence.
    ///
    /// All rows are read from one deferred SQLite transaction. Executor and
    /// Reviewer contexts are fenced to their exact immutable contract;
    /// Planner contexts deliberately have no contract and use only the bounded
    /// current workspace/project descriptions that the Planner may normalize.
    ///
    /// # Errors
    ///
    /// Returns an error when the run context rows are malformed, inconsistent,
    /// or exceed their bounded projection limits.
    pub async fn get_work_run_execution_context(
        &self,
        run_id: &str,
    ) -> Result<Option<WorkRunExecutionContext>, StoreError> {
        let run_id = run_id.trim().to_owned();
        if run_id.is_empty() {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run_id",
                    message: "run id cannot be blank".to_string(),
                },
            ));
        }
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let context = load_work_run_execution_context_tx(&transaction, &run_id)?;
            transaction.commit()?;
            Ok(context)
        })
        .await
    }
}

pub(super) fn load_work_run_execution_context_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<WorkRunExecutionContext>, StoreError> {
    let Some(run) = load_run_tx(transaction, run_id)? else {
        return Ok(None);
    };
    let task =
        load_task(transaction, &run.task_id)?.ok_or_else(|| StoreError::InvariantViolation {
            message: format!("run {} references missing task {}", run.run_id, run.task_id),
        })?;
    validate_run_task_fence(&run, &task)?;
    ensure_context_text(&task.title, "task.title")?;
    ensure_context_text(&task.description_markdown, "task.description_markdown")?;

    let workspace_row = load_workspace(transaction, &task.workspace_id)?;
    let project_row = task
        .project_id
        .as_ref()
        .map(|project_id| load_project(transaction, project_id))
        .transpose()?;
    if project_row
        .as_ref()
        .is_some_and(|project| project.workspace_id != task.workspace_id)
    {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} project crosses workspace fence", task.task_id),
        });
    }
    let workflow = load_workflow(transaction, &task.workflow_id)?;
    let stage = load_stage(transaction, &task.stage_id)?;
    stage
        .belongs_to(&workflow.workflow_id)
        .map_err(StoreError::Work)?;

    let contract = run
        .contract_id
        .as_ref()
        .map(|contract_id| load_contract(transaction, contract_id))
        .transpose()?;
    if let Some(contract) = contract.as_ref() {
        ensure_contract_bounds(contract)?;
    }
    validate_contract_fence(&run, &task, contract.as_ref())?;
    let (workspace, project) = context_snapshots(
        &run,
        &task,
        &workspace_row,
        project_row.as_ref(),
        contract.as_ref(),
    )?;

    let active_gate = task
        .active_gate_id
        .as_ref()
        .map(|gate_id| load_active_gate(transaction, gate_id))
        .transpose()?;
    validate_current_links(&task, Some(&run), active_gate.as_ref())?;
    let relevant_gates = load_relevant_gates(transaction, &task, &run, active_gate.as_ref())?;
    let messages = load_relevant_messages(transaction, &task, &run, &relevant_gates)?;

    let submission_id = run
        .triggering_submission_id
        .as_deref()
        .or(task.latest_submission_id.as_deref());
    let expected_criteria = contract
        .as_ref()
        .map(|contract| {
            contract
                .criteria
                .iter()
                .map(|criterion| criterion.criterion_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let latest_submission = submission_id
        .map(|id| load_submission(transaction, id, &expected_criteria))
        .transpose()?;
    if let Some(submission) = latest_submission.as_ref() {
        ensure_submission_bounds(submission)?;
    }
    validate_submission_link(&run, &task, contract.as_ref(), latest_submission.as_ref())?;

    let review_id = run
        .triggering_review_id
        .as_deref()
        .or(task.latest_review_id.as_deref());
    let latest_review = review_id
        .map(|id| load_review(transaction, id, &expected_criteria))
        .transpose()?;
    if let Some(review) = latest_review.as_ref() {
        ensure_review_bounds(review)?;
    }
    validate_review_link(&run, &task, contract.as_ref(), latest_review.as_ref())?;

    // Reviewers receive the immutable contract and latest submission below;
    // executor transcript items are neither authoritative evidence nor needed
    // to perform the review, so keep them out of the reviewer checkpoint.
    let lineage = if run.run_kind == RunKind::Reviewer {
        Vec::new()
    } else {
        load_lineage_items(transaction, &run)?
    };
    Ok(Some(WorkRunExecutionContext {
        run,
        task,
        workflow,
        stage,
        contract,
        workspace,
        project,
        active_gate,
        relevant_gates,
        messages,
        latest_submission,
        latest_review,
        lineage,
    }))
}

fn validate_run_task_fence(run: &AgentRunRecord, task: &TaskRecord) -> Result<(), StoreError> {
    if !matches!(run.status, RunStatus::Leased | RunStatus::Running) {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} is not leased or running", run.run_id),
        });
    }
    if run.task_id != task.task_id
        || run.task_generation != task.generation
        || task.latest_run_id.as_deref() != Some(run.run_id.as_str())
    {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} crosses task generation fence", run.run_id),
        });
    }
    Ok(())
}

fn validate_contract_fence(
    run: &AgentRunRecord,
    task: &TaskRecord,
    contract: Option<&TaskExecutionContract>,
) -> Result<(), StoreError> {
    match (run.run_kind, run.contract_id.as_ref(), contract) {
        (RunKind::Planner, None, None) => Ok(()),
        (RunKind::Executor | RunKind::Reviewer, Some(run_contract_id), Some(contract))
            if run_contract_id == &contract.contract_id
                && task.current_contract_id.as_ref() == Some(run_contract_id)
                && contract.matches_fence(&task.task_id, task.generation) =>
        {
            Ok(())
        }
        _ => Err(StoreError::InvariantViolation {
            message: format!(
                "run {} does not match the current contract fence",
                run.run_id
            ),
        }),
    }
}

fn context_snapshots(
    run: &AgentRunRecord,
    task: &TaskRecord,
    workspace: &WorkspaceRecord,
    project: Option<&ProjectRecord>,
    contract: Option<&TaskExecutionContract>,
) -> Result<(WorkspaceContextSnapshot, Option<ProjectContextSnapshot>), StoreError> {
    if let Some(contract) = contract {
        if contract.workspace_context.workspace_id != task.workspace_id {
            return Err(StoreError::InvariantViolation {
                message: format!("contract {} crosses workspace fence", contract.contract_id),
            });
        }
        let project = match (&task.project_id, &contract.project_context, project) {
            (None, None, _) => None,
            (Some(task_project_id), Some(contract_project), Some(live_project))
                if task_project_id == &contract_project.project_id
                    && live_project.project_id == contract_project.project_id =>
            {
                Some(contract_project.clone())
            }
            _ => {
                return Err(StoreError::InvariantViolation {
                    message: format!("contract {} crosses project fence", contract.contract_id),
                });
            }
        };
        return Ok((contract.workspace_context.clone(), project));
    }
    if run.run_kind != RunKind::Planner || task.current_contract_id.is_some() {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "Planner context has invalid contract state for {}",
                run.run_id
            ),
        });
    }
    let workspace_snapshot = WorkspaceContextSnapshot {
        workspace_id: workspace.workspace_id.clone(),
        name: bounded_text(workspace.name.clone(), "workspace.name")?,
        description: bounded_text(workspace.description.clone(), "workspace.description")?,
    };
    let project_snapshot = project
        .map(|project| {
            Ok::<ProjectContextSnapshot, StoreError>(ProjectContextSnapshot {
                project_id: project.project_id.clone(),
                name: bounded_text(project.name.clone(), "project.name")?,
                description: bounded_text(project.description.clone(), "project.description")?,
                folder: project.folder.clone(),
            })
        })
        .transpose()?;
    Ok((workspace_snapshot, project_snapshot))
}

fn load_relevant_gates(
    transaction: &Transaction<'_>,
    task: &TaskRecord,
    run: &AgentRunRecord,
    active_gate: Option<&TaskGateRecord>,
) -> Result<Vec<TaskGateRecord>, StoreError> {
    let parent_run_id = run.parent_run_id.as_deref();
    let mut statement = transaction.prepare(
        "SELECT gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, suggested_answers_json, opened_by_actor_id, originating_run_id, resolved_by_actor_id, resolution_message_id, opened_at, resolved_at FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND (originating_run_id = ?3 OR originating_run_id = ?4 OR gate_id = ?5 OR resolution_message_id IN (SELECT message_id FROM task_messages WHERE consumed_by_run_id IN (?3, ?4))) ORDER BY opened_at, gate_id LIMIT ?6",
    )?;
    let rows = statement.query_map(
        params![
            task.task_id.as_str(),
            i64::try_from(task.generation).map_err(|_| StoreError::InvariantViolation {
                message: "task generation exceeds SQLite range".to_string()
            })?,
            run.run_id.as_str(),
            parent_run_id,
            active_gate.map(|gate| gate.gate_id.as_str()),
            (WORK_RUN_CONTEXT_MAX_GATES + 1) as i64,
        ],
        decode_gate,
    )?;
    let mut gates = rows.collect::<Result<Vec<_>, _>>()?;
    if gates.len() > WORK_RUN_CONTEXT_MAX_GATES {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} exceeds bounded gate context", task.task_id),
        });
    }
    for gate in &gates {
        gate.validate().map_err(StoreError::Work)?;
    }
    if let Some(active_gate) = active_gate
        && !gates.iter().any(|gate| gate.gate_id == active_gate.gate_id)
    {
        gates.push(active_gate.clone());
        gates.sort_by(|left, right| {
            left.opened_at
                .cmp(&right.opened_at)
                .then_with(|| left.gate_id.as_str().cmp(right.gate_id.as_str()))
        });
    }
    Ok(gates)
}

fn load_relevant_messages(
    transaction: &Transaction<'_>,
    task: &TaskRecord,
    run: &AgentRunRecord,
    gates: &[TaskGateRecord],
) -> Result<Vec<TaskMessageRecord>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT message_id, task_id, task_generation, contract_id, gate_id, review_id, message_kind, body_markdown, approval_decision, author_actor_id, consumed_by_run_id, consumed_at, created_at FROM task_messages WHERE task_id = ?1 AND task_generation = ?2 AND (consumed_by_run_id = ?3 OR consumed_by_run_id = ?5 OR (consumed_by_run_id IS NULL AND (review_id = ?4 OR gate_id IN (SELECT gate_id FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND (originating_run_id = ?3 OR originating_run_id = ?5))))) ORDER BY created_at, message_id LIMIT ?6",
    )?;
    let rows = statement.query_map(
        params![
            task.task_id.as_str(),
            i64::try_from(task.generation).map_err(|_| StoreError::InvariantViolation {
                message: "task generation exceeds SQLite range".to_string()
            })?,
            run.run_id.as_str(),
            run.triggering_review_id.as_deref(),
            run.parent_run_id.as_deref(),
            (WORK_RUN_CONTEXT_MAX_MESSAGES + 1) as i64,
        ],
        decode_message,
    )?;
    let messages = rows.collect::<Result<Vec<_>, _>>()?;
    if messages.len() > WORK_RUN_CONTEXT_MAX_MESSAGES {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} exceeds bounded message context", task.task_id),
        });
    }
    for message in &messages {
        validate_message(message, task)?;
        if message
            .gate_id
            .as_ref()
            .is_some_and(|gate_id| !gates.iter().any(|gate| &gate.gate_id == gate_id))
        {
            return Err(StoreError::InvariantViolation {
                message: format!("message {} references unrelated gate", message.message_id),
            });
        }
    }
    Ok(messages)
}

fn validate_submission_link(
    run: &AgentRunRecord,
    task: &TaskRecord,
    contract: Option<&TaskExecutionContract>,
    submission: Option<&TaskSubmissionRecord>,
) -> Result<(), StoreError> {
    let Some(submission) = submission else {
        return Ok(());
    };
    if submission.task_id != task.task_id
        || run.run_kind == RunKind::Planner
        || contract.is_none_or(|contract| submission.contract_id != contract.contract_id)
    {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} loaded a foreign submission", run.run_id),
        });
    }
    Ok(())
}

fn validate_review_link(
    run: &AgentRunRecord,
    task: &TaskRecord,
    contract: Option<&TaskExecutionContract>,
    review: Option<&TaskReviewRecord>,
) -> Result<(), StoreError> {
    let Some(review) = review else { return Ok(()) };
    if review.task_id != task.task_id
        || run.run_kind == RunKind::Planner
        || contract.is_none_or(|contract| review.contract_id != contract.contract_id)
    {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} loaded a foreign review", run.run_id),
        });
    }
    Ok(())
}

fn load_lineage_items(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
) -> Result<Vec<AgentRunItemRecord>, StoreError> {
    let mut runs = Vec::with_capacity(WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS);
    let mut seen = HashSet::new();
    let mut current = Some(run.clone());
    while let Some(current_run) = current {
        if !seen.insert(current_run.run_id.clone()) {
            return Err(StoreError::InvariantViolation {
                message: format!("run {} has cyclic parent lineage", run.run_id),
            });
        }
        runs.push(current_run.clone());
        if runs.len() == WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS {
            break;
        }
        current = current_run
            .parent_run_id
            .as_deref()
            .map(|parent_id| load_run_tx(transaction, parent_id))
            .transpose()?
            .flatten();
        if current.is_none() && current_run.parent_run_id.is_some() {
            return Err(StoreError::InvariantViolation {
                message: format!("run {} references missing parent run", current_run.run_id),
            });
        }
    }
    let mut items = Vec::new();
    for lineage_run in runs.into_iter().rev() {
        let mut statement = transaction.prepare("SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM agent_run_items WHERE run_id = ?1 AND kind <> 'context_checkpoint' ORDER BY sequence_index DESC, item_id DESC LIMIT ?2")?;
        let rows = statement.query_map(
            params![
                lineage_run.run_id.as_str(),
                WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN as i64
            ],
            decode_run_item,
        )?;
        let mut run_items = rows.collect::<Result<Vec<_>, _>>()?;
        run_items.reverse();
        items.extend(run_items);
    }
    Ok(items)
}

pub(super) fn decode_run_item(row: &Row<'_>) -> rusqlite::Result<AgentRunItemRecord> {
    crate::work_runs::rows::decode_run_item(row, Some(MAX_CONTEXT_PAYLOAD_BYTES))
}

fn validate_message(message: &TaskMessageRecord, task: &TaskRecord) -> Result<(), StoreError> {
    if message.task_id != task.task_id
        || message.task_generation != task.generation
        || message.body_markdown.trim().is_empty()
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "message {} crosses task fence or is blank",
                message.message_id
            ),
        });
    }
    ensure_context_text(&message.body_markdown, "task_message.body_markdown")?;
    ensure_context_text(&message.author_actor_id, "task_message.author_actor_id")?;
    if message.kind != TaskMessageKind::HumanAnswer && message.approval_decision.is_some() {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "message {} has an approval decision on a non-answer",
                message.message_id
            ),
        });
    }
    Ok(())
}

fn ensure_contract_bounds(contract: &TaskExecutionContract) -> Result<(), StoreError> {
    ensure_context_text(&contract.request_markdown, "contract.request_markdown")?;
    if let Some(plan) = contract.execution_plan_markdown.as_deref() {
        ensure_context_text(plan, "contract.execution_plan_markdown")?;
    }
    ensure_context_text(&contract.workspace_context.name, "workspace.name")?;
    ensure_context_text(
        &contract.workspace_context.description,
        "workspace.description",
    )?;
    for criterion in &contract.criteria {
        ensure_context_text(&criterion.criterion_id, "criterion.criterion_id")?;
        ensure_context_text(&criterion.description, "criterion.description")?;
        if let Some(expected) = criterion.expected_evidence.as_deref() {
            ensure_context_text(expected, "criterion.expected_evidence")?;
        }
    }
    if let Some(project) = contract.project_context.as_ref() {
        ensure_context_text(&project.name, "project.name")?;
        ensure_context_text(&project.description, "project.description")?;
    }
    Ok(())
}

fn ensure_submission_bounds(
    submission: &noema_tasks::TaskSubmissionRecord,
) -> Result<(), StoreError> {
    ensure_context_text(&submission.summary, "submission.summary")?;
    ensure_context_text(&submission.result_markdown, "submission.result_markdown")?;
    for criterion in &submission.criteria {
        ensure_context_text(&criterion.criterion_id, "submission.criterion_id")?;
        ensure_context_text(&criterion.evidence_markdown, "submission.evidence")?;
    }
    Ok(())
}

fn ensure_review_bounds(review: &TaskReviewRecord) -> Result<(), StoreError> {
    ensure_context_text(&review.overall_feedback, "review.overall_feedback")?;
    for criterion in &review.criteria {
        ensure_context_text(&criterion.criterion_id, "review.criterion_id")?;
        if let Some(evidence) = criterion.evidence_markdown.as_deref() {
            ensure_context_text(evidence, "review.evidence")?;
        }
        if let Some(feedback) = criterion.feedback.as_deref() {
            ensure_context_text(feedback, "review.feedback")?;
        }
    }
    Ok(())
}

fn ensure_context_text(value: &str, field: &'static str) -> Result<(), StoreError> {
    if value.len() > MAX_CONTEXT_TEXT_BYTES {
        return Err(StoreError::InvariantViolation {
            message: format!("{field} exceeds bounded context size"),
        });
    }
    Ok(())
}

fn bounded_text(value: String, field: &'static str) -> Result<String, StoreError> {
    ensure_context_text(&value, field)?;
    Ok(value)
}
