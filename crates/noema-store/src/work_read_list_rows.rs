use std::{collections::HashMap, str::FromStr};

use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{
    AcpExecutorLaunch, AgentRunRecord, RunKind, RunStatus, TaskExecutionPolicy,
    TaskExecutorBackend, TaskExecutorSelection, TaskGateRecord, TaskGateState, TaskId,
    WorkflowStage,
};
use rusqlite::{Row, Transaction, types::Type};

use super::{
    decode_project_record,
    rows::{decode_gate, decode_task_record},
};
use crate::{
    StoreError,
    sqlite::conversion_failure,
    work_row::{
        invalid as invalid_sql, nonnegative_u32, nonnegative_u64, positive_u32, positive_u64,
        strict_bool,
    },
};

pub(super) struct TaskPageRow {
    pub(super) task: noema_tasks::TaskRecord,
    pub(super) stage: WorkflowStage,
}

pub(super) fn decode_task_page_row(row: &Row<'_>) -> rusqlite::Result<TaskPageRow> {
    let task = decode_task_record(row)?;
    let stage = noema_tasks::personal_stage(&task.stage_id)
        .map_err(|error| conversion_failure(4, Type::Text, error))?;
    Ok(TaskPageRow { task, stage })
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
        "SELECT ar.run_id, ar.instance_name, ar.task_id, ar.task_generation, ar.run_kind,
                ar.agent_id, ar.attempt_index, ar.review_round, ar.parent_run_id, ar.provider_kind,
                ar.provider_account_id, ar.provider_instance_key, ar.selection_mode,
                ar.model_profile, ar.reasoning_effort, ar.selection_source,
                ar.actual_provider_kind, ar.actual_model_profile, ar.status, ar.queued_at,
                ar.lease_owner, ar.lease_token, ar.lease_expires_at, ar.heartbeat_at,
                ar.started_at, ar.ended_at, ar.cancellation_requested, ar.error_code,
                ar.error_message, ar.provider_call_count, ar.tool_call_count, ar.input_tokens,
                ar.cached_input_tokens, ar.output_tokens, ar.active_milliseconds,
                ar.created_at, ar.updated_at, ar.max_provider_continuations, ar.max_tool_calls,
                ar.max_active_minutes, ar.progress_audit_interval,
                ar.max_automatic_retries, ar.max_review_rounds,
                ar.execution_backend_kind, ar.acp_connection_revision, ar.acp_launch_json,
                ar.effective_cwd, ar.acp_session_id, ar.fast_mode
         FROM agent_runs ar
         WHERE ar.run_id IN (SELECT value FROM json_each(?1))",
    )?;
    let records = statement
        .query_map([ids_json], decode_run)?
        .collect::<Result<Vec<_>, _>>()?;
    collect_exact(ids, records, |run| run.run_id.clone(), "run")
}

fn decode_run(row: &Row<'_>) -> rusqlite::Result<AgentRunRecord> {
    let model = ProviderSelectionSnapshot {
        provider_kind: row.get(9)?,
        provider_account_id: row.get(10)?,
        provider_instance_key: Some(
            ProviderInstanceKey::new(row.get::<_, String>(11)?)
                .map_err(|error| conversion_failure(11, Type::Text, error))?,
        ),
        selection_mode: ProviderSelectionMode::from_str(&row.get::<_, String>(12)?)
            .map_err(|error| conversion_failure(12, Type::Text, error))?,
        model_profile: row.get(13)?,
        reasoning_effort: row
            .get::<_, Option<String>>(14)?
            .map(|value| {
                ReasoningEffort::from_persistence_str(&value)
                    .ok_or_else(|| invalid_sql(14, "unknown reasoning effort"))
            })
            .transpose()?,
        fast_mode: strict_bool(row, 48)?,
        selection_source: row.get(15)?,
    };
    if model
        .normalized_for_persistence()
        .map_err(|error| conversion_failure(9, Type::Text, error))?
        != model
    {
        return Err(invalid_sql(9, "run model is not canonically normalized"));
    }
    let execution_policy = TaskExecutionPolicy {
        max_provider_continuations: positive_u32(row, 37)?,
        max_tool_calls: positive_u32(row, 38)?,
        max_active_minutes: positive_u32(row, 39)?,
        progress_audit_interval: positive_u32(row, 40)?,
        max_automatic_retries: nonnegative_u32(row, 41)?,
        max_review_rounds: positive_u32(row, 42)?,
    }
    .validated()
    .map_err(|error| conversion_failure(37, Type::Integer, error))?;
    let backend = TaskExecutorBackend::from_str(&row.get::<_, String>(43)?)
        .map_err(|error| conversion_failure(43, Type::Text, error))?;
    let acp_revision = row.get::<_, Option<i64>>(44)?;
    let acp = row
        .get::<_, Option<String>>(45)?
        .map(|json| serde_json::from_str::<AcpExecutorLaunch>(&json))
        .transpose()
        .map_err(|error| conversion_failure(45, Type::Text, error))?;
    if acp
        .as_ref()
        .and_then(|snapshot| i64::try_from(snapshot.connection_revision).ok())
        != acp_revision
    {
        return Err(invalid_sql(
            44,
            "run ACP revision does not match launch snapshot",
        ));
    }
    let record = AgentRunRecord {
        run_id: row.get(0)?,
        instance_name: row.get(1)?,
        task_id: TaskId::new(row.get::<_, String>(2)?)
            .map_err(|error| conversion_failure(2, Type::Text, error))?,
        task_generation: positive_u64(row, 3)?,
        run_kind: RunKind::from_str(&row.get::<_, String>(4)?)
            .map_err(|error| conversion_failure(4, Type::Text, error))?,
        agent_id: row.get(5)?,
        attempt_index: nonnegative_u32(row, 6)?,
        review_round: nonnegative_u32(row, 7)?,
        parent_run_id: row.get(8)?,
        model,
        executor: TaskExecutorSelection {
            agent_id: row.get(5)?,
            backend,
            acp,
        },
        effective_cwd: row.get(46)?,
        acp_session_id: row.get(47)?,
        actual_provider_kind: row.get(16)?,
        actual_model_profile: row.get(17)?,
        execution_policy,
        status: RunStatus::from_str(&row.get::<_, String>(18)?)
            .map_err(|error| conversion_failure(18, Type::Text, error))?,
        queued_at: row.get(19)?,
        lease_owner: row.get(20)?,
        lease_token: row.get(21)?,
        lease_expires_at: row.get(22)?,
        heartbeat_at: row.get(23)?,
        started_at: row.get(24)?,
        ended_at: row.get(25)?,
        cancellation_requested: strict_bool(row, 26)?,
        error_code: row.get(27)?,
        error_message: row.get(28)?,
        provider_call_count: nonnegative_u32(row, 29)?,
        tool_call_count: nonnegative_u32(row, 30)?,
        input_tokens: nonnegative_u64(row, 31)?,
        cached_input_tokens: nonnegative_u64(row, 32)?,
        output_tokens: nonnegative_u64(row, 33)?,
        active_milliseconds: nonnegative_u64(row, 34)?,
        created_at: row.get(35)?,
        updated_at: row.get(36)?,
    };
    record
        .validate_lineage()
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
        "SELECT gate_id, task_id, task_generation, gate_kind, gate_state,
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

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
