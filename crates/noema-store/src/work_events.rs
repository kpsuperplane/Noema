//! Append-only global Work event ledger helpers.
//!
//! Every writer supplies a validated [`WorkEventPayload`].  This module owns
//! the one SQL append boundary so no caller can persist an arbitrary event kind
//! paired with unrelated JSON.

use noema_tasks::{WorkDomainError, WorkEventId, WorkEventPayload, WorkEventRecord};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{Transaction, params};

use crate::{StoreError, ids::allocate_id};

pub(crate) use noema_tasks::WorkEventContext as WorkEventScope;

/// Append one typed event and return its durable record.
///
/// SQLite's `AUTOINCREMENT` rowid supplies the global sequence.  The helper
/// never computes `MAX(sequence)+1`, so concurrent transactions cannot collide
/// or reuse a cursor.  The payload is validated by the domain constructor and
/// is bounded again here before it reaches durable JSON storage.
pub(crate) fn append_work_event_tx(
    transaction: &Transaction<'_>,
    scope: WorkEventScope,
    payload: WorkEventPayload,
) -> Result<WorkEventRecord, StoreError> {
    let payload_json = serde_json::to_string(payload.as_value())?;
    if payload_json.len() > 16 * 1024 {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "work_event.payload",
            message: "payload exceeds 16 KiB".to_string(),
        }));
    }
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

/// Convert a row's persisted event payload back into the typed record while
/// rejecting unknown kinds or malformed JSON.  Read code may use this helper;
/// writers should retain the record returned from `append_work_event_tx`.
pub(crate) fn work_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkEventRecord> {
    use std::str::FromStr;

    let event_id = WorkEventId::new(row.get::<_, String>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let kind =
        noema_tasks::WorkEventKind::from_str(&row.get::<_, String>(2)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let payload_value = serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(10)?)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    // WorkEventPayload's fields are private by design; reconstruct through
    // validation by parsing the closed event payload in the domain module.
    let payload = WorkEventPayload::from_persisted(kind, payload_value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let sequence_i64 = row.get::<_, i64>(0)?;
    let event_sequence = u64::try_from(sequence_i64).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    let workspace_id = WorkspaceId::new(row.get::<_, String>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let project_id = row
        .get::<_, Option<String>>(4)?
        .map(ProjectId::new)
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let task_id = row
        .get::<_, Option<String>>(5)?
        .map(noema_tasks::TaskId::new)
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    WorkEventRecord::new(
        event_id,
        event_sequence,
        WorkEventScope {
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
    .map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
