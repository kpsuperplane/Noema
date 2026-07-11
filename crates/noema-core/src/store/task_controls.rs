//! Owner-authorized task continuation, blocking, and cancellation operations.

#![allow(clippy::missing_errors_doc)]

use rusqlite::{OptionalExtension, params};

use crate::{RunKind, RunStatus, TASK_REVIEWER_AGENT_ID, TaskStatus, provider::ReasoningEffort};

use super::{AgentRunRecord, NoemaStore, StoreError, TaskRecord, ids::allocate_id};

impl NoemaStore {
    /// Continue a failed or human-blocked task with its existing lineage and evidence.
    pub async fn resume_task(
        &self,
        task_id: &str,
        owner_human_id: &str,
        actor_id: &str,
        message: Option<&str>,
    ) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
        let task_id = required(task_id, "task id")?;
        let owner_human_id = required(owner_human_id, "owner id")?;
        let actor_id = required(actor_id, "actor id")?;
        let task = self.get_task(task_id).await?.ok_or_else(task_unavailable)?;
        if task.owner_human_id != owner_human_id {
            return Err(task_unavailable());
        }
        if !matches!(
            task.status,
            TaskStatus::Failed | TaskStatus::WaitingForHuman
        ) {
            return Err(StoreError::InvariantViolation {
                message: "only failed or human-blocked tasks can be continued".to_string(),
            });
        }
        let message = message
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if task.status == TaskStatus::WaitingForHuman && message.is_none() {
            return Err(StoreError::InvariantViolation {
                message: "human-blocked task continuation requires a message".to_string(),
            });
        }
        let parent_run_id =
            task.latest_run_id
                .as_deref()
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: "task has no run to continue".to_string(),
                })?;
        let parent = self.get_agent_run(parent_run_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: "task continuation parent run is unavailable".to_string(),
            }
        })?;
        if !matches!(
            parent.status,
            RunStatus::Failed
                | RunStatus::WaitingForApproval
                | RunStatus::Interrupted
                | RunStatus::Completed
        ) {
            return Err(StoreError::InvariantViolation {
                message: "latest task run cannot be continued".to_string(),
            });
        }
        let model = if parent.run_kind == RunKind::Executor {
            self.select_task_model_pool_entry(task.complexity, &task.pool_entry_id)
                .await?
                .model
        } else if let Some(preference) = self
            .get_agent_runtime_preference(TASK_REVIEWER_AGENT_ID)
            .await?
        {
            crate::ModelConfigSnapshot::explicit(
                preference.provider_kind,
                preference.provider_account_id,
                preference.model_profile,
                preference.reasoning_effort,
                Some("agent:task-reviewer".to_string()),
            )
        } else {
            task.reviewer_model.clone()
        }
        .normalized()
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
        let execution_policy = self.get_task_execution_policy().await?;
        let new_run_id = allocate_id("run");
        let next_attempt_index =
            parent
                .attempt_index
                .checked_add(1)
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: "task continuation attempt index is exhausted".to_string(),
                })?;
        let next_status = if parent.run_kind == RunKind::Reviewer {
            TaskStatus::Reviewing
        } else {
            TaskStatus::Queued
        };
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let current = tx
                .query_row(
                    "SELECT status, owner_human_id, latest_run_id FROM tasks WHERE task_id = ?1",
                    [task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(task_unavailable)?;
            if current.1 != owner_human_id
                || !matches!(current.0.as_str(), "failed" | "waiting_for_human")
                || current.2.as_deref() != Some(parent.run_id.as_str())
            {
                return Err(StoreError::InvariantViolation {
                    message: "task changed while continuing".to_string(),
                });
            }
            tx.execute(
                r#"INSERT INTO agent_runs (
                    run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                    parent_run_id, triggering_submission_id, triggering_review_id,
                    resume_message, provider_kind, provider_account_id, selection_mode,
                    model_profile, reasoning_effort, selection_source,
                    max_provider_continuations, max_tool_calls, max_active_minutes,
                    progress_audit_interval, status, priority
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 'queued', ?21)"#,
                params![
                    new_run_id,
                    task_id,
                    parent.run_kind.as_str(),
                    parent.agent_id,
                    next_attempt_index,
                    parent.revision_index,
                    parent.run_id,
                    parent.triggering_submission_id,
                    parent.triggering_review_id,
                    message,
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
                    parent.priority,
                ],
            )?;
            append_run_event_tx(
                &tx,
                &new_run_id,
                "run.queued",
                actor_id,
                serde_json::json!({
                    "continued_from_run_id": parent.run_id,
                    "attempt_index": next_attempt_index,
                    "has_message": message.is_some(),
                }),
            )?;
            let changed = tx.execute(
                "UPDATE tasks SET status = ?2, latest_run_id = ?3, blocked_question = NULL, blocked_context = NULL, terminal_reason = NULL, error_code = NULL, error_message = NULL, completed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?4",
                params![task_id, next_status.as_str(), new_run_id, parent.run_id],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "task changed while continuing".to_string(),
                });
            }
            append_task_event_tx(
                &tx,
                task_id,
                "task.resumed",
                actor_id,
                serde_json::json!({
                    "parent_run_id": parent.run_id,
                    "new_run_id": new_run_id,
                    "status": next_status.as_str(),
                    "has_message": message.is_some(),
                }),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        let task = self.get_task(task_id).await?.ok_or_else(task_unavailable)?;
        let run = self.get_agent_run(&new_run_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: "continued run disappeared".to_string(),
            }
        })?;
        Ok((task, run))
    }

    /// Persist a blocking report and release the active run lease atomically.
    pub async fn report_task_blocked(
        &self,
        task_id: &str,
        run_id: &str,
        lease_token: &str,
        question: &str,
        context: &str,
    ) -> Result<TaskRecord, StoreError> {
        let question = required(question, "blocking question")?;
        let context = required(context, "blocking context")?;
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let changed = tx.execute(
                "UPDATE agent_runs SET status = 'waiting_for_approval', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND task_id = ?2 AND lease_token = ?3 AND status = 'running' AND cancellation_requested = 0",
                params![run_id, task_id, lease_token],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task run lease changed while reporting blocked: {run_id}"),
                });
            }
            let task_changed = tx.execute(
                "UPDATE tasks SET status = 'waiting_for_human', blocked_question = ?3, blocked_context = ?4, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?2 AND status IN ('executing', 'reviewing', 'revision_requested')",
                params![task_id, run_id, question, context],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while reporting blocked: {task_id}"),
                });
            }
            append_run_event_tx(
                &tx,
                run_id,
                "run.blocked",
                "system:task-runtime",
                serde_json::json!({"question": question}),
            )?;
            append_task_event_tx(
                &tx,
                task_id,
                "task.waiting_for_human",
                "system:task-runtime",
                serde_json::json!({"run_id": run_id, "question": question}),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        self.get_task(task_id).await?.ok_or_else(task_unavailable)
    }

    /// Request cancellation for all unfinished work owned by one task.
    pub async fn cancel_task(
        &self,
        task_id: &str,
        owner_human_id: &str,
        actor_id: &str,
    ) -> Result<TaskRecord, StoreError> {
        let task_id = required(task_id, "task id")?;
        let owner_human_id = required(owner_human_id, "owner id")?;
        let actor_id = required(actor_id, "actor id")?;
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let state = tx
                .query_row(
                    "SELECT status, owner_human_id FROM tasks WHERE task_id = ?1",
                    [task_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(task_unavailable)?;
            if state.1 != owner_human_id {
                return Err(task_unavailable());
            }
            if state.0 == TaskStatus::Cancelled.as_str() {
                tx.commit()?;
                return Ok(());
            }
            if state.0 == TaskStatus::Completed.as_str() {
                return Err(StoreError::InvariantViolation {
                    message: "completed task cannot be cancelled".to_string(),
                });
            }
            let active_run_ids = {
                let mut statement = tx.prepare(
                    "SELECT run_id FROM agent_runs WHERE task_id = ?1 AND status IN ('queued', 'leased', 'running', 'waiting_for_approval', 'interrupted')",
                )?;
                statement
                    .query_map([task_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            tx.execute(
                "UPDATE agent_runs SET cancellation_requested = 1, status = CASE WHEN status IN ('queued', 'waiting_for_approval', 'interrupted') THEN 'cancelled' ELSE status END, ended_at = CASE WHEN status IN ('queued', 'waiting_for_approval', 'interrupted') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE ended_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND status IN ('queued', 'leased', 'running', 'waiting_for_approval', 'interrupted')",
                [task_id],
            )?;
            for run_id in active_run_ids {
                append_run_event_tx(
                    &tx,
                    &run_id,
                    "run.cancellation_requested",
                    actor_id,
                    serde_json::json!({"task_id": task_id}),
                )?;
            }
            tx.execute(
                "UPDATE tasks SET status = 'cancelled', terminal_reason = 'cancelled by owner', blocked_question = NULL, blocked_context = NULL, error_code = NULL, error_message = NULL, completed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1",
                [task_id],
            )?;
            append_task_event_tx(
                &tx,
                task_id,
                "task.cancelled",
                actor_id,
                serde_json::json!({"previous_status": state.0}),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        self.get_task(task_id).await?.ok_or_else(task_unavailable)
    }

    /// Compatibility spelling used by the runtime supervisor.
    pub async fn request_task_cancellation(
        &self,
        task_id: &str,
        owner_human_id: &str,
        actor_id: &str,
    ) -> Result<TaskRecord, StoreError> {
        self.cancel_task(task_id, owner_human_id, actor_id).await
    }
}

fn required<'a>(value: &'a str, label: &str) -> Result<&'a str, StoreError> {
    let value = value.trim();
    if value.is_empty() {
        Err(StoreError::InvariantViolation {
            message: format!("task control requires {label}"),
        })
    } else {
        Ok(value)
    }
}

fn task_unavailable() -> StoreError {
    StoreError::InvariantViolation {
        message: "task is unavailable".to_string(),
    }
}

fn append_run_event_tx(
    conn: &rusqlite::Connection,
    run_id: &str,
    event_kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM run_events WHERE run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![allocate_id("event"), run_id, sequence, event_kind, actor_id, payload.to_string()],
    )?;
    Ok(())
}

fn append_task_event_tx(
    conn: &rusqlite::Connection,
    task_id: &str,
    event_kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![allocate_id("event"), task_id, sequence, event_kind, actor_id, payload.to_string()],
    )?;
    Ok(())
}
