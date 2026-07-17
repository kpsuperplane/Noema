use rusqlite::params;

use crate::{StoreError, ids::allocate_id};

pub(super) fn ensure_fenced_write(
    changed: usize,
    run_id: &str,
    operation: &str,
) -> Result<(), StoreError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: format!("agent run lease or state changed while {operation}: {run_id}"),
        })
    }
}
pub(super) fn append_task_event(
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

pub(super) fn append_run_event(
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
            allocate_id("event"),
            run_id,
            sequence,
            event_kind,
            payload.to_string(),
        ],
    )?;
    Ok(())
}
