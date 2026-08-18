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
    pub contributor_instance_names: Vec<String>,
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
        contributor_instance_names: load_contributor_instance_names(transaction, task_id)?,
    })
}

fn load_contributor_instance_names(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<Vec<String>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT instance_name FROM agent_runs WHERE task_id = ?1 \
         GROUP BY instance_name ORDER BY MIN(created_at), instance_name",
    )?;
    statement
        .query_map([task_id.as_str()], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params};

    #[test]
    fn contributor_names_include_every_distinct_task_run_instance() {
        let mut connection = Connection::open_in_memory().expect("open SQLite");
        connection
            .execute_batch(
                "CREATE TABLE agent_runs (
                    task_id TEXT NOT NULL,
                    instance_name TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )",
            )
            .expect("create agent runs");
        let transaction = connection.transaction().expect("start transaction");
        let task_id = TaskId::new("task:contributors").expect("task id");
        for index in 0..=DETAIL_HISTORY_LIMIT {
            transaction
                .execute(
                    "INSERT INTO agent_runs (task_id, instance_name, created_at) VALUES (?1, ?2, ?3)",
                    params![task_id.as_str(), format!("Agent {index:02}"), format!("2026-01-01T00:00:{index:02}Z")],
                )
                .expect("insert contributor");
        }
        transaction
            .execute(
                "INSERT INTO agent_runs (task_id, instance_name, created_at) VALUES (?1, 'Agent 00', '2026-01-01T00:01:00Z')",
                [task_id.as_str()],
            )
            .expect("insert repeated contributor");
        transaction
            .execute(
                "INSERT INTO agent_runs (task_id, instance_name, created_at) VALUES ('task:other', 'Other Agent', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("insert other task contributor");

        let contributors =
            load_contributor_instance_names(&transaction, &task_id).expect("load contributors");

        assert_eq!(contributors.len(), DETAIL_HISTORY_LIMIT + 1);
        assert_eq!(contributors.first().map(String::as_str), Some("Agent 00"));
        assert_eq!(contributors.last().map(String::as_str), Some("Agent 20"));
        assert!(!contributors.iter().any(|name| name == "Other Agent"));
    }
}
