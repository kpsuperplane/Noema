//! Durable queue and lease records for task executor/reviewer runs.

use rusqlite::{OptionalExtension, params};

use crate::{ModelConfigSnapshot, RunKind, RunStatus, provider::ReasoningEffort};

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, now_string},
};

/// Input for one queued background run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentRun {
    /// Optional stable run id.
    pub run_id: Option<String>,
    /// Owning task id.
    pub task_id: String,
    /// Executor, reviewer, or completion-delivery role.
    pub run_kind: RunKind,
    /// Built-in or user-configured agent id.
    pub agent_id: String,
    /// Revision represented by this run.
    pub revision_index: i64,
    /// Retry attempt within the revision.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional submission this run reviews.
    pub triggering_submission_id: Option<String>,
    /// Optional review this run follows.
    pub triggering_review_id: Option<String>,
    /// Immutable model request snapshot.
    pub model: ModelConfigSnapshot,
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
    /// Retry attempt.
    pub attempt_index: i64,
    /// Optional parent run.
    pub parent_run_id: Option<String>,
    /// Optional reviewed submission id.
    pub triggering_submission_id: Option<String>,
    /// Optional preceding review id.
    pub triggering_review_id: Option<String>,
    /// Requested model snapshot.
    pub model: ModelConfigSnapshot,
    /// Provider-reported actual model, when available.
    pub actual_provider_kind: Option<String>,
    /// Provider-reported actual profile, when available.
    pub actual_model_profile: Option<String>,
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
    /// Input token count, when reported.
    pub input_tokens: Option<i64>,
    /// Output token count, when reported.
    pub output_tokens: Option<i64>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

impl NoemaStore {
    /// Queue a new background run.
    pub async fn create_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord, StoreError> {
        let model = input
            .model
            .normalized()
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
                    reasoning_effort, selection_source, status, priority
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'queued', ?16)"#,
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
            conn.query_row("SELECT run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, actual_provider_kind, actual_model_profile, status, priority, queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at, started_at, ended_at, cancellation_requested, retry_count, error_code, error_message, input_tokens, output_tokens, created_at, updated_at FROM agent_runs WHERE run_id = ?1", [run_id], run_from_row).optional().map_err(StoreError::Sqlite)
        }).await
    }

    /// List runs for one task in creation order.
    pub async fn list_agent_runs_for_task(
        &self,
        task_id: &str,
    ) -> Result<Vec<AgentRunRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare("SELECT run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, actual_provider_kind, actual_model_profile, status, priority, queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at, started_at, ended_at, cancellation_requested, retry_count, error_code, error_message, input_tokens, output_tokens, created_at, updated_at FROM agent_runs WHERE task_id = ?1 ORDER BY created_at, run_id")?;
            let rows = statement.query_map([task_id], run_from_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        }).await
    }

    /// Atomically claim the oldest queued/interruptible run.
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
            let now = now_string();
            let lease_expires_at = (now.parse::<i64>().unwrap_or_default() + lease_seconds).to_string();
            conn.execute(
                "UPDATE agent_runs SET status = 'interrupted', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, retry_count = retry_count + 1, error_code = 'lease_expired', error_message = 'worker lease expired before run completion', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE status IN ('leased', 'running') AND lease_expires_at IS NOT NULL AND CAST(lease_expires_at AS INTEGER) <= CAST(?1 AS INTEGER)",
                [now.as_str()],
            )?;
            let changed = conn.execute(
                "UPDATE agent_runs SET status = 'leased', lease_owner = ?1, lease_token = ?2, lease_expires_at = ?3, heartbeat_at = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = (SELECT run_id FROM agent_runs WHERE status IN ('queued', 'interrupted') ORDER BY priority DESC, queued_at, run_id LIMIT 1) AND status IN ('queued', 'interrupted')",
                params![worker_id, lease_token, lease_expires_at, now],
            )?;
            if changed == 0 { return Ok(None); }
            conn.query_row("SELECT run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, actual_provider_kind, actual_model_profile, status, priority, queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at, started_at, ended_at, cancellation_requested, retry_count, error_code, error_message, input_tokens, output_tokens, created_at, updated_at FROM agent_runs WHERE lease_owner = ?1 AND lease_token = ?2 AND status = 'leased' ORDER BY updated_at DESC LIMIT 1", params![worker_id, lease_token], run_from_row).optional().map_err(StoreError::Sqlite)
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
            let current = conn.query_row("SELECT status FROM agent_runs WHERE run_id = ?1", [run_id], |row| row.get::<_, String>(0)).optional()?.ok_or_else(|| StoreError::InvariantViolation { message: format!("agent run not found: {run_id}") })?;
            let current = current.parse::<RunStatus>().map_err(|error| StoreError::InvalidEnum { kind: "run_status", value: error.to_string() })?;
            if !current.can_transition_to(next) { return Err(StoreError::InvariantViolation { message: format!("invalid run transition {current} -> {next}") }); }
            let changed = if let Some(lease_token) = lease_token {
                conn.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?5", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str()), lease_token])?
            } else {
                conn.execute("UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, started_at = CASE WHEN ?2 = 'running' THEN COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE started_at END, ended_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, lease_owner = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_owner END, lease_token = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_token END, lease_expires_at = CASE WHEN ?2 IN ('completed', 'failed', 'cancelled', 'interrupted') THEN NULL ELSE lease_expires_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1", params![run_id, next.as_str(), error.as_ref().map(|value| value.0.as_str()), error.as_ref().map(|value| value.1.as_str())])?
            };
            if changed != 1 { return Err(StoreError::InvariantViolation { message: format!("agent run lease or state changed while updating: {run_id}") }); }
            Ok(())
        }).await?;
        self.get_agent_run(run_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("agent run disappeared: {run_id}"),
            })
    }

    /// Record provider identity and usage for a leased run.
    pub async fn record_agent_run_observation(
        &self,
        run_id: &str,
        lease_token: &str,
        actual_provider_kind: &str,
        actual_model_profile: &str,
        usage: Option<&crate::TokenUsage>,
    ) -> Result<(), StoreError> {
        let changed = self
            .with_connection(|conn| {
                Ok(conn.execute(
                    "UPDATE agent_runs SET actual_provider_kind = ?3, actual_model_profile = ?4, input_tokens = ?5, output_tokens = ?6, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?2 AND status = 'running'",
                    params![
                        run_id,
                        lease_token,
                        actual_provider_kind,
                        actual_model_profile,
                        usage.map(|value| i64::try_from(value.input_tokens).unwrap_or(i64::MAX)),
                        usage.map(|value| i64::try_from(value.output_tokens).unwrap_or(i64::MAX)),
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
        .get::<_, String>(11)?
        .parse::<crate::ModelSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort = row
        .get::<_, Option<String>>(13)?
        .as_deref()
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    13,
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
        .get::<_, String>(17)?
        .parse::<RunStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                17,
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
        model: ModelConfigSnapshot {
            provider_kind: row.get(9)?,
            provider_account_id: row.get(10)?,
            selection_mode,
            model_profile: row.get(12)?,
            reasoning_effort,
            selection_source: row.get(14)?,
        },
        actual_provider_kind: row.get(15)?,
        actual_model_profile: row.get(16)?,
        status,
        priority: row.get(18)?,
        queued_at: row.get(19)?,
        lease_owner: row.get(20)?,
        lease_token: row.get(21)?,
        lease_expires_at: row.get(22)?,
        heartbeat_at: row.get(23)?,
        started_at: row.get(24)?,
        ended_at: row.get(25)?,
        cancellation_requested: row.get::<_, i64>(26)? != 0,
        retry_count: row.get(27)?,
        error_code: row.get(28)?,
        error_message: row.get(29)?,
        input_tokens: row.get(30)?,
        output_tokens: row.get(31)?,
        created_at: row.get(32)?,
        updated_at: row.get(33)?,
    })
}
