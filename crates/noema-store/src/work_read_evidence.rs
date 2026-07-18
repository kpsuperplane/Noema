use std::{collections::HashMap, str::FromStr};

use noema_tasks::{
    ContractOrigin, CriterionOutcome, NewTaskReview, NewTaskSubmission,
    SubmissionCriterionEvidence, TaskComplexity, TaskContractId, TaskExecutionContract,
    TaskExecutionPolicy, TaskGateKind, TaskId, TaskReviewCriterion, TaskReviewRecord,
    TaskReviewVerdict, TaskSubmissionRecord, TaskValidationCriterion, WorkspaceContextSnapshot,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Row, Transaction, types::Type};

use crate::{StoreError, sqlite::conversion_failure};

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

/// Hydrate a bounded contract page in three fixed queries regardless of page size.
pub(crate) fn load_contracts(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskExecutionContract>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT contract_id, task_id, version, task_generation, supersedes_contract_id,
                origin, request_markdown, execution_plan_markdown, complexity,
                max_provider_continuations, max_tool_calls, max_active_minutes,
                progress_audit_interval, max_automatic_retries, max_review_rounds,
                workspace_id_snapshot, workspace_name_snapshot, workspace_description_snapshot,
                project_id_snapshot, project_name_snapshot, project_description_snapshot,
                created_by_actor_id, created_at
         FROM task_execution_contracts
         WHERE contract_id IN (SELECT value FROM json_each(?1))",
    )?;
    let cores = statement
        .query_map([&ids_json], decode_contract_core)?
        .collect::<Result<Vec<_>, _>>()?;
    if cores.len() != ids.len() {
        return Err(noncanonical(
            "contract history references a missing contract",
        ));
    }
    let criteria = load_contract_criteria_batch(transaction, &ids_json, ids)?;
    let models = load_contract_models_batch(transaction, &ids_json)?;
    let mut records = HashMap::with_capacity(cores.len());
    for core in cores {
        let contract_key = core.contract_id.as_str().to_string();
        let contract_criteria = criteria.get(&contract_key).cloned().unwrap_or_default();
        let (executor_model, reviewer_model) = models
            .get(&contract_key)
            .cloned()
            .ok_or_else(|| noncanonical("contract history is missing model snapshots"))?;
        let contract = TaskExecutionContract {
            contract_id: core.contract_id,
            task_id: core.task_id,
            version: core.version,
            task_generation: core.task_generation,
            supersedes_contract_id: core.supersedes_contract_id,
            origin: core.origin,
            request_markdown: core.request_markdown,
            execution_plan_markdown: core.execution_plan_markdown,
            criteria: contract_criteria,
            complexity: core.complexity,
            executor_model,
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
        if records.insert(contract_key, contract).is_some() {
            return Err(noncanonical("duplicate contract in history batch"));
        }
    }
    Ok(records)
}

fn load_contract_criteria_batch(
    transaction: &Transaction<'_>,
    ids_json: &str,
    ids: &[String],
) -> Result<HashMap<String, Vec<TaskValidationCriterion>>, StoreError> {
    let mut grouped = ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect::<HashMap<_, _>>();
    let mut statement = transaction.prepare(
        "SELECT contract_id, criterion_id, ordinal, description, expected_evidence
         FROM task_contract_criteria
         WHERE contract_id IN (SELECT value FROM json_each(?1))
         ORDER BY contract_id, ordinal, criterion_id",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok((
            row.get::<_, String>(0)?,
            TaskValidationCriterion {
                criterion_id: row.get(1)?,
                ordinal: positive_u32(row, 2)?,
                description: row.get(3)?,
                expected_evidence: row.get(4)?,
            },
        ))
    })?;
    for row in rows {
        let (contract_id, criterion) = row?;
        let values = grouped
            .get_mut(&contract_id)
            .ok_or_else(|| noncanonical("criterion crosses contract history batch"))?;
        if values.len() == EMBEDDED_LIMIT {
            return Err(noncanonical(
                "contract criteria exceed the embedded detail limit",
            ));
        }
        values.push(criterion);
    }
    Ok(grouped)
}

type ModelPair = (
    noema_providers::ProviderSelectionSnapshot,
    noema_providers::ProviderSelectionSnapshot,
);

fn load_contract_models_batch(
    transaction: &Transaction<'_>,
    ids_json: &str,
) -> Result<HashMap<String, ModelPair>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT contract_id,
                executor_provider_kind, executor_provider_account_id,
                executor_provider_instance_key, executor_selection_mode,
                executor_model_profile, executor_reasoning_effort, executor_selection_source,
                reviewer_provider_kind, reviewer_provider_account_id,
                reviewer_provider_instance_key, reviewer_selection_mode,
                reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source
         FROM task_execution_contracts
         WHERE contract_id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok((
            row.get::<_, String>(0)?,
            (
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ),
            (
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
            ),
        ))
    })?;
    let mut models = HashMap::new();
    for row in rows {
        let (id, executor, reviewer) = row?;
        let pair = (
            crate::work_commands::helpers::provider_snapshot(executor)?,
            crate::work_commands::helpers::provider_snapshot(reviewer)?,
        );
        if models.insert(id, pair).is_some() {
            return Err(noncanonical("duplicate contract model row"));
        }
    }
    Ok(models)
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
    created_by_actor_id: String,
    created_at: String,
}

fn decode_contract_core(row: &Row<'_>) -> rusqlite::Result<ContractCore> {
    let project_context = match (
        row.get::<_, Option<String>>(18)?,
        row.get::<_, Option<String>>(19)?,
        row.get::<_, Option<String>>(20)?,
    ) {
        (None, None, None) => None,
        (Some(id), Some(name), Some(description)) => Some(noema_tasks::ProjectContextSnapshot {
            project_id: ProjectId::new(id).map_err(|e| conversion_failure(18, Type::Text, e))?,
            name,
            description,
        }),
        _ => return Err(noncanonical_sql(18, "partial project context snapshot")),
    };
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
        created_by_actor_id: row.get(21)?,
        created_at: row.get(22)?,
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
    expected_criteria: &[String],
) -> Result<TaskSubmissionRecord, StoreError> {
    let Some(base) = transaction
        .query_row(
            "SELECT submission_id, task_id, contract_id, executor_run_id, review_round,
                    summary, result_markdown, created_at
             FROM task_submissions WHERE submission_id = ?1 LIMIT 1",
            [submission_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    TaskId::new(row.get::<_, String>(1)?)
                        .map_err(|e| conversion_failure(1, Type::Text, e))?,
                    TaskContractId::new(row.get::<_, String>(2)?)
                        .map_err(|e| conversion_failure(2, Type::Text, e))?,
                    row.get::<_, String>(3)?,
                    positive_u32(row, 4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?
    else {
        return Err(super::rows::missing_link("submission", submission_id));
    };
    let criteria = load_submission_criteria(transaction, submission_id)?;
    let artifacts = load_submission_artifacts(transaction, submission_id, &base.1)?;
    let input = NewTaskSubmission {
        submission_id: Some(base.0.clone()),
        task_id: base.1.clone(),
        contract_id: base.2.clone(),
        executor_run_id: base.3.clone(),
        review_round: base.4,
        summary: base.5.clone(),
        result_markdown: base.6.clone(),
        criteria: criteria.clone(),
        artifact_ids: artifacts
            .iter()
            .map(|link| link.artifact.artifact_id.clone())
            .collect(),
    };
    if input
        .normalized(expected_criteria)
        .map_err(StoreError::Work)?
        != input
        || base.7.trim().is_empty()
    {
        return Err(noncanonical("submission row is not canonically normalized"));
    }
    Ok(TaskSubmissionRecord {
        submission_id: base.0,
        task_id: base.1,
        contract_id: base.2,
        executor_run_id: base.3,
        review_round: base.4,
        summary: base.5,
        result_markdown: base.6,
        criteria,
        artifacts,
        created_at: base.7,
    })
}

fn load_submission_criteria(
    transaction: &Transaction<'_>,
    submission_id: &str,
) -> Result<Vec<SubmissionCriterionEvidence>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT criterion_id, evidence_markdown FROM task_submission_criteria
         WHERE submission_id = ?1 ORDER BY criterion_id LIMIT 101",
    )?;
    let rows = statement.query_map([submission_id], |row| {
        Ok(SubmissionCriterionEvidence {
            criterion_id: row.get(0)?,
            evidence_markdown: row.get(1)?,
        })
    })?;
    bounded(rows.collect::<Result<Vec<_>, _>>()?, "submission criteria")
}

fn load_submission_artifacts(
    transaction: &Transaction<'_>,
    submission_id: &str,
    task_id: &TaskId,
) -> Result<Vec<noema_tasks::TaskSubmissionArtifactRecord>, StoreError> {
    use crate::artifacts::{
        ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, artifact_from_row, artifact_row,
        artifact_version_from_row, artifact_version_row,
    };
    let mut statement = transaction.prepare(
        "SELECT ordinal, artifact_id, artifact_version_id FROM task_submission_artifacts
         WHERE submission_id = ?1 ORDER BY ordinal LIMIT 101",
    )?;
    let links = bounded(
        statement
            .query_map([submission_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?,
        "submission artifacts",
    )?;
    links
        .into_iter()
        .map(|(ordinal, artifact_id, version_id)| {
            let artifact = transaction.query_row(
                &format!("SELECT {ARTIFACT_SELECT} FROM artifacts WHERE artifact_id = ?1 LIMIT 1"),
                [&artifact_id],
                artifact_row,
            )?;
            let version = transaction.query_row(
                &format!("SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"),
                [&version_id],
                artifact_version_row,
            )?;
            let artifact = artifact_from_row(artifact)?;
            let version = artifact_version_from_row(version)?;
            if artifact.owner.object_type != "task"
                || artifact.owner.object_id != task_id.as_str()
                || version.artifact_id != artifact.artifact_id
            {
                return Err(noncanonical("submission artifact crosses an ownership or version link"));
            }
            Ok(noema_tasks::TaskSubmissionArtifactRecord {
                ordinal: u32::try_from(ordinal)
                    .ok()
                    .filter(|ordinal| *ordinal > 0)
                    .ok_or_else(|| noncanonical("invalid artifact ordinal"))?,
                artifact,
                version,
            })
        })
        .collect()
}

pub(crate) fn load_review(
    transaction: &Transaction<'_>,
    review_id: &str,
    expected_criteria: &[String],
) -> Result<TaskReviewRecord, StoreError> {
    let Some(base) = transaction
        .query_row(
            "SELECT review_id, task_id, contract_id, reviewer_run_id, reviewed_submission_id,
                    review_attempt_index, supersedes_review_id, overall_verdict,
                    human_gate_kind, overall_feedback, created_at
             FROM task_reviews WHERE review_id = ?1 LIMIT 1",
            [review_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    TaskId::new(row.get::<_, String>(1)?)
                        .map_err(|e| conversion_failure(1, Type::Text, e))?,
                    TaskContractId::new(row.get::<_, String>(2)?)
                        .map_err(|e| conversion_failure(2, Type::Text, e))?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    positive_u32(row, 5)?,
                    row.get::<_, Option<String>>(6)?,
                    TaskReviewVerdict::from_str(&row.get::<_, String>(7)?)
                        .map_err(|e| conversion_failure(7, Type::Text, e))?,
                    row.get::<_, Option<String>>(8)?
                        .map(|value| TaskGateKind::from_str(&value))
                        .transpose()
                        .map_err(|e| conversion_failure(8, Type::Text, e))?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .optional()?
    else {
        return Err(super::rows::missing_link("review", review_id));
    };
    let criteria = load_review_criteria(transaction, review_id)?;
    let input = NewTaskReview {
        review_id: Some(base.0.clone()),
        task_id: base.1.clone(),
        contract_id: base.2.clone(),
        reviewer_run_id: base.3.clone(),
        reviewed_submission_id: base.4.clone(),
        review_attempt_index: base.5,
        supersedes_review_id: base.6.clone(),
        overall_verdict: base.7,
        human_gate_kind: base.8,
        overall_feedback: base.9.clone(),
        criteria: criteria.clone(),
    };
    if input
        .normalized(expected_criteria)
        .map_err(StoreError::Work)?
        != input
        || base.10.trim().is_empty()
    {
        return Err(noncanonical("review row is not canonically normalized"));
    }
    Ok(TaskReviewRecord {
        review_id: base.0,
        task_id: base.1,
        contract_id: base.2,
        reviewer_run_id: base.3,
        reviewed_submission_id: base.4,
        review_attempt_index: base.5,
        supersedes_review_id: base.6,
        overall_verdict: base.7,
        human_gate_kind: base.8,
        overall_feedback: base.9,
        criteria,
        created_at: base.10,
    })
}

fn load_review_criteria(
    transaction: &Transaction<'_>,
    review_id: &str,
) -> Result<Vec<TaskReviewCriterion>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT criterion_id, outcome, evidence_markdown, feedback FROM task_review_criteria
         WHERE review_id = ?1 ORDER BY criterion_id LIMIT 101",
    )?;
    let rows = statement.query_map([review_id], |row| {
        Ok(TaskReviewCriterion {
            criterion_id: row.get(0)?,
            outcome: CriterionOutcome::from_str(&row.get::<_, String>(1)?)
                .map_err(|e| conversion_failure(1, Type::Text, e))?,
            evidence_markdown: row.get(2)?,
            feedback: row.get(3)?,
        })
    })?;
    bounded(rows.collect::<Result<Vec<_>, _>>()?, "review criteria")
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

fn positive_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = u64::try_from(row.get::<_, i64>(index)?)
        .map_err(|e| conversion_failure(index, Type::Integer, e))?;
    if value == 0 {
        Err(noncanonical_sql(index, "expected positive integer"))
    } else {
        Ok(value)
    }
}

fn positive_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = u32::try_from(row.get::<_, i64>(index)?)
        .map_err(|e| conversion_failure(index, Type::Integer, e))?;
    if value == 0 {
        Err(noncanonical_sql(index, "expected positive integer"))
    } else {
        Ok(value)
    }
}

fn nonnegative_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    u32::try_from(row.get::<_, i64>(index)?)
        .map_err(|e| conversion_failure(index, Type::Integer, e))
}

fn noncanonical(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}

fn noncanonical_sql(index: usize, message: &'static str) -> rusqlite::Error {
    conversion_failure(
        index,
        Type::Text,
        std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    )
}
