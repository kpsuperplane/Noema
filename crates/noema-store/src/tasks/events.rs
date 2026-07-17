use crate::ids::allocate_id;

pub(super) fn append_task_event_tx(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    tx.execute("INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", rusqlite::params![allocate_id("event"), task_id, sequence, kind, actor_id, payload.to_string()])?;
    Ok(())
}

pub(super) fn append_run_event_tx(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM run_events WHERE run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            allocate_id("event"),
            run_id,
            sequence,
            kind,
            actor_id,
            payload.to_string()
        ],
    )?;
    Ok(())
}
