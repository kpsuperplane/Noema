use std::str::FromStr;

use noema_tasks::{
    AcpExecutorSnapshot, ContractOrigin, TaskComplexity, TaskContractId, TaskExecutionContract,
    TaskExecutionPolicy, TaskExecutorBackend, TaskExecutorSelection, TaskId, TaskReviewRecord,
    TaskSubmissionRecord, TaskValidationCriterion, WorkspaceContextSnapshot,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Row, Transaction, types::Type};

use crate::{
    StoreError,
    sqlite::conversion_failure,
    work_row::{invalid as noncanonical_sql, nonnegative_u32, positive_u32, positive_u64},
};

const EMBEDDED_LIMIT: usize = 100;

pub(crate) fn load_contract(
    transaction: &Transaction<'_>,
    contract_id: &TaskContractId,
) -> Result<TaskExecutionContract, StoreError> {
    let Some(core) = transaction
        .query_row(
            "SELECT contract_id, task_id, version, task_generation, supersedes_contract_id,
                    origin, request_markdown, execution_plan_markdown, complexity,
                    max_provider_continuations, max_tool_calls, max_active_minutes,
                    progress_audit_interval, max_automatic_retries, max_review_rounds,
                    workspace_id_snapshot, workspace_name_snapshot, workspace_description_snapshot,
                    project_id_snapshot, project_name_snapshot, project_description_snapshot,
                    project_folder_snapshot, executor_backend_kind, executor_agent_id,
                    executor_acp_connection_revision, executor_acp_launch_json, effective_cwd,
                    created_by_actor_id, created_at
             FROM task_execution_contracts WHERE contract_id = ?1 LIMIT 1",
            [contract_id.as_str()],
            decode_contract_core,
        )
        .optional()?
    else {
        return Err(super::rows::missing_link("contract", contract_id.as_str()));
    };
    let criteria = load_contract_criteria(transaction, contract_id)?;
    let executor_model =
        crate::work_commands::helpers::load_contract_model_tx(transaction, contract_id, false)?;
    let reviewer_model =
        crate::work_commands::helpers::load_contract_model_tx(transaction, contract_id, true)?;
    let contract = TaskExecutionContract {
        contract_id: core.contract_id,
        task_id: core.task_id,
        version: core.version,
        task_generation: core.task_generation,
        supersedes_contract_id: core.supersedes_contract_id,
        origin: core.origin,
        request_markdown: core.request_markdown,
        execution_plan_markdown: core.execution_plan_markdown,
        criteria,
        complexity: core.complexity,
        executor_model,
        executor: core.executor,
        effective_cwd: core.effective_cwd,
        reviewer_model,
        execution_policy: core.execution_policy,
        workspace_context: core.workspace_context,
        project_context: core.project_context,
        created_by_actor_id: core.created_by_actor_id,
        created_at: core.created_at,
    };
    if contract.clone().normalized().map_err(StoreError::Work)? != contract {
        return Err(noncanonical("contract row is not canonically normalized"));
    }
    Ok(contract)
}

struct ContractCore {
    contract_id: TaskContractId,
    task_id: TaskId,
    version: u32,
    task_generation: u64,
    supersedes_contract_id: Option<TaskContractId>,
    origin: ContractOrigin,
    request_markdown: String,
    execution_plan_markdown: Option<String>,
    complexity: TaskComplexity,
    execution_policy: TaskExecutionPolicy,
    workspace_context: WorkspaceContextSnapshot,
    project_context: Option<noema_tasks::ProjectContextSnapshot>,
    executor: TaskExecutorSelection,
    effective_cwd: Option<String>,
    created_by_actor_id: String,
    created_at: String,
}

fn decode_contract_core(row: &Row<'_>) -> rusqlite::Result<ContractCore> {
    let project_context = match (
        row.get::<_, Option<String>>(18)?,
        row.get::<_, Option<String>>(19)?,
        row.get::<_, Option<String>>(20)?,
        row.get::<_, Option<String>>(21)?,
    ) {
        (None, None, None, None) => None,
        (Some(id), Some(name), Some(description), folder) => {
            Some(noema_tasks::ProjectContextSnapshot {
                project_id: ProjectId::new(id)
                    .map_err(|e| conversion_failure(18, Type::Text, e))?,
                name,
                description,
                folder,
            })
        }
        _ => return Err(noncanonical_sql(18, "partial project context snapshot")),
    };
    let backend = TaskExecutorBackend::from_str(&row.get::<_, String>(22)?)
        .map_err(|e| conversion_failure(22, Type::Text, e))?;
    let connection_revision = row.get::<_, Option<i64>>(24)?;
    let acp = row
        .get::<_, Option<String>>(25)?
        .map(|json| serde_json::from_str::<AcpExecutorSnapshot>(&json))
        .transpose()
        .map_err(|e| conversion_failure(25, Type::Text, e))?;
    if acp
        .as_ref()
        .and_then(|value| i64::try_from(value.connection_revision).ok())
        != connection_revision
    {
        return Err(noncanonical_sql(
            24,
            "ACP revision does not match launch snapshot",
        ));
    }
    let execution_policy = TaskExecutionPolicy {
        max_provider_continuations: positive_u32(row, 9)?,
        max_tool_calls: positive_u32(row, 10)?,
        max_active_minutes: positive_u32(row, 11)?,
        progress_audit_interval: positive_u32(row, 12)?,
        max_automatic_retries: nonnegative_u32(row, 13)?,
        max_review_rounds: positive_u32(row, 14)?,
    }
    .validated()
    .map_err(|e| conversion_failure(9, Type::Integer, e))?;
    Ok(ContractCore {
        contract_id: TaskContractId::new(row.get::<_, String>(0)?)
            .map_err(|e| conversion_failure(0, Type::Text, e))?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|e| conversion_failure(1, Type::Text, e))?,
        version: positive_u32(row, 2)?,
        task_generation: positive_u64(row, 3)?,
        supersedes_contract_id: row
            .get::<_, Option<String>>(4)?
            .map(TaskContractId::new)
            .transpose()
            .map_err(|e| conversion_failure(4, Type::Text, e))?,
        origin: ContractOrigin::from_str(&row.get::<_, String>(5)?)
            .map_err(|e| conversion_failure(5, Type::Text, e))?,
        request_markdown: row.get(6)?,
        execution_plan_markdown: row.get(7)?,
        complexity: TaskComplexity::from_str(&row.get::<_, String>(8)?)
            .map_err(|e| conversion_failure(8, Type::Text, e))?,
        execution_policy,
        workspace_context: WorkspaceContextSnapshot {
            workspace_id: WorkspaceId::new(row.get::<_, String>(15)?)
                .map_err(|e| conversion_failure(15, Type::Text, e))?,
            name: row.get(16)?,
            description: row.get(17)?,
        },
        project_context,
        executor: TaskExecutorSelection {
            agent_id: row.get(23)?,
            backend,
            acp,
        },
        effective_cwd: row.get(26)?,
        created_by_actor_id: row.get(27)?,
        created_at: row.get(28)?,
    })
}

fn load_contract_criteria(
    transaction: &Transaction<'_>,
    contract_id: &TaskContractId,
) -> Result<Vec<TaskValidationCriterion>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT criterion_id, ordinal, description, expected_evidence
         FROM task_contract_criteria WHERE contract_id = ?1
         ORDER BY ordinal, criterion_id LIMIT 101",
    )?;
    let rows = statement.query_map([contract_id.as_str()], |row| {
        Ok(TaskValidationCriterion {
            criterion_id: row.get(0)?,
            ordinal: positive_u32(row, 1)?,
            description: row.get(2)?,
            expected_evidence: row.get(3)?,
        })
    })?;
    bounded(rows.collect::<Result<Vec<_>, _>>()?, "contract criteria")
}

pub(crate) fn load_submission(
    transaction: &Transaction<'_>,
    submission_id: &str,
    _expected_criteria: &[String],
) -> Result<TaskSubmissionRecord, StoreError> {
    super::submission_batch::load_submissions(transaction, &[submission_id.to_string()])?
        .remove(submission_id)
        .ok_or_else(|| super::rows::missing_link("submission", submission_id))
}

pub(crate) fn load_review(
    transaction: &Transaction<'_>,
    review_id: &str,
    _expected_criteria: &[String],
) -> Result<TaskReviewRecord, StoreError> {
    super::list_rows::load_reviews(transaction, &[review_id.to_string()])?
        .remove(review_id)
        .ok_or_else(|| super::rows::missing_link("review", review_id))
}
fn bounded<T>(rows: Vec<T>, kind: &'static str) -> Result<Vec<T>, StoreError> {
    if rows.len() > EMBEDDED_LIMIT {
        Err(StoreError::InvariantViolation {
            message: format!("{kind} exceed the embedded detail limit"),
        })
    } else {
        Ok(rows)
    }
}

fn noncanonical(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
