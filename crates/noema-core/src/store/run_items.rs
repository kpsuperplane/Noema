//! Durable transcript items emitted by background agent runs.

#![allow(clippy::missing_errors_doc)]

use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use super::{NoemaStore, StoreError, ids::allocate_id};

/// Input for one run transcript item.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentRunItem {
    /// Optional stable item id.
    pub item_id: Option<String>,
    /// Run that emitted the item.
    pub run_id: String,
    /// Transcript kind, such as `model_input`, `assistant_output`, `tool_call`, or `tool_result`.
    pub kind: String,
    /// Human-readable text, when present.
    pub content_text: Option<String>,
    /// Structured event payload.
    pub payload: Value,
}

/// Persisted transcript item for one background run.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunItemRecord {
    /// Stable item id.
    pub item_id: String,
    /// Owning run id.
    pub run_id: String,
    /// Monotonic display order within the run.
    pub sequence_index: i64,
    /// Transcript kind.
    pub kind: String,
    /// Human-readable text, when present.
    pub content_text: Option<String>,
    /// Structured event payload.
    pub payload: Value,
    /// Creation timestamp.
    pub created_at: String,
}

impl NoemaStore {
    /// Append one transcript item and allocate its per-run sequence.
    pub async fn append_agent_run_item(
        &self,
        input: NewAgentRunItem,
    ) -> Result<AgentRunItemRecord, StoreError> {
        let run_id = input.run_id.trim().to_string();
        let kind = input.kind.trim().to_string();
        if run_id.is_empty() || kind.is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "run activity requires run and kind".to_string(),
            });
        }
        let item_id = input.item_id.unwrap_or_else(|| allocate_id("run_item"));
        // Preserve the exact streamed text. The provider may split a word or
        // sentence across deltas, so trimming here would corrupt the live
        // transcript when the UI joins adjacent assistant-output items.
        let content_text = input.content_text.filter(|value| !value.is_empty());
        let payload = serde_json::to_string(&input.payload)?;
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let sequence_index: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1",
                [&run_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO agent_run_items (item_id, run_id, sequence_index, kind, content_text, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item_id, run_id, sequence_index, kind, content_text, payload],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        self.get_agent_run_item(&item_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("run activity item disappeared: {item_id}"),
            })
    }

    /// Return transcript items in display order.
    pub async fn list_agent_run_items(
        &self,
        run_id: &str,
    ) -> Result<Vec<AgentRunItemRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT item_id, run_id, sequence_index, kind, content_text, payload_json, created_at FROM agent_run_items WHERE run_id = ?1 ORDER BY sequence_index, item_id",
            )?;
            let rows = statement.query_map([run_id], run_item_from_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        })
        .await
    }

    async fn get_agent_run_item(
        &self,
        item_id: &str,
    ) -> Result<Option<AgentRunItemRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT item_id, run_id, sequence_index, kind, content_text, payload_json, created_at FROM agent_run_items WHERE item_id = ?1 LIMIT 1",
                [item_id],
                run_item_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }
}

fn run_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRunItemRecord> {
    let payload = serde_json::from_str::<Value>(&row.get::<_, String>(5)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(AgentRunItemRecord {
        item_id: row.get(0)?,
        run_id: row.get(1)?,
        sequence_index: row.get(2)?,
        kind: row.get(3)?,
        content_text: row.get(4)?,
        payload,
        created_at: row.get(6)?,
    })
}
