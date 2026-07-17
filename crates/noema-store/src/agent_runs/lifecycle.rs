use noema_tasks::{AgentRunHeartbeat, AgentRunRecord, RunStatus};
use rusqlite::{OptionalExtension, params};

use super::{
    RUN_COLUMNS,
    events::{append_run_event, append_task_event, ensure_fenced_write},
    recovery::{recover_expired_runs, recover_interrupted_runs},
};
use crate::{NoemaStore, StoreError, agent_run_rows::run_from_row, ids::now_string};

impl NoemaStore {
    /// Atomically recover expired leases and claim the oldest queued run.
    pub async fn claim_next_agent_run(
        &self,
        worker_id: &str,
        lease_token: &str,
        lease_seconds: i64,
    ) -> Result<Option<AgentRunRecord>, StoreError> {
        if worker_id.trim().is_empty() || lease_token.trim().is_empty() || lease_seconds < 1 {
            return Err(StoreError::InvariantViolation {
                message: "invalid agent run lease request".to_string(),
            });
        }
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now = now_string();
            let lease_expires_at = (now.parse::<i64>().unwrap_or_default() + lease_seconds).to_string();
            recover_expired_runs(&tx, &now)?;
            recover_interrupted_runs(&tx)?;
            let changed = tx.execute(
                "UPDATE agent_runs SET status = 'leased', lease_owner = ?1, lease_token = ?2, lease_expires_at = ?3, heartbeat_at = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = (SELECT queued.run_id FROM agent_runs queued WHERE queued.status = 'queued' AND NOT EXISTS (SELECT 1 FROM agent_runs active WHERE active.task_id = queued.task_id AND active.status IN ('leased', 'running')) ORDER BY queued.priority DESC, queued.queued_at, queued.run_id LIMIT 1) AND status = 'queued'",
                params![worker_id, lease_token, lease_expires_at, now],
            )?;
            if changed == 0 {
                tx.commit()?;
                return Ok(None);
            }
            let run = tx
                .query_row(&format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE lease_owner = ?1 AND lease_token = ?2 AND status = 'leased' ORDER BY updated_at DESC LIMIT 1"), params![worker_id, lease_token], run_from_row)
                .optional()?;
            if let Some(run) = &run {
                append_run_event(
                    &tx,
                    &run.run_id,
                    "run.leased",
                    serde_json::json!({
                        "worker_id": worker_id,
                        "lease_expires_at": run.lease_expires_at,
                    }),
                )?;
                append_task_event(
                    &tx,
                    &run.task_id,
                    "run.updated",
                    serde_json::json!({
                        "run_id": run.run_id,
                        "status": "leased",
                    }),
                )?;
            }
            tx.commit()?;
            Ok(run)
        }).await
    }

    /// Change a run state while enforcing the domain transition matrix.
    pub async fn transition_agent_run(
        &self,
        run_id: &str,
        next: RunStatus,
        lease_token: Option<&str>,
        error: Option<(String, String)>,
    ) -> Result<AgentRunRecord, StoreError> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let (current, task_id) = tx.query_row("SELECT status, task_id FROM agent_runs WHERE run_id = ?1", [run_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).optional()?.ok_or_else(|| StoreError::InvariantViolation { message: format!("agent run not found: {run_id}") })?;
            let current = current.parse::<RunStatus>().map_err(|error| StoreError::InvalidEnum { kind: "run_status", value: error.to_string() })?;
            if !current.can_transition_to(next) { return Err(StoreError::InvariantViolation { message: format!("invalid run transition {current} -> {next}") }); }
            let changed = if let Some(lease_token) = lease_token {
                tx.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = ?6 AND lease_token = ?5 AND (?2 = 'cancelled' OR cancellation_requested = 0)", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str()), lease_token, current.as_str()])?
            } else {
                tx.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = ?5 AND lease_token IS NULL AND status IN ('queued', 'waiting_for_approval', 'interrupted', 'failed') AND (?2 = 'cancelled' OR cancellation_requested = 0)", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str()), current.as_str()])?
            };
            if changed != 1 { return Err(StoreError::InvariantViolation { message: format!("agent run lease or state changed while updating: {run_id}") }); }
            append_run_event(
                &tx,
                run_id,
                &format!("run.{}", next.as_str()),
                serde_json::json!({
                    "from": current.as_str(),
                    "to": next.as_str(),
                    "error_code": error.as_ref().map(|value| value.0.as_str()),
                }),
            )?;
            append_task_event(
                &tx,
                &task_id,
                "run.updated",
                serde_json::json!({
                    "run_id": run_id,
                    "from": current.as_str(),
                    "status": next.as_str(),
                    "error_code": error.as_ref().map(|value| value.0.as_str()),
                }),
            )?;
            if next == RunStatus::Failed {
                let task_changed = tx.execute(
                    "UPDATE tasks SET status = 'failed', terminal_reason = ?3, error_code = ?2, error_message = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = (SELECT task_id FROM agent_runs WHERE run_id = ?1) AND latest_run_id = ?1 AND status NOT IN ('completed', 'cancelled')",
                    params![run_id, error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str())],
                )?;
                if task_changed == 1 {
                    append_task_event(
                        &tx,
                        &task_id,
                        "task.failed",
                        serde_json::json!({
                            "run_id": run_id,
                            "error_code": error.as_ref().map(|value| value.0.as_str()),
                        }),
                    )?;
                }
            }
            tx.commit()?;
            Ok(())
        }).await?;
        self.get_agent_run(run_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("agent run disappeared: {run_id}"),
            })
    }

    /// Record one completed provider call and add its usage to run totals.
    pub async fn record_agent_run_observation(
        &self,
        run_id: &str,
        lease_token: &str,
        actual_provider_kind: &str,
        actual_model_profile: &str,
        usage: Option<&noema_providers::TokenUsage>,
    ) -> Result<(), StoreError> {
        let changed = self
            .with_connection(|conn| {
                Ok(conn.execute(
                    "UPDATE agent_runs SET actual_provider_kind = ?3, actual_model_profile = ?4, provider_call_count = provider_call_count + 1, input_tokens = input_tokens + ?5, cached_input_tokens = cached_input_tokens + ?6, output_tokens = output_tokens + ?7, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?2 AND status = 'running' AND cancellation_requested = 0",
                    params![
                        run_id,
                        lease_token,
                        actual_provider_kind,
                        actual_model_profile,
                        usage.map_or(0, |value| i64::try_from(value.input_tokens).unwrap_or(i64::MAX)),
                        usage.and_then(|value| value.cached_input_tokens).map_or(0, |value| i64::try_from(value).unwrap_or(i64::MAX)),
                        usage.map_or(0, |value| i64::try_from(value.output_tokens).unwrap_or(i64::MAX)),
                    ],
                )?)
            })
            .await?;
        if changed != 1 {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "agent run lease or state changed while recording observation: {run_id}"
                ),
            });
        }
        Ok(())
    }

    /// Add dispatched tool calls and active time while fencing against lease loss.
    pub async fn record_agent_run_progress(
        &self,
        run_id: &str,
        lease_token: &str,
        tool_calls: i64,
        active_milliseconds: i64,
    ) -> Result<(), StoreError> {
        if tool_calls < 0 || active_milliseconds < 0 {
            return Err(StoreError::InvariantViolation {
                message: "agent run progress deltas cannot be negative".to_string(),
            });
        }
        let changed = self
            .with_connection(|conn| {
                Ok(conn.execute(
                    "UPDATE agent_runs SET tool_call_count = tool_call_count + ?3, active_milliseconds = active_milliseconds + ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?2 AND status = 'running' AND cancellation_requested = 0",
                    params![run_id, lease_token, tool_calls, active_milliseconds],
                )?)
            })
            .await?;
        ensure_fenced_write(changed, run_id, "recording progress")
    }

    /// Renew an active run lease and return whether cancellation was requested.
    pub async fn heartbeat_agent_run(
        &self,
        run_id: &str,
        lease_token: &str,
        lease_seconds: i64,
    ) -> Result<AgentRunHeartbeat, StoreError> {
        if lease_seconds < 1 {
            return Err(StoreError::InvariantViolation {
                message: "agent run lease duration must be positive".to_string(),
            });
        }
        let now = now_string();
        let expires_at = (now.parse::<i64>().unwrap_or_default() + lease_seconds).to_string();
        let cancellation_requested = self
            .with_connection(|conn| {
                let changed = conn.execute(
                    "UPDATE agent_runs SET lease_expires_at = ?3, heartbeat_at = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?2 AND status IN ('leased', 'running')",
                    params![run_id, lease_token, expires_at, now],
                )?;
                if changed != 1 {
                    return Err(StoreError::InvariantViolation {
                        message: format!("agent run lease changed while heartbeating: {run_id}"),
                    });
                }
                conn.query_row(
                    "SELECT cancellation_requested FROM agent_runs WHERE run_id = ?1",
                    [run_id],
                    |row| Ok(row.get::<_, i64>(0)? != 0),
                )
                .map_err(StoreError::Sqlite)
            })
            .await?;
        Ok(AgentRunHeartbeat {
            lease_expires_at: expires_at,
            cancellation_requested,
        })
    }

    /// Check the durable cancellation flag for an active run.
    pub async fn agent_run_cancellation_requested(
        &self,
        run_id: &str,
        lease_token: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT cancellation_requested FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2 AND status IN ('leased', 'running')",
                params![run_id, lease_token],
                |row| Ok(row.get::<_, i64>(0)? != 0),
            )
            .optional()
            .map_err(StoreError::Sqlite)?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("agent run lease changed while checking cancellation: {run_id}"),
            })
        })
        .await
    }
}
