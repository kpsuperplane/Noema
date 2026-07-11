//! Append-only task and run event persistence.

use serde_json::Value;

use super::{NoemaStore, StoreError, ids::allocate_id};

/// One append-only task lifecycle event.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTaskEvent {
    /// Optional stable event id.
    pub event_id: Option<String>,
    /// Task owning the event.
    pub task_id: String,
    /// Event name.
    pub event_kind: String,
    /// Actor/component responsible.
    pub actor_id: String,
    /// Direct cause, when present.
    pub causation_id: Option<String>,
    /// Cross-run correlation id, when present.
    pub correlation_id: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

/// One append-only run event.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRunEvent {
    /// Optional stable event id.
    pub event_id: Option<String>,
    /// Run owning the event.
    pub run_id: String,
    /// Event name.
    pub event_kind: String,
    /// Actor/component responsible.
    pub actor_id: String,
    /// Direct cause, when present.
    pub causation_id: Option<String>,
    /// Cross-run correlation id, when present.
    pub correlation_id: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

impl NoemaStore {
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
}

async fn append_event(
    store: &NoemaStore,
    table: &str,
    owner_column: &str,
    owner_id: &str,
    event_id: Option<String>,
    event_kind: String,
    actor_id: String,
    causation_id: Option<String>,
    correlation_id: Option<String>,
    payload: Value,
) -> Result<String, StoreError> {
    if owner_id.trim().is_empty() || event_kind.trim().is_empty() || actor_id.trim().is_empty() {
        return Err(StoreError::InvariantViolation {
            message: "event owner, kind, and actor are required".to_string(),
        });
    }
    let event_id = event_id.unwrap_or_else(|| allocate_id("event"));
    let payload = serde_json::to_string(&payload)?;
    store.with_connection(|conn| {
        let sequence: i64 = conn.query_row(
            &format!("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM {table} WHERE {owner_column} = ?1"),
            [owner_id],
            |row| row.get(0),
        )?;
        conn.execute(
            &format!("INSERT INTO {table} (event_id, {owner_column}, sequence_number, event_kind, actor_id, causation_id, correlation_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
            rusqlite::params![event_id, owner_id, sequence, event_kind.trim(), actor_id.trim(), causation_id, correlation_id, payload],
        )?;
        Ok(())
    }).await?;
    Ok(event_id)
}
