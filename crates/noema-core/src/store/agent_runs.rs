//! Durable queue and lease records for task executor/reviewer runs.

#![allow(clippy::missing_errors_doc)]

use noema_providers::{ProviderSelectionSnapshot, ReasoningEffort};
use rusqlite::{OptionalExtension, params};

use crate::{RunKind, RunStatus, TaskExecutionPolicy, TaskStatus};

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, now_string},
};

const RUN_COLUMNS: &str = "run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, resume_message, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, actual_provider_kind, actual_model_profile, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status, priority, queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at, started_at, ended_at, cancellation_requested, retry_count, error_code, error_message, provider_call_count, tool_call_count, input_tokens, cached_input_tokens, output_tokens, active_milliseconds, created_at, updated_at";

/// Input for one queued background run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentRun {
    /// Optional stable run id.
    pub run_id: Option<String>,
    /// Owning task id.
    pub task_id: String,
    /// Executor or reviewer role.
    pub run_kind: RunKind,
    /// Built-in or user-configured agent id.
    pub agent_id: String,
    /// Revision represented by this run.
    pub revision_index: i64,
    /// Continuation or infrastructure-recovery attempt within the revision.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional submission this run reviews.
    pub triggering_submission_id: Option<String>,
    /// Optional review this run follows.
    pub triggering_review_id: Option<String>,
    /// Immutable model request snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Immutable execution-policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
    /// Queue priority; larger values run first.
    pub priority: i64,
}

/// Persisted background run and lease state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunRecord {
    /// Stable run id.
    pub run_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Run role.
    pub run_kind: RunKind,
    /// Acting agent id.
    pub agent_id: String,
    /// Revision index.
    pub revision_index: i64,
    /// Continuation or infrastructure-recovery attempt.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional reviewed submission id.
    pub triggering_submission_id: Option<String>,
    /// Optional preceding review id.
    pub triggering_review_id: Option<String>,
    /// Optional human guidance that resumed this run.
    pub resume_message: Option<String>,
    /// Requested model snapshot.
    pub model: ProviderSelectionSnapshot,
    /// Provider-reported actual model, when available.
    pub actual_provider_kind: Option<String>,
    /// Provider-reported actual profile, when available.
    pub actual_model_profile: Option<String>,
    /// Immutable provider-independent execution-policy snapshot.
    pub execution_policy: TaskExecutionPolicy,
    /// Current queue state.
    pub status: RunStatus,
    /// Queue priority.
    pub priority: i64,
    /// Queue timestamp.
    pub queued_at: String,
    /// Current lease owner.
    pub lease_owner: Option<String>,
    /// Current lease token.
    pub lease_token: Option<String>,
    /// Lease expiry timestamp.
    pub lease_expires_at: Option<String>,
    /// Last heartbeat timestamp.
    pub heartbeat_at: Option<String>,
    /// Start timestamp.
    pub started_at: Option<String>,
    /// End timestamp.
    pub ended_at: Option<String>,
    /// Whether cancellation was requested.
    pub cancellation_requested: bool,
    /// Number of infrastructure retries.
    pub retry_count: i64,
    /// Safe terminal error code.
    pub error_code: Option<String>,
    /// Safe terminal error message.
    pub error_message: Option<String>,
    /// Number of completed provider calls.
    pub provider_call_count: i64,
    /// Number of dispatched tool calls.
    pub tool_call_count: i64,
    /// Cumulative input token count.
    pub input_tokens: i64,
    /// Cumulative cached-input token count.
    pub cached_input_tokens: i64,
    /// Cumulative output token count.
    pub output_tokens: i64,
    /// Cumulative active execution duration.
    pub active_milliseconds: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

impl NoemaStore {
    /// Queue a new background run.
    pub async fn create_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord, StoreError> {
        let model = input.model.normalized_for_persistence().map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        })?;
        let execution_policy =
            input
                .execution_policy
                .validated()
                .map_err(|error| StoreError::InvariantViolation {
                    message: error.to_string(),
                })?;
        if input.task_id.trim().is_empty() || input.agent_id.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "agent run task and agent ids are required".to_string(),
            });
        }
        if input.revision_index < 0 || input.attempt_index < 0 {
            return Err(StoreError::InvariantViolation {
                message: "agent run indexes cannot be negative".to_string(),
            });
        }
        let run_id = input.run_id.unwrap_or_else(|| allocate_id("run"));
        self.with_connection(|conn| {
            conn.execute(
                r#"INSERT INTO agent_runs (
                    run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                    parent_run_id, triggering_submission_id, triggering_review_id,
                    provider_kind, provider_account_id, selection_mode, model_profile,
                    reasoning_effort, selection_source, max_provider_continuations,
                    max_tool_calls, max_active_minutes, progress_audit_interval,
                    status, priority
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'queued', ?20)"#,
                params![
                    run_id,
                    input.task_id,
                    input.run_kind.as_str(),
                    input.agent_id,
                    input.attempt_index,
                    input.revision_index,
                    input.parent_run_id,
                    input.triggering_submission_id,
                    input.triggering_review_id,
                    model.provider_kind,
                    model.provider_account_id,
                    model.selection_mode.as_str(),
                    model.model_profile,
                    model.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    model.selection_source,
                    execution_policy.max_provider_continuations,
                    execution_policy.max_tool_calls,
                    execution_policy.max_active_minutes,
                    execution_policy.progress_audit_interval,
                    input.priority,
                ],
            )?;
            Ok(())
        }).await?;
        self.get_agent_run(&run_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("created run disappeared: {run_id}"),
            })
    }

    /// Return one run by id.
    pub async fn get_agent_run(&self, run_id: &str) -> Result<Option<AgentRunRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                &format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE run_id = ?1"),
                [run_id],
                run_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List runs for one task in creation order.
    pub async fn list_agent_runs_for_task(
        &self,
        task_id: &str,
    ) -> Result<Vec<AgentRunRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(&format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE task_id = ?1 ORDER BY created_at, run_id"))?;
            let rows = statement.query_map([task_id], run_from_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        }).await
    }

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
            let (current, task_id) = conn.query_row("SELECT status, task_id FROM agent_runs WHERE run_id = ?1", [run_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).optional()?.ok_or_else(|| StoreError::InvariantViolation { message: format!("agent run not found: {run_id}") })?;
            let current = current.parse::<RunStatus>().map_err(|error| StoreError::InvalidEnum { kind: "run_status", value: error.to_string() })?;
            if !current.can_transition_to(next) { return Err(StoreError::InvariantViolation { message: format!("invalid run transition {current} -> {next}") }); }
            let changed = if let Some(lease_token) = lease_token {
                conn.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?5 AND (?2 = 'cancelled' OR cancellation_requested = 0)", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str()), lease_token])?
            } else {
                conn.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND (?2 = 'cancelled' OR cancellation_requested = 0)", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str())])?
            };
            if changed != 1 { return Err(StoreError::InvariantViolation { message: format!("agent run lease or state changed while updating: {run_id}") }); }
            append_run_event(
                conn,
                run_id,
                &format!("run.{}", next.as_str()),
                serde_json::json!({
                    "from": current.as_str(),
                    "to": next.as_str(),
                    "error_code": error.as_ref().map(|value| value.0.as_str()),
                }),
            )?;
            append_task_event(
                conn,
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
                let task_changed = conn.execute(
                    "UPDATE tasks SET status = 'failed', terminal_reason = ?3, error_code = ?2, error_message = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = (SELECT task_id FROM agent_runs WHERE run_id = ?1) AND latest_run_id = ?1 AND status NOT IN ('completed', 'cancelled')",
                    params![run_id, error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str())],
                )?;
                if task_changed == 1 {
                    append_task_event(
                        conn,
                        &task_id,
                        "task.failed",
                        serde_json::json!({
                            "run_id": run_id,
                            "error_code": error.as_ref().map(|value| value.0.as_str()),
                        }),
                    )?;
                }
            }
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

/// Result of one lease renewal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunHeartbeat {
    /// New lease expiration as Unix seconds.
    pub lease_expires_at: String,
    /// Whether the owner requested cancellation.
    pub cancellation_requested: bool,
}

fn ensure_fenced_write(changed: usize, run_id: &str, operation: &str) -> Result<(), StoreError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: format!("agent run lease or state changed while {operation}: {run_id}"),
        })
    }
}

fn recover_expired_runs(conn: &rusqlite::Connection, now: &str) -> Result<(), StoreError> {
    let expired_ids = {
        let mut statement = conn.prepare(
            "SELECT run_id FROM agent_runs WHERE status IN ('leased', 'running') AND lease_expires_at IS NOT NULL AND CAST(lease_expires_at AS INTEGER) <= CAST(?1 AS INTEGER) ORDER BY lease_expires_at, run_id",
        )?;
        statement
            .query_map([now], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for run_id in expired_ids {
        let run = conn.query_row(
            &format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE run_id = ?1"),
            [&run_id],
            run_from_row,
        )?;
        let task_state = conn
            .query_row(
                "SELECT status, latest_run_id FROM tasks WHERE task_id = ?1",
                [&run.task_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let is_latest = task_state
            .as_ref()
            .is_some_and(|(_, latest)| latest.as_deref() == Some(run_id.as_str()));
        conn.execute(
            "UPDATE agent_runs SET status = 'interrupted', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), error_code = 'lease_expired', error_message = 'worker lease expired before run completion', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status IN ('leased', 'running')",
            [&run_id],
        )?;
        append_run_event(
            conn,
            &run_id,
            "run.interrupted",
            serde_json::json!({"reason": "lease_expired"}),
        )?;
        if !is_latest || run.cancellation_requested {
            continue;
        }
        if run.retry_count >= 3 {
            conn.execute(
                "UPDATE tasks SET status = 'failed', terminal_reason = 'automatic infrastructure resume limit reached', error_code = 'automatic_resume_exhausted', error_message = 'run lease expired after three automatic resumptions', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND status NOT IN ('completed', 'failed', 'cancelled')",
                [&run.task_id],
            )?;
            append_task_event(
                conn,
                &run.task_id,
                "task.failed",
                serde_json::json!({
                    "run_id": run_id,
                    "error_code": "automatic_resume_exhausted",
                }),
            )?;
            continue;
        }
        let child_run_id = allocate_id("run");
        let child_retry_count = run.retry_count + 1;
        conn.execute(
            r#"INSERT INTO agent_runs (
                run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                parent_run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                status, priority, retry_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'queued', ?20, ?21)"#,
            params![
                child_run_id,
                run.task_id,
                run.run_kind.as_str(),
                run.agent_id,
                run.attempt_index + 1,
                run.revision_index,
                run.run_id,
                run.triggering_submission_id,
                run.triggering_review_id,
                run.model.provider_kind,
                run.model.provider_account_id,
                run.model.selection_mode.as_str(),
                run.model.model_profile,
                run.model
                    .reasoning_effort
                    .map(ReasoningEffort::as_persistence_str),
                run.model.selection_source,
                run.execution_policy.max_provider_continuations,
                run.execution_policy.max_tool_calls,
                run.execution_policy.max_active_minutes,
                run.execution_policy.progress_audit_interval,
                run.priority,
                child_retry_count,
            ],
        )?;
        append_run_event(
            conn,
            &child_run_id,
            "run.queued",
            serde_json::json!({
                "automatic_resume_of_run_id": run_id,
                "retry_count": child_retry_count,
            }),
        )?;
        let next_task_status = match run.run_kind {
            RunKind::Executor => Some(TaskStatus::Queued),
            RunKind::Reviewer => Some(TaskStatus::Reviewing),
        };
        if let Some(next_task_status) = next_task_status {
            conn.execute(
                "UPDATE tasks SET status = ?2, latest_run_id = ?3, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND status NOT IN ('completed', 'failed', 'cancelled')",
                params![run.task_id, next_task_status.as_str(), child_run_id],
            )?;
        }
        append_task_event(
            conn,
            &run.task_id,
            "task.automatically_resumed",
            serde_json::json!({
                "interrupted_run_id": run_id,
                "new_run_id": child_run_id,
                "retry_count": child_retry_count,
            }),
        )?;
    }
    Ok(())
}

fn recover_interrupted_runs(conn: &rusqlite::Connection) -> Result<(), StoreError> {
    let interrupted = {
        let mut statement = conn.prepare(
            "SELECT r.run_id, r.task_id, r.run_kind, r.retry_count FROM agent_runs r JOIN tasks t ON t.task_id = r.task_id AND t.latest_run_id = r.run_id WHERE r.status = 'interrupted' AND r.cancellation_requested = 0 AND t.status NOT IN ('completed', 'failed', 'cancelled') ORDER BY r.updated_at, r.run_id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (run_id, task_id, run_kind, retry_count) in interrupted {
        if retry_count >= 3 {
            let changed = conn.execute(
                "UPDATE tasks SET status = 'failed', terminal_reason = 'automatic infrastructure resume limit reached', error_code = 'automatic_resume_exhausted', error_message = 'run was interrupted after three automatic resumptions', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?2 AND status NOT IN ('completed', 'failed', 'cancelled')",
                params![task_id, run_id],
            )?;
            if changed == 1 {
                append_task_event(
                    conn,
                    &task_id,
                    "task.failed",
                    serde_json::json!({
                        "run_id": run_id,
                        "error_code": "automatic_resume_exhausted",
                    }),
                )?;
            }
            continue;
        }
        let child_run_id = allocate_id("run");
        let inserted = conn.execute(
            r#"INSERT INTO agent_runs (
                run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                parent_run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                status, priority, retry_count
            ) SELECT ?2, task_id, run_kind, agent_id, attempt_index + 1, revision_index,
                run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                'queued', priority, retry_count + 1
              FROM agent_runs WHERE run_id = ?1 AND status = 'interrupted'
                AND cancellation_requested = 0"#,
            params![run_id, child_run_id],
        )?;
        if inserted != 1 {
            continue;
        }
        append_run_event(
            conn,
            &child_run_id,
            "run.queued",
            serde_json::json!({
                "automatic_resume_of_run_id": run_id,
                "retry_count": retry_count + 1,
            }),
        )?;
        let next_task_status = if run_kind == RunKind::Reviewer.as_str() {
            TaskStatus::Reviewing
        } else {
            TaskStatus::Queued
        };
        let changed = conn.execute(
            "UPDATE tasks SET status = ?3, latest_run_id = ?2, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?4 AND status NOT IN ('completed', 'failed', 'cancelled')",
            params![task_id, child_run_id, next_task_status.as_str(), run_id],
        )?;
        if changed != 1 {
            return Err(StoreError::InvariantViolation {
                message: format!("task changed while recovering interrupted run: {task_id}"),
            });
        }
        append_task_event(
            conn,
            &task_id,
            "task.automatically_resumed",
            serde_json::json!({
                "interrupted_run_id": run_id,
                "new_run_id": child_run_id,
                "retry_count": retry_count + 1,
            }),
        )?;
    }
    Ok(())
}

fn append_task_event(
    conn: &rusqlite::Connection,
    task_id: &str,
    event_kind: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, 'system:task-runtime', ?5)",
        params![allocate_id("event"), task_id, sequence, event_kind, payload.to_string()],
    )?;
    Ok(())
}

fn append_run_event(
    conn: &rusqlite::Connection,
    run_id: &str,
    event_kind: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM run_events WHERE run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, 'system:task-runtime', ?5)",
        params![
            super::ids::allocate_id("event"),
            run_id,
            sequence,
            event_kind,
            payload.to_string(),
        ],
    )?;
    Ok(())
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRunRecord> {
    let run_kind = row
        .get::<_, String>(2)?
        .parse::<RunKind>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let selection_mode = row
        .get::<_, String>(12)?
        .parse::<noema_providers::ProviderSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort = row
        .get::<_, Option<String>>(14)?
        .as_deref()
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid reasoning effort",
                    )),
                )
            })
        })
        .transpose()?;
    let status = row
        .get::<_, String>(22)?
        .parse::<RunStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                22,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(AgentRunRecord {
        run_id: row.get(0)?,
        task_id: row.get(1)?,
        run_kind,
        agent_id: row.get(3)?,
        attempt_index: row.get(4)?,
        revision_index: row.get(5)?,
        parent_run_id: row.get(6)?,
        triggering_submission_id: row.get(7)?,
        triggering_review_id: row.get(8)?,
        resume_message: row.get(9)?,
        model: ProviderSelectionSnapshot {
            provider_instance_key: None,
            provider_kind: row.get(10)?,
            provider_account_id: row.get(11)?,
            selection_mode,
            model_profile: row.get(13)?,
            reasoning_effort,
            selection_source: row.get(15)?,
        },
        actual_provider_kind: row.get(16)?,
        actual_model_profile: row.get(17)?,
        execution_policy: TaskExecutionPolicy {
            max_provider_continuations: row.get(18)?,
            max_tool_calls: row.get(19)?,
            max_active_minutes: row.get(20)?,
            progress_audit_interval: row.get(21)?,
        },
        status,
        priority: row.get(23)?,
        queued_at: row.get(24)?,
        lease_owner: row.get(25)?,
        lease_token: row.get(26)?,
        lease_expires_at: row.get(27)?,
        heartbeat_at: row.get(28)?,
        started_at: row.get(29)?,
        ended_at: row.get(30)?,
        cancellation_requested: row.get::<_, i64>(31)? != 0,
        retry_count: row.get(32)?,
        error_code: row.get(33)?,
        error_message: row.get(34)?,
        provider_call_count: row.get(35)?,
        tool_call_count: row.get(36)?,
        input_tokens: row.get(37)?,
        cached_input_tokens: row.get(38)?,
        output_tokens: row.get(39)?,
        active_milliseconds: row.get(40)?,
        created_at: row.get(41)?,
        updated_at: row.get(42)?,
    })
}
