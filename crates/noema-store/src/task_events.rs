//! Append-only global Work event ledger helpers.
//!
//! Every writer supplies a validated [`WorkEventPayload`].  This module owns
//! the one SQL append boundary so no caller can persist an arbitrary event kind
//! paired with unrelated JSON.

use std::str::FromStr;

use noema_tasks::{
    TaskId, WorkEventContext, WorkEventId, WorkEventKind, WorkEventPayload, WorkEventRecord,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{Row, Transaction, params, types::Type};

use crate::{StoreError, ids::allocate_id, sqlite::conversion_failure};

pub(crate) use noema_tasks::WorkEventContext as WorkEventScope;

pub(crate) const WORK_EVENT_COLUMNS: &str = "
    event_sequence, event_id, event_kind, workspace_id, project_id, task_id,
    run_id, actor_id, causation_id, correlation_id, payload_json, created_at
";

/// Append one typed event and return its durable record.
///
/// SQLite's `AUTOINCREMENT` rowid supplies the global sequence.  The helper
/// never computes `MAX(sequence)+1`, so concurrent transactions cannot collide
/// or reuse a cursor. The typed payload was validated by its domain constructor.
pub(crate) fn append_work_event_tx(
    transaction: &Transaction<'_>,
    scope: WorkEventScope,
    payload: WorkEventPayload,
) -> Result<WorkEventRecord, StoreError> {
    let payload_json = serde_json::to_string(payload.as_value())?;
    let event_id = WorkEventId::new(allocate_id("event")).map_err(StoreError::Work)?;
    transaction.execute(
        "INSERT INTO work_events (event_id, event_kind, workspace_id, project_id, task_id, run_id, actor_id, causation_id, correlation_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event_id.to_string(),
            payload.kind().as_str(),
            scope.workspace_id.to_string(),
            scope.project_id.as_ref().map(ToString::to_string),
            scope.task_id.as_ref().map(ToString::to_string),
            scope.run_id,
            scope.actor_id,
            scope.causation_id,
            scope.correlation_id,
            payload_json,
        ],
    )?;
    let sequence_i64 = transaction.last_insert_rowid();
    let event_sequence =
        u64::try_from(sequence_i64).map_err(|_| StoreError::InvariantViolation {
            message: format!("work event sequence is not positive: {sequence_i64}"),
        })?;
    let created_at = transaction.query_row(
        "SELECT created_at FROM work_events WHERE event_sequence = ?1",
        [sequence_i64],
        |row| row.get::<_, String>(0),
    )?;
    WorkEventRecord::new(event_id, event_sequence, scope, payload, created_at)
        .map_err(StoreError::Work)
}

/// Decode and validate one persisted event row selected with [`WORK_EVENT_COLUMNS`].
pub(crate) fn decode_work_event_record(row: &Row<'_>) -> rusqlite::Result<WorkEventRecord> {
    let raw_sequence = row.get::<_, i64>(0)?;
    let event_sequence =
        u64::try_from(raw_sequence).map_err(|error| conversion_failure(0, Type::Integer, error))?;
    let event_id = WorkEventId::new(row.get::<_, String>(1)?)
        .map_err(|error| conversion_failure(1, Type::Text, error))?;
    let kind = WorkEventKind::from_str(&row.get::<_, String>(2)?)
        .map_err(|error| conversion_failure(2, Type::Text, error))?;
    let workspace_id = WorkspaceId::new(row.get::<_, String>(3)?)
        .map_err(|error| conversion_failure(3, Type::Text, error))?;
    let project_id = row
        .get::<_, Option<String>>(4)?
        .map(ProjectId::new)
        .transpose()
        .map_err(|error| conversion_failure(4, Type::Text, error))?;
    let task_id = row
        .get::<_, Option<String>>(5)?
        .map(TaskId::new)
        .transpose()
        .map_err(|error| conversion_failure(5, Type::Text, error))?;
    let payload_value = serde_json::from_str(&row.get::<_, String>(10)?)
        .map_err(|error| conversion_failure(10, Type::Text, error))?;
    let payload = WorkEventPayload::from_persisted(kind, payload_value)
        .map_err(|error| conversion_failure(10, Type::Text, error))?;
    WorkEventRecord::new(
        event_id,
        event_sequence,
        WorkEventContext {
            workspace_id,
            project_id,
            task_id,
            run_id: row.get(6)?,
            actor_id: row.get(7)?,
            causation_id: row.get(8)?,
            correlation_id: row.get(9)?,
        },
        payload,
        row.get(11)?,
    )
    .map_err(|error| conversion_failure(0, Type::Text, error))
}
