use std::{collections::HashMap, str::FromStr};

use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{
    AcpExecutorSnapshot, AgentRunRecord, CriterionOutcome, NewTaskReview, RunKind, RunStatus,
    TaskContractId, TaskExecutionPolicy, TaskExecutorBackend, TaskExecutorSelection,
    TaskGateRecord, TaskGateState, TaskId, TaskReviewCriterion, TaskReviewRecord,
    TaskReviewVerdict, WorkflowStage,
};
use rusqlite::{Row, Transaction, types::Type};

use super::{
    decode_project_record,
    evidence::load_contract_criterion_ids,
    rows::{decode_gate, decode_stage_record, decode_task_record},
};
use crate::{
    StoreError,
    sqlite::conversion_failure,
    work_row::{
        invalid as invalid_sql, nonnegative_u32, nonnegative_u64, optional_id, positive_u32,
        positive_u64, strict_bool,
    },
};

pub(super) struct TaskPageRow {
    pub(super) task: noema_tasks::TaskRecord,
    pub(super) stage: WorkflowStage,
}

pub(super) fn decode_task_page_row(row: &Row<'_>) -> rusqlite::Result<TaskPageRow> {
    Ok(TaskPageRow {
        task: decode_task_record(row)?,
        stage: decode_stage_record(row, 38)?,
    })
}

pub(super) fn load_projects(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, noema_workspaces::ProjectRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT project_id, workspace_id, name, description, folder, revision, archived_at,
                created_at, updated_at
         FROM projects WHERE project_id IN (SELECT value FROM json_each(?1))",
    )?;
    let records = statement
        .query_map([ids_json], decode_project_record)?
        .collect::<Result<Vec<_>, _>>()?;
    collect_exact(
        ids,
        records,
        |project| project.project_id.as_str().to_string(),
        "project",
    )
}

pub(crate) fn load_runs(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, AgentRunRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT ar.run_id, ar.instance_name, ar.task_id, ar.task_generation, ar.contract_id, ar.run_kind,
                ar.agent_id, ar.attempt_index, ar.review_round, ar.parent_run_id,
                ar.triggering_submission_id, ar.triggering_review_id, ar.provider_kind,
                ar.provider_account_id, ar.provider_instance_key, ar.selection_mode,
                ar.model_profile, ar.reasoning_effort, ar.selection_source,
                ar.actual_provider_kind, ar.actual_model_profile, ar.status, ar.queued_at,
                ar.lease_owner, ar.lease_token, ar.lease_expires_at, ar.heartbeat_at,
                ar.started_at, ar.ended_at, ar.cancellation_requested, ar.error_code,
                ar.error_message, ar.provider_call_count, ar.tool_call_count, ar.input_tokens,
                ar.cached_input_tokens, ar.output_tokens, ar.active_milliseconds,
                ar.created_at, ar.updated_at, contract.contract_id,
                ar.max_provider_continuations, ar.max_tool_calls,
                ar.max_active_minutes, ar.progress_audit_interval,
                ar.max_automatic_retries, ar.max_review_rounds,
                ar.execution_backend_kind, ar.acp_connection_revision, ar.acp_launch_json,
                ar.effective_cwd, ar.acp_session_id, ar.fast_mode
         FROM agent_runs ar
         LEFT JOIN task_execution_contracts contract ON contract.contract_id = ar.contract_id
         WHERE ar.run_id IN (SELECT value FROM json_each(?1))",
    )?;
    let records = statement
        .query_map([ids_json], decode_run)?
        .collect::<Result<Vec<_>, _>>()?;
    collect_exact(ids, records, |run| run.run_id.clone(), "run")
}

fn decode_run(row: &Row<'_>) -> rusqlite::Result<AgentRunRecord> {
    let contract_id = optional_contract_id(row, 4)?;
    let joined_contract_id = row.get::<_, Option<String>>(40)?;
    if contract_id.as_ref().map(TaskContractId::as_str) != joined_contract_id.as_deref() {
        return Err(invalid_sql(40, "run references a missing contract"));
    }
    let model = ProviderSelectionSnapshot {
        provider_kind: row.get(12)?,
        provider_account_id: row.get(13)?,
        provider_instance_key: Some(
            ProviderInstanceKey::new(row.get::<_, String>(14)?)
                .map_err(|error| conversion_failure(14, Type::Text, error))?,
        ),
        selection_mode: ProviderSelectionMode::from_str(&row.get::<_, String>(15)?)
            .map_err(|error| conversion_failure(15, Type::Text, error))?,
        model_profile: row.get(16)?,
        reasoning_effort: row
            .get::<_, Option<String>>(17)?
            .map(|value| {
                ReasoningEffort::from_persistence_str(&value)
                    .ok_or_else(|| invalid_sql(17, "unknown reasoning effort"))
            })
            .transpose()?,
        fast_mode: strict_bool(row, 52)?,
        selection_source: row.get(18)?,
    };
    if model
        .normalized_for_persistence()
        .map_err(|error| conversion_failure(12, Type::Text, error))?
        != model
    {
        return Err(invalid_sql(12, "run model is not canonically normalized"));
    }
    let execution_policy = TaskExecutionPolicy {
        max_provider_continuations: positive_u32(row, 41)?,
        max_tool_calls: positive_u32(row, 42)?,
        max_active_minutes: positive_u32(row, 43)?,
        progress_audit_interval: positive_u32(row, 44)?,
        max_automatic_retries: nonnegative_u32(row, 45)?,
        max_review_rounds: positive_u32(row, 46)?,
    }
    .validated()
    .map_err(|error| conversion_failure(41, Type::Integer, error))?;
    let backend = TaskExecutorBackend::from_str(&row.get::<_, String>(47)?)
        .map_err(|error| conversion_failure(47, Type::Text, error))?;
    let acp_revision = row.get::<_, Option<i64>>(48)?;
    let acp = row
        .get::<_, Option<String>>(49)?
        .map(|json| serde_json::from_str::<AcpExecutorSnapshot>(&json))
        .transpose()
        .map_err(|error| conversion_failure(49, Type::Text, error))?;
    if acp
        .as_ref()
        .and_then(|snapshot| i64::try_from(snapshot.connection_revision).ok())
        != acp_revision
    {
        return Err(invalid_sql(
            48,
            "run ACP revision does not match launch snapshot",
        ));
    }
    let record = AgentRunRecord {
        run_id: row.get(0)?,
        instance_name: row.get(1)?,
        task_id: TaskId::new(row.get::<_, String>(2)?)
            .map_err(|error| conversion_failure(2, Type::Text, error))?,
        task_generation: positive_u64(row, 3)?,
        contract_id,
        run_kind: RunKind::from_str(&row.get::<_, String>(5)?)
            .map_err(|error| conversion_failure(5, Type::Text, error))?,
        agent_id: row.get(6)?,
        attempt_index: nonnegative_u32(row, 7)?,
        review_round: nonnegative_u32(row, 8)?,
        parent_run_id: row.get(9)?,
        triggering_submission_id: row.get(10)?,
        triggering_review_id: row.get(11)?,
        model,
        executor: TaskExecutorSelection {
            agent_id: row.get(6)?,
            backend,
            acp,
        },
        effective_cwd: row.get(50)?,
        acp_session_id: row.get(51)?,
        actual_provider_kind: row.get(19)?,
        actual_model_profile: row.get(20)?,
        execution_policy,
        status: RunStatus::from_str(&row.get::<_, String>(21)?)
            .map_err(|error| conversion_failure(21, Type::Text, error))?,
        queued_at: row.get(22)?,
        lease_owner: row.get(23)?,
        lease_token: row.get(24)?,
        lease_expires_at: row.get(25)?,
        heartbeat_at: row.get(26)?,
        started_at: row.get(27)?,
        ended_at: row.get(28)?,
        cancellation_requested: strict_bool(row, 29)?,
        error_code: row.get(30)?,
        error_message: row.get(31)?,
        provider_call_count: nonnegative_u32(row, 32)?,
        tool_call_count: nonnegative_u32(row, 33)?,
        input_tokens: nonnegative_u64(row, 34)?,
        cached_input_tokens: nonnegative_u64(row, 35)?,
        output_tokens: nonnegative_u64(row, 36)?,
        active_milliseconds: nonnegative_u64(row, 37)?,
        created_at: row.get(38)?,
        updated_at: row.get(39)?,
    };
    record
        .validate_contract_lineage()
        .map_err(|error| conversion_failure(0, Type::Text, error))?;
    Ok(record)
}

pub(super) fn load_gates(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskGateRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT gate_id, task_id, task_generation, contract_id, gate_kind, gate_state,
                recovery_reason, retry_run_kind, prompt_markdown, context_markdown,
                suggested_answers_json,
                opened_by_actor_id, originating_run_id, resolved_by_actor_id,
                resolution_message_id, opened_at, resolved_at
         FROM task_gates WHERE gate_id IN (SELECT value FROM json_each(?1))",
    )?;
    let records = statement
        .query_map([ids_json], |row| {
            let gate = decode_gate(row)?;
            gate.validate()
                .map_err(|error| conversion_failure(0, Type::Text, error))?;
            if gate.state != TaskGateState::Open {
                return Err(invalid_sql(0, "task active gate is not open"));
            }
            Ok(gate)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    collect_exact(
        ids,
        records,
        |gate| gate.gate_id.as_str().to_string(),
        "gate",
    )
}

pub(crate) fn load_reviews(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskReviewRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT review_id, task_id, contract_id, reviewer_run_id, reviewed_submission_id,
                review_attempt_index, supersedes_review_id, overall_verdict,
                human_gate_kind, overall_feedback, created_at
         FROM task_reviews WHERE review_id IN (SELECT value FROM json_each(?1))",
    )?;
    let bases = statement
        .query_map([&ids_json], decode_review_base)?
        .collect::<Result<Vec<_>, _>>()?;
    if bases.len() != ids.len() {
        return Err(invariant("task references a missing review"));
    }
    let criteria = load_review_criteria(transaction, &ids_json)?;
    let expected =
        load_contract_criterion_ids(transaction, bases.iter().map(|base| &base.contract_id))?;
    let mut records = Vec::with_capacity(bases.len());
    for base in bases {
        let review_criteria = criteria.get(&base.review_id).cloned().unwrap_or_default();
        let expected_criteria = expected
            .get(base.contract_id.as_str())
            .ok_or_else(|| invariant("review references a missing contract"))?;
        let input = NewTaskReview {
            review_id: Some(base.review_id.clone()),
            task_id: base.task_id.clone(),
            contract_id: base.contract_id.clone(),
            reviewer_run_id: base.reviewer_run_id.clone(),
            reviewed_submission_id: base.reviewed_submission_id.clone(),
            review_attempt_index: base.review_attempt_index,
            supersedes_review_id: base.supersedes_review_id.clone(),
            overall_verdict: base.overall_verdict,
            human_gate_kind: base.human_gate_kind,
            overall_feedback: base.overall_feedback.clone(),
            criteria: review_criteria.clone(),
        };
        if input
            .normalized(expected_criteria)
            .map_err(StoreError::Work)?
            != input
            || base.created_at.trim().is_empty()
        {
            return Err(invariant("review row is not canonically normalized"));
        }
        records.push(TaskReviewRecord {
            review_id: base.review_id,
            task_id: base.task_id,
            contract_id: base.contract_id,
            reviewer_run_id: base.reviewer_run_id,
            reviewed_submission_id: base.reviewed_submission_id,
            review_attempt_index: base.review_attempt_index,
            supersedes_review_id: base.supersedes_review_id,
            overall_verdict: base.overall_verdict,
            human_gate_kind: base.human_gate_kind,
            overall_feedback: base.overall_feedback,
            criteria: review_criteria,
            created_at: base.created_at,
        });
    }
    collect_exact(ids, records, |review| review.review_id.clone(), "review")
}

struct ReviewBase {
    review_id: String,
    task_id: TaskId,
    contract_id: TaskContractId,
    reviewer_run_id: String,
    reviewed_submission_id: String,
    review_attempt_index: u32,
    supersedes_review_id: Option<String>,
    overall_verdict: TaskReviewVerdict,
    human_gate_kind: Option<noema_tasks::TaskGateKind>,
    overall_feedback: String,
    created_at: String,
}

fn decode_review_base(row: &Row<'_>) -> rusqlite::Result<ReviewBase> {
    Ok(ReviewBase {
        review_id: row.get(0)?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|error| conversion_failure(1, Type::Text, error))?,
        contract_id: TaskContractId::new(row.get::<_, String>(2)?)
            .map_err(|error| conversion_failure(2, Type::Text, error))?,
        reviewer_run_id: row.get(3)?,
        reviewed_submission_id: row.get(4)?,
        review_attempt_index: positive_u32(row, 5)?,
        supersedes_review_id: row.get(6)?,
        overall_verdict: TaskReviewVerdict::from_str(&row.get::<_, String>(7)?)
            .map_err(|error| conversion_failure(7, Type::Text, error))?,
        human_gate_kind: row
            .get::<_, Option<String>>(8)?
            .map(|value| noema_tasks::TaskGateKind::from_str(&value))
            .transpose()
            .map_err(|error| conversion_failure(8, Type::Text, error))?,
        overall_feedback: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn load_review_criteria(
    transaction: &Transaction<'_>,
    ids_json: &str,
) -> Result<HashMap<String, Vec<TaskReviewCriterion>>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT review_id, criterion_id, outcome, evidence_markdown, feedback
         FROM task_review_criteria WHERE review_id IN (SELECT value FROM json_each(?1))
         ORDER BY review_id, criterion_id",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok((
            row.get::<_, String>(0)?,
            TaskReviewCriterion {
                criterion_id: row.get(1)?,
                outcome: CriterionOutcome::from_str(&row.get::<_, String>(2)?)
                    .map_err(|error| conversion_failure(2, Type::Text, error))?,
                evidence_markdown: row.get(3)?,
                feedback: row.get(4)?,
            },
        ))
    })?;
    let mut grouped = HashMap::<String, Vec<TaskReviewCriterion>>::new();
    for row in rows {
        let (review_id, criterion) = row?;
        let values = grouped.entry(review_id).or_default();
        if values.len() == 100 {
            return Err(invariant(
                "review criteria exceed the embedded detail limit",
            ));
        }
        values.push(criterion);
    }
    Ok(grouped)
}

fn collect_exact<T>(
    ids: &[String],
    records: Vec<T>,
    key: impl Fn(&T) -> String,
    kind: &'static str,
) -> Result<HashMap<String, T>, StoreError> {
    let mut map = HashMap::with_capacity(records.len());
    for record in records {
        if map.insert(key(&record), record).is_some() {
            return Err(invariant("duplicate row in a bounded task projection"));
        }
    }
    if map.len() != ids.len() || ids.iter().any(|id| !map.contains_key(id)) {
        return Err(StoreError::InvariantViolation {
            message: format!("task references a missing {kind}"),
        });
    }
    Ok(map)
}

fn optional_contract_id(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<TaskContractId>> {
    optional_id(row, index, TaskContractId::new)
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
