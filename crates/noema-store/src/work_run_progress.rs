//! Lease-fenced, additive run usage and provider-observation persistence.

use noema_tasks::{AgentRunRecord, RunStatus, WorkDomainError, WorkEventPayload};
use rusqlite::params;

use super::{WorkRunProgress, rows::load_run_tx};
use crate::{
    StoreError,
    work_commands::{WorkCommandService, helpers},
    work_events::{WorkEventScope, append_work_event_tx},
};

impl WorkCommandService {
    /// Atomically add one run-progress observation and append `run.heartbeat`.
    ///
    /// # Errors
    ///
    /// Returns an error when progress validation or lease fencing fails, or the
    /// observation/event cannot be committed atomically.
    pub async fn record_work_run_progress(
        &self,
        progress: WorkRunProgress,
    ) -> Result<AgentRunRecord, StoreError> {
        progress.validate().map_err(StoreError::Work)?;
        self.store
            .with_immediate_transaction_retry(|transaction| {
                let run = load_run_tx(transaction, &progress.fence.run_id)?
                    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                if run.status != RunStatus::Running {
                    return Err(StoreError::Work(WorkDomainError::InvalidTransition));
                }
                if run.task_generation != progress.fence.task_generation {
                    return Err(StoreError::Work(WorkDomainError::StaleGeneration));
                }
                if run.contract_id != progress.fence.contract_id
                    || run.lease_token.as_deref() != Some(progress.fence.lease_token.as_str())
                {
                    return Err(StoreError::Work(WorkDomainError::RunFenced));
                }
                let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
                if task.generation != progress.fence.task_generation {
                    return Err(StoreError::Work(WorkDomainError::StaleGeneration));
                }
                let provider_call_count = add_u32(
                    run.provider_call_count,
                    progress.provider_call_count_delta,
                    "run_progress.provider_call_count",
                )?;
                let tool_call_count = add_u32(
                    run.tool_call_count,
                    progress.tool_call_count_delta,
                    "run_progress.tool_call_count",
                )?;
                let input_tokens = add_sql_u64(
                    run.input_tokens,
                    progress.input_tokens_delta,
                    "run_progress.input_tokens",
                )?;
                let cached_input_tokens = add_sql_u64(
                    run.cached_input_tokens,
                    progress.cached_input_tokens_delta,
                    "run_progress.cached_input_tokens",
                )?;
                let output_tokens = add_sql_u64(
                    run.output_tokens,
                    progress.output_tokens_delta,
                    "run_progress.output_tokens",
                )?;
                let active_milliseconds = add_sql_u64(
                    run.active_milliseconds,
                    progress.active_milliseconds_delta,
                    "run_progress.active_milliseconds",
                )?;
                let actual_provider_kind = progress
                    .actual_provider_kind
                    .as_deref()
                    .or(run.actual_provider_kind.as_deref());
                let actual_model_profile = progress
                    .actual_model_profile
                    .as_deref()
                    .or(run.actual_model_profile.as_deref());
                let changed = transaction.execute(
                    "UPDATE agent_runs SET
                        actual_provider_kind = ?2, actual_model_profile = ?3,
                        provider_call_count = ?4, tool_call_count = ?5,
                        input_tokens = ?6, cached_input_tokens = ?7, output_tokens = ?8,
                        active_milliseconds = ?9,
                        heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE run_id = ?1 AND lease_token = ?10 AND status = 'running'
                       AND cancellation_requested = 0
                       AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                       AND task_generation = ?11
                       AND ((contract_id IS NULL AND ?12 IS NULL) OR contract_id = ?12)
                       AND EXISTS (
                         SELECT 1 FROM tasks task
                         WHERE task.task_id = agent_runs.task_id AND task.generation = ?11
                       )",
                    params![
                        run.run_id,
                        actual_provider_kind,
                        actual_model_profile,
                        i64::from(provider_call_count),
                        i64::from(tool_call_count),
                        input_tokens,
                        cached_input_tokens,
                        output_tokens,
                        active_milliseconds,
                        progress.fence.lease_token,
                        sql_u64(
                            progress.fence.task_generation,
                            "run_progress.task_generation"
                        )?,
                        progress.fence.contract_id.as_ref().map(ToString::to_string),
                    ],
                )?;
                if changed != 1 {
                    return Err(StoreError::Work(WorkDomainError::RunFenced));
                }
                let updated = load_run_tx(transaction, &run.run_id)?
                    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                append_work_event_tx(
                    transaction,
                    WorkEventScope {
                        workspace_id: task.workspace_id,
                        project_id: task.project_id,
                        task_id: Some(task.task_id),
                        run_id: Some(updated.run_id.clone()),
                        actor_id: "actor:store:run-progress".to_string(),
                        causation_id: None,
                        correlation_id: format!("correlation:run:{}", updated.run_id),
                    },
                    WorkEventPayload::run_heartbeat(
                        updated.run_kind,
                        updated.task_generation,
                        updated.provider_call_count,
                        updated.tool_call_count,
                        updated.active_milliseconds,
                    )
                    .map_err(StoreError::Work)?,
                )?;
                Ok(updated)
            })
            .await
    }
}

fn add_u32(current: u32, delta: u32, field: &'static str) -> Result<u32, StoreError> {
    current.checked_add(delta).ok_or_else(|| overflow(field))
}

fn add_sql_u64(current: u64, delta: u64, field: &'static str) -> Result<i64, StoreError> {
    let value = current.checked_add(delta).ok_or_else(|| overflow(field))?;
    sql_u64(value, field)
}

fn sql_u64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| overflow(field))
}

fn overflow(field: &'static str) -> StoreError {
    StoreError::Work(WorkDomainError::InvalidInput {
        field,
        message: "aggregate exceeds the durable SQLite/domain range".to_string(),
    })
}
