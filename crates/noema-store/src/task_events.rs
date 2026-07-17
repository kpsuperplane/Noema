//! Append-only task and run event persistence.

#![allow(clippy::missing_errors_doc, clippy::too_many_arguments)]

use noema_tasks::{NewRunEvent, NewTaskEvent, TaskEventKind, TaskEventRecord};
use rusqlite::OptionalExtension;
use serde_json::Value;

use super::{NoemaStore, StoreError, ids::allocate_id};

impl NoemaStore {
    /// Return the blocking question recorded for one task run, when present.
    pub async fn task_blocking_question_for_run(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> Result<Option<String>, StoreError> {
        let payload = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT payload_json FROM task_events WHERE task_id = ?1 AND event_kind = 'task.waiting_for_human' AND json_extract(payload_json, '$.run_id') = ?2 ORDER BY sequence_number DESC LIMIT 1",
                    rusqlite::params![task_id, run_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        payload
            .map(|payload| {
                serde_json::from_str::<Value>(&payload)
                    .map_err(|error| StoreError::InvariantViolation {
                        message: format!("task blocking event payload is invalid: {error}"),
                    })
                    .map(|payload| {
                        payload
                            .get("question")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|question| !question.is_empty())
                            .map(ToOwned::to_owned)
                    })
            })
            .transpose()
            .map(Option::flatten)
    }

    /// Append a task event with the next monotonic sequence number.
    pub async fn append_task_event(&self, event: NewTaskEvent) -> Result<String, StoreError> {
        append_event(
            self,
            "task_events",
            "task_id",
            &event.task_id,
            event.event_id,
            event.event_kind,
            event.actor_id,
            event.causation_id,
            event.correlation_id,
            event.payload,
        )
        .await
    }

    /// Append a run event with the next monotonic sequence number.
    pub async fn append_run_event(&self, event: NewRunEvent) -> Result<String, StoreError> {
        append_event(
            self,
            "run_events",
            "run_id",
            &event.run_id,
            event.event_id,
            event.event_kind,
            event.actor_id,
            event.causation_id,
            event.correlation_id,
            event.payload,
        )
        .await
    }

    /// Return task events after an exclusive durable sequence cursor.
    pub async fn list_task_events_after(
        &self,
        task_id: &str,
        after_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<TaskEventRecord>, StoreError> {
        if limit < 1 {
            return Err(StoreError::InvariantViolation {
                message: "task event limit must be positive".to_string(),
            });
        }
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT event_id, task_id, sequence_number, event_kind, actor_id, causation_id, correlation_id, payload_json, created_at FROM task_events WHERE task_id = ?1 AND sequence_number > ?2 ORDER BY sequence_number, event_id LIMIT ?3",
            )?;
            let rows = statement.query_map(
                rusqlite::params![task_id, after_sequence.unwrap_or(0).max(0), limit],
                |row| {
                    let payload = serde_json::from_str::<Value>(&row.get::<_, String>(7)?)
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(TaskEventRecord {
                        event_id: row.get(0)?,
                        task_id: row.get(1)?,
                        sequence_number: row.get(2)?,
                        event_kind: row
                            .get::<_, String>(3)?
                            .parse::<TaskEventKind>()
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    3,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        actor_id: row.get(4)?,
                        causation_id: row.get(5)?,
                        correlation_id: row.get(6)?,
                        payload,
                        created_at: row.get(8)?,
                    })
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return the latest durable task event sequence, or zero for no events.
    pub async fn latest_task_event_sequence(&self, task_id: &str) -> Result<i64, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) FROM task_events WHERE task_id = ?1",
                [task_id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return actionable/terminal task ids whose durable status event has not
    /// yet been materialized into the source conversation. Task events form
    /// the outbox; deterministic conversation item ids make draining safe to
    /// repeat after a crash.
    pub async fn list_pending_task_status_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<String>, StoreError> {
        if limit < 1 {
            return Err(StoreError::InvariantViolation {
                message: "task status delivery limit must be positive".to_string(),
            });
        }
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"SELECT t.task_id
                   FROM tasks t
                   JOIN task_events e ON e.event_id = (
                     SELECT candidate.event_id
                     FROM task_events candidate
                     WHERE candidate.task_id = t.task_id
                       AND candidate.event_kind = 'task.' || t.status
                     ORDER BY candidate.sequence_number DESC
                     LIMIT 1
                   )
                   WHERE t.source_conversation_id IS NOT NULL
                     AND t.status IN ('waiting_for_human', 'completed', 'failed', 'cancelled')
                     AND NOT EXISTS (
                       SELECT 1 FROM conversation_items item
                       WHERE item.item_id = 'item:task_status:' || e.event_id
                     )
                   ORDER BY e.created_at, t.task_id
                   LIMIT ?1"#,
            )?;
            let rows = statement.query_map([limit], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return terminal task/event pairs whose primary-agent completion report
    /// has not yet been materialized into the source conversation.
    ///
    /// The event id is part of the projected item id so a later terminal state
    /// (for example, a successful retry after a failure) gets its own report.
    pub async fn list_pending_task_completion_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String)>, StoreError> {
        if limit < 1 {
            return Err(StoreError::InvariantViolation {
                message: "task completion delivery limit must be positive".to_string(),
            });
        }
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"SELECT t.task_id, e.event_id
                   FROM tasks t
                   JOIN task_events e ON e.event_id = (
                     SELECT candidate.event_id
                     FROM task_events candidate
                     WHERE candidate.task_id = t.task_id
                       AND candidate.event_kind = 'task.' || t.status
                     ORDER BY candidate.sequence_number DESC
                     LIMIT 1
                   )
                   WHERE t.source_conversation_id IS NOT NULL
                     AND t.status IN ('completed', 'failed', 'cancelled')
                     AND NOT EXISTS (
                       SELECT 1 FROM conversation_items item
                       WHERE item.item_id = 'item:task_completion:' || e.event_id
                   )
                   ORDER BY e.created_at, t.task_id
                   LIMIT ?1"#,
            )?;
            let rows = statement.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
}

async fn append_event(
    store: &NoemaStore,
    table: &str,
    owner_column: &str,
    owner_id: &str,
    event_id: Option<String>,
    event_kind: TaskEventKind,
    actor_id: String,
    causation_id: Option<String>,
    correlation_id: Option<String>,
    payload: Value,
) -> Result<String, StoreError> {
    if owner_id.trim().is_empty() || actor_id.trim().is_empty() {
        return Err(StoreError::InvariantViolation {
            message: "event owner, kind, and actor are required".to_string(),
        });
    }
    let event_id = event_id.unwrap_or_else(|| allocate_id("event"));
    let payload = serde_json::to_string(&payload)?;
    store.with_connection(|conn| {
        let transaction = conn.transaction()?;
        let sequence: i64 = transaction.query_row(
            &format!("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM {table} WHERE {owner_column} = ?1"),
            [owner_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            &format!("INSERT INTO {table} (event_id, {owner_column}, sequence_number, event_kind, actor_id, causation_id, correlation_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
            rusqlite::params![event_id, owner_id, sequence, event_kind.as_str(), actor_id.trim(), causation_id, correlation_id, payload],
        )?;
        transaction.commit()?;
        Ok(())
    }).await?;
    Ok(event_id)
}
