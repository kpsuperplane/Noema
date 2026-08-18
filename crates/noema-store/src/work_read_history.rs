use super::list_rows::load_runs;
use crate::{
    StoreError,
    sqlite::conversion_failure,
    work_row::{optional_id, positive_u64},
};
use noema_tasks::{
    AgentRunRecord, ApprovalDecision, TaskGateId, TaskId, TaskMessageId, TaskMessageKind,
    TaskMessageRecord,
};
use rusqlite::{Row, Transaction, params, types::Type};
use std::{collections::HashMap, str::FromStr};

const DETAIL_HISTORY_LIMIT: usize = 20;

pub(crate) struct WorkTaskHistory {
    pub messages: Vec<TaskMessageRecord>,
    pub runs: Vec<AgentRunRecord>,
}

pub(crate) fn load_task_history(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<WorkTaskHistory, StoreError> {
    Ok(WorkTaskHistory {
        messages: load_recent(
            transaction,
            task_id,
            "task_messages",
            "message_id",
            "created_at",
            load_messages,
            "message",
        )?,
        runs: load_recent(
            transaction,
            task_id,
            "agent_runs",
            "run_id",
            "created_at",
            load_runs,
            "run",
        )?,
    })
}

fn load_recent<T: Clone>(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
    table: &'static str,
    id_column: &'static str,
    timestamp_column: &'static str,
    loader: impl FnOnce(&Transaction<'_>, &[String]) -> Result<HashMap<String, T>, StoreError>,
    kind: &'static str,
) -> Result<Vec<T>, StoreError> {
    let sql = format!(
        "SELECT {id_column} FROM {table} WHERE task_id = ?1 \
         ORDER BY {timestamp_column} DESC, {id_column} DESC LIMIT ?2"
    );
    let ids = transaction
        .prepare(&sql)?
        .query_map(
            params![task_id.as_str(), DETAIL_HISTORY_LIMIT as i64],
            |row| row.get(0),
        )?
        .collect::<Result<Vec<String>, _>>()?;
    let records = loader(transaction, &ids)?;
    ids.iter()
        .map(|id| {
            records
                .get(id)
                .cloned()
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("history references a missing {kind}: {id}"),
                })
        })
        .collect()
}

fn load_messages(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskMessageRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT message_id, task_id, task_generation, gate_id,
                message_kind, body_markdown, approval_decision, author_actor_id,
                consumed_by_run_id, consumed_at, created_at
         FROM task_messages WHERE message_id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids_json], decode_message)?;
    let mut records = HashMap::new();
    for row in rows {
        let message = row?;
        validate_message(&message)?;
        records.insert(message.message_id.as_str().to_string(), message);
    }
    Ok(records)
}

pub(crate) fn decode_message(row: &Row<'_>) -> rusqlite::Result<TaskMessageRecord> {
    Ok(TaskMessageRecord {
        message_id: TaskMessageId::new(row.get::<_, String>(0)?)
            .map_err(|error| conversion_failure(0, Type::Text, error))?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|error| conversion_failure(1, Type::Text, error))?,
        task_generation: positive_u64(row, 2)?,
        gate_id: optional_id(row, 3, TaskGateId::new)?,
        kind: TaskMessageKind::from_str(&row.get::<_, String>(4)?)
            .map_err(|error| conversion_failure(4, Type::Text, error))?,
        body_markdown: row.get(5)?,
        approval_decision: row
            .get::<_, Option<String>>(6)?
            .map(|value| ApprovalDecision::from_str(&value))
            .transpose()
            .map_err(|error| conversion_failure(6, Type::Text, error))?,
        author_actor_id: row.get(7)?,
        consumed_by_run_id: row.get(8)?,
        consumed_at: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn validate_message(message: &TaskMessageRecord) -> Result<(), StoreError> {
    let consumed = message.consumed_by_run_id.is_some() == message.consumed_at.is_some();
    let decision =
        message.kind == TaskMessageKind::HumanAnswer || message.approval_decision.is_none();
    if message.body_markdown.trim().is_empty()
        || message.author_actor_id.trim().is_empty()
        || message.created_at.trim().is_empty()
        || !consumed
        || !decision
    {
        return Err(StoreError::InvariantViolation {
            message: "task message is not canonically valid".to_string(),
        });
    }
    Ok(())
}
