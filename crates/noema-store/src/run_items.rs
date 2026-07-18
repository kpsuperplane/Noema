//! Durable, lease-fenced transcript items emitted by task agent runs.

use noema_tasks::{AgentRunItemKind, AgentRunItemRecord, NewAgentRunItem};
use ring::digest::{SHA256, digest};
use rusqlite::{OptionalExtension, params};

use super::{
    NoemaStore, StoreError, WorkPageInfo, WorkRunFence, WorkRunItemConnection, WorkRunItemCursor,
    WorkRunItemEdge, WorkRunItemQuery,
    ids::allocate_id,
    sqlite::{json_column, parse_column},
};

impl NoemaStore {
    /// Read a newest transcript window while returning that window chronologically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the owner scope or cursor is invalid, when
    /// stored run-item data violates cursor invariants, or when SQLite fails.
    pub async fn list_work_run_items(
        &self,
        query: WorkRunItemQuery,
    ) -> Result<WorkRunItemConnection, StoreError> {
        let run_id = query.run_id.trim().to_string();
        if run_id.is_empty() {
            return Err(invalid_run_item_cursor());
        }
        let hash = run_item_query_hash(&query);
        if query
            .before
            .as_ref()
            .is_some_and(|cursor| cursor.query_hash != hash)
        {
            return Err(invalid_run_item_cursor());
        }
        let before = query
            .before
            .map(|cursor| {
                i64::try_from(cursor.sequence_index)
                    .map(|sequence| (sequence, cursor.item_id))
                    .map_err(|_| invalid_run_item_cursor())
            })
            .transpose()?;
        let first = query.first.get();
        let limit = i64::try_from(first + 1).map_err(|_| StoreError::InvariantViolation {
            message: "run-item limit exceeds SQLite range".to_string(),
        })?;
        let workspace_id = query.owner.workspace_id.into_string();
        let task_id = query.owner.task_id.map(|id| id.into_string());
        self.with_connection(move |conn| {
            let authorized = conn
                .query_row(
                    "SELECT task.workspace_id, run.task_id
                     FROM agent_runs run JOIN tasks task ON task.task_id = run.task_id
                     WHERE run.run_id = ?1 LIMIT 1",
                    [&run_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            if authorized
                .as_ref()
                .is_none_or(|(owner_workspace, owner_task)| {
                    owner_workspace != &workspace_id
                        || task_id
                            .as_ref()
                            .is_some_and(|task_id| task_id != owner_task)
                })
            {
                return Err(StoreError::InvariantViolation {
                    message: "run is unavailable in the requested owner scope".to_string(),
                });
            }
            let mut statement = conn.prepare(
                "SELECT item_id, run_id, sequence_index, round_index, kind, status,
                        correlation_id, parent_item_id, content_text, payload_json,
                        created_at, updated_at
                 FROM agent_run_items
                 WHERE run_id = ?1 AND kind <> 'context_checkpoint'
                   AND (?2 IS NULL OR sequence_index < ?2
                    OR (sequence_index = ?2 AND item_id < ?3))
                 ORDER BY sequence_index DESC, item_id DESC LIMIT ?4",
            )?;
            let rows = statement.query_map(
                params![
                    run_id,
                    before.as_ref().map(|value| value.0),
                    before.as_ref().map(|value| value.1.as_str()),
                    limit
                ],
                run_item_from_row,
            )?;
            let mut items = rows.collect::<Result<Vec<_>, _>>()?;
            let has_next_page = items.len() > first;
            items.truncate(first);
            items.reverse();
            let edges = items
                .into_iter()
                .map(|node| {
                    let sequence = u64::try_from(node.sequence_index).map_err(|_| {
                        StoreError::InvariantViolation {
                            message: "run-item sequence is not cursor-safe".to_string(),
                        }
                    })?;
                    if sequence == 0 {
                        return Err(StoreError::InvariantViolation {
                            message: "run-item sequence is not cursor-safe".to_string(),
                        });
                    }
                    let cursor =
                        WorkRunItemCursor::new(hash.clone(), sequence, node.item_id.clone())
                            .encode();
                    Ok(WorkRunItemEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkRunItemConnection {
                page_info: WorkPageInfo {
                    end_cursor: edges.first().map(|edge| edge.cursor.clone()),
                    has_next_page,
                },
                edges,
            })
        })
        .await
    }

    /// Append one transcript item while the caller owns the active run lease.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the item or lease fence is invalid, the
    /// active lease no longer matches, serialization fails, or SQLite fails.
    pub async fn append_agent_run_item(
        &self,
        input: NewAgentRunItem,
        fence: &WorkRunFence,
    ) -> Result<AgentRunItemRecord, StoreError> {
        self.upsert_agent_run_item(input, fence).await
    }

    /// Insert or update a stable transcript item while fencing stale workers.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the item or lease fence is invalid, the
    /// active lease no longer matches, serialization fails, or SQLite fails.
    pub async fn upsert_agent_run_item(
        &self,
        input: NewAgentRunItem,
        fence: &WorkRunFence,
    ) -> Result<AgentRunItemRecord, StoreError> {
        if input.kind == AgentRunItemKind::ContextCheckpoint
            || input
                .item_id
                .as_deref()
                .is_some_and(|item_id| item_id.starts_with("run_item:context_checkpoint:"))
        {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run_item.context_checkpoint",
                    message: "context checkpoints are reserved for Store admission".to_string(),
                },
            ));
        }
        let run_id = input.run_id.trim().to_string();
        let kind = input.kind;
        fence.validate().map_err(StoreError::Work)?;
        if run_id.is_empty() || run_id != fence.run_id {
            return Err(StoreError::InvariantViolation {
                message: "run item must target the fenced run".to_string(),
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
            require_active_fence(&tx, fence)?;
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
                    params![item_id, run_id, input.round_index, kind.as_str(), input.status.as_str(), input.correlation_id, input.parent_item_id, content_text, payload],
                )?;
            } else {
                let sequence_index: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1 AND kind <> 'context_checkpoint'",
                    [&run_id],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![item_id, run_id, sequence_index, input.round_index, kind.as_str(), input.status.as_str(), input.correlation_id, input.parent_item_id, content_text, payload],
                )?;
            }
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

fn run_item_query_hash(query: &WorkRunItemQuery) -> String {
    let canonical = format!(
        "work-run-item-query:v1\0{}\0{}\0{}",
        query.owner.workspace_id.as_str(),
        query
            .owner
            .task_id
            .as_ref()
            .map_or("", noema_tasks::TaskId::as_str),
        query.run_id.trim(),
    );
    let hash = digest(&SHA256, canonical.as_bytes());
    hash.as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid_run_item_cursor() -> StoreError {
    StoreError::Work(noema_tasks::WorkDomainError::InvalidInput {
        field: "work_run_item.cursor",
        message: "invalid_cursor".to_string(),
    })
}

fn require_active_fence(
    conn: &rusqlite::Connection,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    let owns_lease = conn
        .query_row(
            "SELECT run.task_id
             FROM agent_runs run
             JOIN tasks task ON task.task_id = run.task_id
             WHERE run.run_id = ?1 AND run.lease_token = ?2
               AND run.status = 'running' AND run.cancellation_requested = 0
               AND run.lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
               AND run.task_generation = ?3 AND task.generation = ?3
               AND ((run.contract_id IS NULL AND ?4 IS NULL) OR run.contract_id = ?4)
               AND task.stage_id = 'stage:personal:doing'
               AND task.latest_run_id = run.run_id",
            params![
                fence.run_id,
                fence.lease_token,
                i64::try_from(fence.task_generation).map_err(|_| StoreError::Work(
                    noema_tasks::WorkDomainError::InvalidInput {
                        field: "run_fence.task_generation",
                        message: "generation exceeds SQLite range".to_string(),
                    }
                ))?,
                fence.contract_id.as_ref().map(ToString::to_string),
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    owns_lease
        .map(|_| ())
        .ok_or(StoreError::Work(noema_tasks::WorkDomainError::RunFenced))
}

fn run_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRunItemRecord> {
    let kind = parse_column(row, 4)?;
    let status = parse_column(row, 5)?;
    let payload = json_column(row, 9)?;
    Ok(AgentRunItemRecord {
        item_id: row.get(0)?,
        run_id: row.get(1)?,
        sequence_index: row.get(2)?,
        round_index: row.get(3)?,
        kind,
        status,
        correlation_id: row.get(6)?,
        parent_item_id: row.get(7)?,
        content_text: row.get(8)?,
        payload,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
