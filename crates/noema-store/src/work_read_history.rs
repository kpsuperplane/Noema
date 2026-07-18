//! Independent query-bound keyset connections for immutable task histories.

use std::{collections::HashMap, str::FromStr};

use noema_tasks::{
    ApprovalDecision, TaskGateId, TaskGateRecord, TaskId, TaskMessageId, TaskMessageKind,
    TaskMessageRecord,
};
use ring::digest::{SHA256, digest};
use rusqlite::{Row, Transaction, params, types::Type};

use super::{
    evidence::load_contracts,
    list_rows::{load_reviews, load_runs},
    rows::decode_gate,
    submission_batch::load_submissions,
};
use crate::{
    NoemaStore, StoreError, WorkContractConnection, WorkContractCursor, WorkContractEdge,
    WorkContractHistoryQuery, WorkGateConnection, WorkGateCursor, WorkGateEdge,
    WorkGateHistoryQuery, WorkMessageConnection, WorkMessageCursor, WorkMessageEdge,
    WorkMessageHistoryQuery, WorkPageInfo, WorkReviewConnection, WorkReviewCursor, WorkReviewEdge,
    WorkReviewHistoryQuery, WorkRunConnection, WorkRunCursor, WorkRunEdge, WorkRunHistoryQuery,
    WorkSubmissionConnection, WorkSubmissionCursor, WorkSubmissionEdge, WorkSubmissionHistoryQuery,
    sqlite::conversion_failure,
};

struct HistoryKey {
    id: String,
    created_at: String,
}

impl NoemaStore {
    /// List fully hydrated immutable contracts, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_contracts(
        &self,
        query: WorkContractHistoryQuery,
    ) -> Result<WorkContractConnection, StoreError> {
        let hash = validate_cursor(
            "contract",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "task_execution_contracts",
                "contract_id",
                "created_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_contracts(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "contract")?;
                    let cursor = WorkContractCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkContractEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkContractConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }

    /// List strict gate history, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_gates(
        &self,
        query: WorkGateHistoryQuery,
    ) -> Result<WorkGateConnection, StoreError> {
        let hash = validate_cursor(
            "gate",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "task_gates",
                "gate_id",
                "opened_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_history_gates(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "gate")?;
                    let cursor = WorkGateCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkGateEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkGateConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }

    /// List strict human-message history, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_messages(
        &self,
        query: WorkMessageHistoryQuery,
    ) -> Result<WorkMessageConnection, StoreError> {
        let hash = validate_cursor(
            "message",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "task_messages",
                "message_id",
                "created_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_messages(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "message")?;
                    let cursor = WorkMessageCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkMessageEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkMessageConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }

    /// List strict historical runs, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_runs(
        &self,
        query: WorkRunHistoryQuery,
    ) -> Result<WorkRunConnection, StoreError> {
        let hash = validate_cursor(
            "run",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "agent_runs",
                "run_id",
                "created_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_runs(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "run")?;
                    let cursor = WorkRunCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkRunEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkRunConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }

    /// List fully hydrated immutable submissions, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_submissions(
        &self,
        query: WorkSubmissionHistoryQuery,
    ) -> Result<WorkSubmissionConnection, StoreError> {
        let hash = validate_cursor(
            "submission",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "task_submissions",
                "submission_id",
                "created_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_submissions(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "submission")?;
                    let cursor = WorkSubmissionCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkSubmissionEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkSubmissionConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }

    /// List fully hydrated immutable reviews, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid query/cursor input or failed hydration.
    pub async fn list_work_task_reviews(
        &self,
        query: WorkReviewHistoryQuery,
    ) -> Result<WorkReviewConnection, StoreError> {
        let hash = validate_cursor(
            "review",
            &query.task_id,
            query
                .after
                .as_ref()
                .map(|cursor| cursor.query_hash.as_str()),
        )?;
        let after = query.after.map(|cursor| (cursor.created_at, cursor.id));
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let (keys, has_next_page) = load_keys(
                &transaction,
                "task_reviews",
                "review_id",
                "created_at",
                &query.task_id,
                after,
                query.first.get(),
            )?;
            let records = load_reviews(&transaction, &ids(&keys))?;
            let edges = keys
                .into_iter()
                .map(|key| {
                    let node = exact(&records, &key.id, "review")?;
                    let cursor = WorkReviewCursor::new(hash.clone(), key.created_at, key.id)
                        .map_err(|_| invariant("history cursor timestamp is not canonical"))?
                        .encode();
                    Ok(WorkReviewEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkReviewConnection {
                page_info: page_info(&edges, has_next_page, |edge| &edge.cursor),
                edges,
            })
        })
        .await
    }
}

fn load_keys(
    transaction: &Transaction<'_>,
    table: &'static str,
    id_column: &'static str,
    timestamp_column: &'static str,
    task_id: &TaskId,
    after: Option<(String, String)>,
    first: usize,
) -> Result<(Vec<HistoryKey>, bool), StoreError> {
    let limit = i64::try_from(first + 1).map_err(|_| invariant("history limit overflow"))?;
    let (after_created_at, after_id) = after.unzip();
    let sql = format!(
        "SELECT {id_column}, {timestamp_column} FROM {table}
         WHERE task_id = ?1 AND (
           ?2 IS NULL OR {timestamp_column} < ?2
           OR ({timestamp_column} = ?2 AND {id_column} < ?3)
         )
         ORDER BY {timestamp_column} DESC, {id_column} DESC LIMIT ?4"
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map(
        params![task_id.as_str(), after_created_at, after_id, limit],
        |row| {
            Ok(HistoryKey {
                id: row.get(0)?,
                created_at: row.get(1)?,
            })
        },
    )?;
    let mut keys = rows.collect::<Result<Vec<_>, _>>()?;
    if keys.iter().any(|key| {
        key.id.trim().is_empty()
            || key.created_at.trim().is_empty()
            || key.id.chars().any(char::is_control)
            || key.created_at.chars().any(char::is_control)
    }) {
        return Err(invariant("history key is blank"));
    }
    let has_next_page = keys.len() > first;
    keys.truncate(first);
    Ok((keys, has_next_page))
}

fn load_history_gates(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskGateRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let mut statement = transaction.prepare(
        "SELECT gate_id, task_id, task_generation, contract_id, gate_kind, gate_state,
                recovery_reason, retry_run_kind, prompt_markdown, context_markdown,
                opened_by_actor_id, originating_run_id, resolved_by_actor_id,
                resolution_message_id, opened_at, resolved_at
         FROM task_gates WHERE gate_id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids_json], decode_gate)?;
    let mut records = HashMap::new();
    for row in rows {
        let gate = row?;
        gate.validate().map_err(StoreError::Work)?;
        records.insert(gate.gate_id.as_str().to_string(), gate);
    }
    exact_count(ids, &records, "gate")?;
    Ok(records)
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
        "SELECT message_id, task_id, task_generation, contract_id, gate_id, review_id,
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
    exact_count(ids, &records, "message")?;
    Ok(records)
}

fn decode_message(row: &Row<'_>) -> rusqlite::Result<TaskMessageRecord> {
    Ok(TaskMessageRecord {
        message_id: TaskMessageId::new(row.get::<_, String>(0)?)
            .map_err(|error| conversion_failure(0, Type::Text, error))?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|error| conversion_failure(1, Type::Text, error))?,
        task_generation: positive_u64(row, 2)?,
        contract_id: row
            .get::<_, Option<String>>(3)?
            .map(noema_tasks::TaskContractId::new)
            .transpose()
            .map_err(|error| conversion_failure(3, Type::Text, error))?,
        gate_id: row
            .get::<_, Option<String>>(4)?
            .map(TaskGateId::new)
            .transpose()
            .map_err(|error| conversion_failure(4, Type::Text, error))?,
        review_id: row.get(5)?,
        kind: TaskMessageKind::from_str(&row.get::<_, String>(6)?)
            .map_err(|error| conversion_failure(6, Type::Text, error))?,
        body_markdown: row.get(7)?,
        approval_decision: row
            .get::<_, Option<String>>(8)?
            .map(|value| ApprovalDecision::from_str(&value))
            .transpose()
            .map_err(|error| conversion_failure(8, Type::Text, error))?,
        author_actor_id: row.get(9)?,
        consumed_by_run_id: row.get(10)?,
        consumed_at: row.get(11)?,
        created_at: row.get(12)?,
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
        return Err(invariant("task message is not canonically valid"));
    }
    Ok(())
}

fn validate_cursor(
    family: &str,
    task_id: &TaskId,
    cursor_hash: Option<&str>,
) -> Result<String, StoreError> {
    let hash = query_hash(family, task_id);
    if cursor_hash.is_some_and(|cursor_hash| cursor_hash != hash) {
        Err(StoreError::Work(
            noema_tasks::WorkDomainError::InvalidInput {
                field: "work_history.cursor",
                message: "invalid_cursor".to_string(),
            },
        ))
    } else {
        Ok(hash)
    }
}

fn query_hash(family: &str, task_id: &TaskId) -> String {
    let canonical = format!("work-history-query:v1\0{family}\0{}", task_id.as_str());
    let hash = digest(&SHA256, canonical.as_bytes());
    hash.as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn ids(keys: &[HistoryKey]) -> Vec<String> {
    keys.iter().map(|key| key.id.clone()).collect()
}

fn exact<T: Clone>(
    records: &HashMap<String, T>,
    id: &str,
    kind: &'static str,
) -> Result<T, StoreError> {
    records
        .get(id)
        .cloned()
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("history references a missing {kind}: {id}"),
        })
}

fn exact_count<T>(
    ids: &[String],
    records: &HashMap<String, T>,
    kind: &'static str,
) -> Result<(), StoreError> {
    if records.len() == ids.len() && ids.iter().all(|id| records.contains_key(id)) {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: format!("history references a missing {kind}"),
        })
    }
}

fn page_info<T>(edges: &[T], has_next_page: bool, cursor: impl Fn(&T) -> &String) -> WorkPageInfo {
    WorkPageInfo {
        end_cursor: edges.last().map(cursor).cloned(),
        has_next_page,
    }
}

fn positive_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = u64::try_from(row.get::<_, i64>(index)?)
        .map_err(|error| conversion_failure(index, Type::Integer, error))?;
    if value == 0 {
        Err(conversion_failure(
            index,
            Type::Integer,
            std::io::Error::new(std::io::ErrorKind::InvalidData, "expected positive integer"),
        ))
    } else {
        Ok(value)
    }
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
