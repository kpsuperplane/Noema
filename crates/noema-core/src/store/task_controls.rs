//! Owner-authorized task control operations.

#![allow(clippy::missing_errors_doc)]

use rusqlite::{OptionalExtension, params};

use crate::{RunKind, TaskStatus};

use super::{AgentRunRecord, NoemaStore, StoreError, TaskRecord, ids::allocate_id};

impl NoemaStore {
    /// Atomically queue a fresh attempt for the run that failed a task.
    ///
    /// The failed run remains immutable audit history. The replacement copies
    /// its role, model snapshot, revision, and trigger provenance, and links
    /// back through `parent_run_id`.
    pub async fn retry_failed_task(
        &self,
        task_id: &str,
        owner_human_id: &str,
        actor_id: &str,
    ) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
        let task_id = task_id.trim();
        let owner_human_id = owner_human_id.trim();
        let actor_id = actor_id.trim();
        if task_id.is_empty() || owner_human_id.is_empty() || actor_id.is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "task retry requires task, owner, and actor ids".to_string(),
            });
        }
        let new_run_id = allocate_id("run");
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let task = tx
                .query_row(
                    "SELECT status, owner_human_id, latest_run_id FROM tasks WHERE task_id = ?1 LIMIT 1",
                    [task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((status, owner, latest_run_id)) = task else {
                return Err(StoreError::InvariantViolation {
                    message: "task is unavailable".to_string(),
                });
            };
            if owner != owner_human_id {
                return Err(StoreError::InvariantViolation {
                    message: "task is unavailable".to_string(),
                });
            }
            if status != TaskStatus::Failed.as_str() {
                return Err(StoreError::InvariantViolation {
                    message: "only failed tasks can be retried".to_string(),
                });
            }
            let failed_run_id = latest_run_id.ok_or_else(|| StoreError::InvariantViolation {
                message: "failed task has no run to retry".to_string(),
            })?;
            let run = tx
                .query_row(
                    "SELECT run_kind, agent_id, attempt_index, revision_index, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, priority, status FROM agent_runs WHERE run_id = ?1 AND task_id = ?2 LIMIT 1",
                    params![failed_run_id, task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, Option<String>>(10)?,
                            row.get::<_, Option<String>>(11)?,
                            row.get::<_, i64>(12)?,
                            row.get::<_, String>(13)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: "failed task run is unavailable".to_string(),
                })?;
            let (
                run_kind,
                agent_id,
                attempt_index,
                revision_index,
                triggering_submission_id,
                triggering_review_id,
                provider_kind,
                provider_account_id,
                selection_mode,
                model_profile,
                reasoning_effort,
                selection_source,
                priority,
                run_status,
            ) = run;
            if run_status != "failed" {
                return Err(StoreError::InvariantViolation {
                    message: "latest task run is not failed".to_string(),
                });
            }
            if run_kind == RunKind::CompletionDelivery.as_str() {
                return Err(StoreError::InvariantViolation {
                    message: "completion delivery retries are not task retries".to_string(),
                });
            }
            let next_status = if run_kind == RunKind::Reviewer.as_str() {
                TaskStatus::Reviewing
            } else {
                TaskStatus::Queued
            };
            let next_attempt_index = attempt_index.checked_add(1).ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "task retry attempt index is exhausted".to_string(),
                }
            })?;
            tx.execute(
                "INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, status, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'queued', ?16)",
                params![new_run_id, task_id, run_kind, agent_id, next_attempt_index, revision_index, failed_run_id, triggering_submission_id, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, priority],
            )?;
            tx.execute(
                "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                params![allocate_id("event"), new_run_id, actor_id, serde_json::json!({"retry_of_run_id": failed_run_id, "attempt_index": next_attempt_index}).to_string()],
            )?;
            let changed = tx.execute(
                "UPDATE tasks SET status = ?2, latest_run_id = ?3, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND status = 'failed'",
                params![task_id, next_status.as_str(), new_run_id],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "task changed while retrying".to_string(),
                });
            }
            let sequence: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
                [task_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, 'task.retried', ?4, ?5)",
                params![allocate_id("event"), task_id, sequence, actor_id, serde_json::json!({"failed_run_id": failed_run_id, "new_run_id": new_run_id, "status": next_status.as_str()}).to_string()],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        let task = self
            .get_task(task_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "retried task disappeared".to_string(),
            })?;
        let run = self.get_agent_run(&new_run_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: "retried run disappeared".to_string(),
            }
        })?;
        Ok((task, run))
    }
}
