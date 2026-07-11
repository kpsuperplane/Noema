//! Durable, lease-fenced transcript items emitted by task agent runs.

#![allow(clippy::missing_errors_doc)]

use std::{fmt, str::FromStr};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{NoemaStore, StoreError, ids::allocate_id};

/// Lifecycle state for one correlated transcript item.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunItemStatus {
    /// Work has been declared but has not started.
    Pending,
    /// Work is currently in progress.
    Running,
    /// Work completed successfully.
    #[default]
    Completed,
    /// Work ended with an error.
    Failed,
    /// Work was cancelled.
    Cancelled,
    /// Work was intentionally not executed.
    Skipped,
}

impl AgentRunItemStatus {
    /// Return the stable persistence representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }
}

impl fmt::Display for AgentRunItemStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AgentRunItemStatus {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            _ => Err(StoreError::InvalidEnum {
                kind: "agent_run_item_status",
                value: value.to_string(),
            }),
        }
    }
}

/// Input for appending or upserting one run transcript item.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentRunItem {
    /// Optional stable id used to coalesce streaming updates.
    pub item_id: Option<String>,
    /// Owning run id.
    pub run_id: String,
    /// Zero-based provider round.
    pub round_index: i64,
    /// Transcript item kind.
    pub kind: String,
    /// Current lifecycle state.
    pub status: AgentRunItemStatus,
    /// Tool/provider correlation id.
    pub correlation_id: Option<String>,
    /// Optional parent transcript item.
    pub parent_item_id: Option<String>,
    /// Human-readable content.
    pub content_text: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

/// Persisted transcript item for one background run.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunItemRecord {
    /// Stable item id.
    pub item_id: String,
    /// Owning run id.
    pub run_id: String,
    /// Monotonic sequence within the run.
    pub sequence_index: i64,
    /// Zero-based provider round.
    pub round_index: i64,
    /// Transcript item kind.
    pub kind: String,
    /// Current lifecycle state.
    pub status: AgentRunItemStatus,
    /// Tool/provider correlation id.
    pub correlation_id: Option<String>,
    /// Optional parent transcript item.
    pub parent_item_id: Option<String>,
    /// Human-readable content.
    pub content_text: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl NoemaStore {
    /// Append one transcript item while the caller owns the active run lease.
    pub async fn append_agent_run_item(
        &self,
        input: NewAgentRunItem,
        lease_token: &str,
    ) -> Result<AgentRunItemRecord, StoreError> {
        self.upsert_agent_run_item(input, lease_token).await
    }

    /// Insert or update a stable transcript item while fencing stale workers.
    pub async fn upsert_agent_run_item(
        &self,
        input: NewAgentRunItem,
        lease_token: &str,
    ) -> Result<AgentRunItemRecord, StoreError> {
        let run_id = input.run_id.trim().to_string();
        let kind = input.kind.trim().to_string();
        if run_id.is_empty() || kind.is_empty() || lease_token.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "run item requires run, kind, and lease token".to_string(),
            });
        }
        if input.round_index < 0 {
            return Err(StoreError::InvariantViolation {
                message: "run item round cannot be negative".to_string(),
            });
        }
        let item_id = input.item_id.unwrap_or_else(|| allocate_id("run_item"));
        let content_text = input.content_text.filter(|value| !value.is_empty());
        let payload = serde_json::to_string(&input.payload)?;
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let task_id = require_active_lease(&tx, &run_id, lease_token)?;
            let existing = tx
                .query_row(
                    "SELECT run_id FROM agent_run_items WHERE item_id = ?1",
                    [&item_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(existing_run_id) = existing {
                if existing_run_id != run_id {
                    return Err(StoreError::InvariantViolation {
                        message: "run item cannot move between runs".to_string(),
                    });
                }
                tx.execute(
                    "UPDATE agent_run_items SET round_index = ?3, kind = ?4, status = ?5, correlation_id = ?6, parent_item_id = ?7, content_text = ?8, payload_json = ?9, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE item_id = ?1 AND run_id = ?2",
                    params![item_id, run_id, input.round_index, kind, input.status.as_str(), input.correlation_id, input.parent_item_id, content_text, payload],
                )?;
            } else {
                let sequence_index: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1",
                    [&run_id],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![item_id, run_id, sequence_index, input.round_index, kind, input.status.as_str(), input.correlation_id, input.parent_item_id, content_text, payload],
                )?;
            }
            let sequence_index: i64 = tx.query_row(
                "SELECT sequence_index FROM agent_run_items WHERE item_id = ?1",
                [&item_id],
                |row| row.get(0),
            )?;
            let task_event_sequence: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
                [&task_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, 'run.item_upserted', 'system:task-runtime', ?4)",
                params![
                    allocate_id("event"),
                    task_id,
                    task_event_sequence,
                    serde_json::json!({
                        "run_id": run_id,
                        "item_id": item_id,
                        "sequence_index": sequence_index,
                        "round_index": input.round_index,
                        "kind": kind,
                        "status": input.status.as_str(),
                    })
                    .to_string(),
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
        self.get_agent_run_item(&item_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("run item disappeared: {item_id}"),
            })
    }

    /// Return all transcript items in display order.
    pub async fn list_agent_run_items(
        &self,
        run_id: &str,
    ) -> Result<Vec<AgentRunItemRecord>, StoreError> {
        self.list_agent_run_items_page(run_id, None, i64::MAX).await
    }

    /// Return one forward page after an exclusive sequence cursor.
    pub async fn list_agent_run_items_page(
        &self,
        run_id: &str,
        after_sequence: Option<i64>,
        first: i64,
    ) -> Result<Vec<AgentRunItemRecord>, StoreError> {
        if first < 1 {
            return Err(StoreError::InvariantViolation {
                message: "run item page size must be positive".to_string(),
            });
        }
        let after_sequence = after_sequence.unwrap_or(0).max(0);
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM agent_run_items WHERE run_id = ?1 AND sequence_index > ?2 ORDER BY sequence_index, item_id LIMIT ?3",
            )?;
            let rows = statement.query_map(
                params![run_id, after_sequence, first],
                run_item_from_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return the newest page before an exclusive sequence cursor, ordered for display.
    pub async fn list_agent_run_items_before_page(
        &self,
        run_id: &str,
        before_sequence: Option<i64>,
        first: i64,
    ) -> Result<Vec<AgentRunItemRecord>, StoreError> {
        if first < 1 {
            return Err(StoreError::InvariantViolation {
                message: "run item page size must be positive".to_string(),
            });
        }
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM agent_run_items WHERE run_id = ?1 AND (?2 IS NULL OR sequence_index < ?2) ORDER BY sequence_index DESC, item_id DESC LIMIT ?3",
            )?;
            let rows = statement.query_map(
                params![run_id, before_sequence, first],
                run_item_from_row,
            )?;
            let mut items = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)?;
            items.reverse();
            Ok(items)
        })
        .await
    }

    /// Return the most recent transcript items in chronological order.
    pub async fn list_recent_agent_run_items(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<AgentRunItemRecord>, StoreError> {
        if limit < 1 {
            return Err(StoreError::InvariantViolation {
                message: "recent run item limit must be positive".to_string(),
            });
        }
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM (SELECT * FROM agent_run_items WHERE run_id = ?1 ORDER BY sequence_index DESC, item_id DESC LIMIT ?2) ORDER BY sequence_index, item_id",
            )?;
            let rows = statement.query_map(params![run_id, limit], run_item_from_row)?;
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
                "SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM agent_run_items WHERE item_id = ?1 LIMIT 1",
                [item_id],
                run_item_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }
}

fn require_active_lease(
    conn: &rusqlite::Connection,
    run_id: &str,
    lease_token: &str,
) -> Result<String, StoreError> {
    let owns_lease = conn
        .query_row(
            "SELECT task_id FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2 AND status IN ('leased', 'running') AND cancellation_requested = 0",
            params![run_id, lease_token],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    owns_lease.ok_or_else(|| StoreError::InvariantViolation {
        message: format!("agent run lease changed while writing transcript: {run_id}"),
    })
}

fn run_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRunItemRecord> {
    let status = row
        .get::<_, String>(5)?
        .parse::<AgentRunItemStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let payload = serde_json::from_str::<Value>(&row.get::<_, String>(9)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(AgentRunItemRecord {
        item_id: row.get(0)?,
        run_id: row.get(1)?,
        sequence_index: row.get(2)?,
        round_index: row.get(3)?,
        kind: row.get(4)?,
        status,
        correlation_id: row.get(6)?,
        parent_item_id: row.get(7)?,
        content_text: row.get(8)?,
        payload,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
