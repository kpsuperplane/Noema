//! V3 agent-run row decoding used by the semantic Work writer.

use std::str::FromStr;

use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{
    AgentRunRecord, RunKind, RunStatus, TaskContractId, TaskExecutionPolicy, TaskId,
    WorkDomainError,
};
use rusqlite::{OptionalExtension, Row, Transaction};

use crate::{StoreError, work_commands::helpers};

/// Load one V3 run while retaining the policy snapshot that was in force when
/// the run was queued.  Policy columns are deliberately not inferred from the
/// current global settings for contract-bound runs.
pub(crate) fn load_run_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<AgentRunRecord>, StoreError> {
    let raw = transaction
        .query_row(
            r#"SELECT run_id, task_id, task_generation, contract_id, run_kind, agent_id,
                      attempt_index, review_round, parent_run_id, triggering_submission_id,
                      triggering_review_id, provider_kind, provider_account_id,
                      provider_instance_key, selection_mode, model_profile, reasoning_effort,
                      selection_source, max_provider_continuations, max_tool_calls,
                      max_active_minutes, progress_audit_interval, max_automatic_retries,
                      max_review_rounds, actual_provider_kind, actual_model_profile, status,
                      queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at,
                      started_at, ended_at, cancellation_requested, error_code, error_message,
                      provider_call_count, tool_call_count, input_tokens, cached_input_tokens,
                      output_tokens, active_milliseconds, created_at, updated_at
                 FROM agent_runs WHERE run_id = ?1"#,
            [run_id],
            raw_run_from_row,
        )
        .optional()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let contract_id = raw
        .contract_id
        .as_deref()
        .map(TaskContractId::new)
        .transpose()
        .map_err(StoreError::Work)?;
    let model = provider_snapshot(&raw)?;
    let policy = load_run_policy(&raw)?;
    let run = AgentRunRecord {
        run_id: raw.run_id.clone(),
        task_id: TaskId::new(raw.task_id.clone()).map_err(StoreError::Work)?,
        task_generation: helpers::positive_u64(raw.task_generation, "run.task_generation")?,
        contract_id,
        run_kind: RunKind::from_str(&raw.run_kind).map_err(StoreError::Work)?,
        agent_id: raw.agent_id.clone(),
        attempt_index: helpers::nonnegative_u32(raw.attempt_index, "run.attempt_index")?,
        review_round: helpers::nonnegative_u32(raw.review_round, "run.review_round")?,
        parent_run_id: raw.parent_run_id.clone(),
        triggering_submission_id: raw.triggering_submission_id.clone(),
        triggering_review_id: raw.triggering_review_id.clone(),
        model,
        actual_provider_kind: raw.actual_provider_kind.clone(),
        actual_model_profile: raw.actual_model_profile.clone(),
        execution_policy: policy,
        status: RunStatus::from_str(&raw.status).map_err(StoreError::Work)?,
        queued_at: raw.queued_at.clone(),
        lease_owner: raw.lease_owner.clone(),
        lease_token: raw.lease_token.clone(),
        lease_expires_at: raw.lease_expires_at.clone(),
        heartbeat_at: raw.heartbeat_at.clone(),
        started_at: raw.started_at.clone(),
        ended_at: raw.ended_at.clone(),
        cancellation_requested: raw.cancellation_requested != 0,
        error_code: raw.error_code.clone(),
        error_message: raw.error_message.clone(),
        provider_call_count: helpers::nonnegative_u32(
            raw.provider_call_count,
            "run.provider_call_count",
        )?,
        tool_call_count: helpers::nonnegative_u32(raw.tool_call_count, "run.tool_call_count")?,
        input_tokens: helpers::nonnegative_u64(raw.input_tokens, "run.input_tokens")?,
        cached_input_tokens: helpers::nonnegative_u64(
            raw.cached_input_tokens,
            "run.cached_input_tokens",
        )?,
        output_tokens: helpers::nonnegative_u64(raw.output_tokens, "run.output_tokens")?,
        active_milliseconds: helpers::nonnegative_u64(
            raw.active_milliseconds,
            "run.active_milliseconds",
        )?,
        created_at: raw.created_at.clone(),
        updated_at: raw.updated_at.clone(),
    };
    run.validate_contract_lineage().map_err(StoreError::Work)?;
    Ok(Some(run))
}

fn load_run_policy(raw: &RawRun) -> Result<TaskExecutionPolicy, StoreError> {
    let policy = TaskExecutionPolicy {
        max_provider_continuations: helpers::positive_u32(
            raw.max_provider_continuations,
            "run.max_provider_continuations",
        )?,
        max_tool_calls: helpers::positive_u32(raw.max_tool_calls, "run.max_tool_calls")?,
        max_active_minutes: helpers::positive_u32(
            raw.max_active_minutes,
            "run.max_active_minutes",
        )?,
        progress_audit_interval: helpers::positive_u32(
            raw.progress_audit_interval,
            "run.progress_audit_interval",
        )?,
        max_automatic_retries: helpers::nonnegative_u32(
            raw.max_automatic_retries,
            "run.max_automatic_retries",
        )?,
        max_review_rounds: helpers::positive_u32(raw.max_review_rounds, "run.max_review_rounds")?,
    };
    policy.validated().map_err(StoreError::Work)
}

fn provider_snapshot(raw: &RawRun) -> Result<ProviderSelectionSnapshot, StoreError> {
    Ok(ProviderSelectionSnapshot {
        provider_kind: raw.provider_kind.clone(),
        provider_account_id: raw.provider_account_id.clone(),
        provider_instance_key: Some(
            ProviderInstanceKey::new(raw.provider_instance_key.clone()).map_err(|error| {
                StoreError::Work(WorkDomainError::InvalidInput {
                    field: "run.provider_instance_key",
                    message: error.to_string(),
                })
            })?,
        ),
        selection_mode: ProviderSelectionMode::from_str(&raw.selection_mode).map_err(|error| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.selection_mode",
                message: error.to_string(),
            })
        })?,
        model_profile: raw.model_profile.clone(),
        reasoning_effort: raw
            .reasoning_effort
            .as_ref()
            .map(|value| {
                ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                    StoreError::Work(WorkDomainError::InvalidInput {
                        field: "run.reasoning_effort",
                        message: format!("unknown value {value}"),
                    })
                })
            })
            .transpose()?,
        selection_source: raw.selection_source.clone(),
    })
}

#[derive(Debug)]
struct RawRun {
    run_id: String,
    task_id: String,
    task_generation: i64,
    contract_id: Option<String>,
    run_kind: String,
    agent_id: String,
    attempt_index: i64,
    review_round: i64,
    parent_run_id: Option<String>,
    triggering_submission_id: Option<String>,
    triggering_review_id: Option<String>,
    provider_kind: String,
    provider_account_id: String,
    provider_instance_key: String,
    selection_mode: String,
    model_profile: Option<String>,
    reasoning_effort: Option<String>,
    selection_source: Option<String>,
    max_provider_continuations: i64,
    max_tool_calls: i64,
    max_active_minutes: i64,
    progress_audit_interval: i64,
    max_automatic_retries: i64,
    max_review_rounds: i64,
    actual_provider_kind: Option<String>,
    actual_model_profile: Option<String>,
    status: String,
    queued_at: String,
    lease_owner: Option<String>,
    lease_token: Option<String>,
    lease_expires_at: Option<String>,
    heartbeat_at: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    cancellation_requested: i64,
    error_code: Option<String>,
    error_message: Option<String>,
    provider_call_count: i64,
    tool_call_count: i64,
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    active_milliseconds: i64,
    created_at: String,
    updated_at: String,
}

fn raw_run_from_row(row: &Row<'_>) -> rusqlite::Result<RawRun> {
    Ok(RawRun {
        run_id: row.get(0)?,
        task_id: row.get(1)?,
        task_generation: row.get(2)?,
        contract_id: row.get(3)?,
        run_kind: row.get(4)?,
        agent_id: row.get(5)?,
        attempt_index: row.get(6)?,
        review_round: row.get(7)?,
        parent_run_id: row.get(8)?,
        triggering_submission_id: row.get(9)?,
        triggering_review_id: row.get(10)?,
        provider_kind: row.get(11)?,
        provider_account_id: row.get(12)?,
        provider_instance_key: row.get(13)?,
        selection_mode: row.get(14)?,
        model_profile: row.get(15)?,
        reasoning_effort: row.get(16)?,
        selection_source: row.get(17)?,
        max_provider_continuations: row.get(18)?,
        max_tool_calls: row.get(19)?,
        max_active_minutes: row.get(20)?,
        progress_audit_interval: row.get(21)?,
        max_automatic_retries: row.get(22)?,
        max_review_rounds: row.get(23)?,
        actual_provider_kind: row.get(24)?,
        actual_model_profile: row.get(25)?,
        status: row.get(26)?,
        queued_at: row.get(27)?,
        lease_owner: row.get(28)?,
        lease_token: row.get(29)?,
        lease_expires_at: row.get(30)?,
        heartbeat_at: row.get(31)?,
        started_at: row.get(32)?,
        ended_at: row.get(33)?,
        cancellation_requested: row.get(34)?,
        error_code: row.get(35)?,
        error_message: row.get(36)?,
        provider_call_count: row.get(37)?,
        tool_call_count: row.get(38)?,
        input_tokens: row.get(39)?,
        cached_input_tokens: row.get(40)?,
        output_tokens: row.get(41)?,
        active_milliseconds: row.get(42)?,
        created_at: row.get(43)?,
        updated_at: row.get(44)?,
    })
}
