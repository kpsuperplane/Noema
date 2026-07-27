use noema_conversations::{
    ConversationItemKind, ConversationItemPage, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem, ReplayMode,
};
use rusqlite::params;

use super::{NoemaStore, StoreError};
use crate::{
    ids::allocate_id,
    sqlite::{deserialize_json, serialize_json},
};

const CONVERSATION_ITEM_CURSOR_PREFIX: &str = "conversation_item:";
const DEFAULT_TRANSCRIPT_PAGE_LIMIT: i64 = 80;
const MAX_TRANSCRIPT_PAGE_LIMIT: i64 = 200;

impl NoemaStore {
    /// Return the latest durable provider-context reset boundary.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store read fails.
    pub async fn latest_context_reset_sequence(
        &self,
        conversation_id: &str,
    ) -> Result<i64, StoreError> {
        self.require_conversation(conversation_id).await?;
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT COALESCE(MAX(sequence_index), 0)
                FROM conversation_items
                WHERE conversation_id = ?1
                  AND deleted_at IS NULL
                  AND kind = 'activity'
                  AND status = 'completed'
                  AND json_extract(payload_json, '$.activity_kind') = 'context_reset'
                "#,
                [conversation_id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Capture one finite conversation head and its completed text evidence in
    /// a single SQLite snapshot. Human text is evidence; assistant text is
    /// bounded context for the memory model.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the snapshot
    /// query cannot be completed.
    pub async fn capture_memory_source_range(
        &self,
        conversation_id: &str,
        last_consolidated_sequence: i64,
    ) -> Result<MemoryConversationSourceRange, StoreError> {
        self.require_conversation(conversation_id).await?;
        self.with_connection(|conn| {
            let captured_head_sequence = conn.query_row(
                "SELECT COALESCE(MAX(sequence_index), 0) FROM conversation_items WHERE conversation_id = ?1 AND deleted_at IS NULL",
                [conversation_id],
                |row| row.get::<_, i64>(0),
            )?;
            let rows = collect_conversation_item_rows(
                conn,
                r#"
                WHERE conversation_id = ?1
                  AND deleted_at IS NULL
                  AND sequence_index > ?2
                  AND sequence_index <= ?3
                  AND status = 'completed'
                  AND kind IN ('user_text', 'assistant_text')
                ORDER BY sequence_index ASC
                "#,
                params![conversation_id, last_consolidated_sequence, captured_head_sequence],
            )?;
            let items = rows
                .into_iter()
                .map(conversation_item_from_row)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(MemoryConversationSourceRange {
                conversation_id: conversation_id.to_string(),
                captured_head_sequence,
                items,
            })
        })
        .await
    }

    /// Append a durable item to a conversation stream.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when referenced conversation or turn rows are
    /// missing, or the embedded store write/read fails.
    pub async fn append_conversation_item(
        &self,
        item: NewConversationItem,
    ) -> Result<ConversationItemRecord, StoreError> {
        self.append_conversation_item_with_id(allocate_id("item"), item)
            .await
    }

    /// Idempotently append a durable item with a caller-derived stable id.
    /// This is reserved for exactly-once projections of another durable event.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the id is empty, ownership references are
    /// invalid, or an existing id belongs to a different projection.
    pub async fn append_conversation_item_with_id(
        &self,
        item_id: String,
        item: NewConversationItem,
    ) -> Result<ConversationItemRecord, StoreError> {
        self.append_conversation_item_with_id_if_absent(item_id, item)
            .await
            .map(|(record, _)| record)
    }

    /// Idempotently append an item and report whether this call inserted it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation, insertion, or readback fails.
    pub async fn append_conversation_item_with_id_if_absent(
        &self,
        item_id: String,
        item: NewConversationItem,
    ) -> Result<(ConversationItemRecord, bool), StoreError> {
        if item_id.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "conversation item id cannot be empty".to_string(),
            });
        }
        self.require_conversation(&item.conversation_id).await?;
        if let Some(turn_id) = &item.turn_id {
            self.require_turn_for_conversation(turn_id, &item.conversation_id)
                .await?;
        }
        if let Some(parent_item_id) = &item.parent_item_id {
            self.require_conversation_item_for_conversation(parent_item_id, &item.conversation_id)
                .await?;
        }
        let _append_guard = self.append_item_lock.lock().await;
        let (sequence_index, inserted) = self
            .with_connection(|conn| {
                if let Some(existing) = collect_conversation_item_rows(
                    conn,
                    "WHERE item_id = ?1 LIMIT 1",
                    params![item_id],
                )?
                .into_iter()
                .next()
                {
                    let existing = conversation_item_from_row(existing)?;
                    if existing.conversation_id != item.conversation_id
                        || existing.kind != item.kind
                    {
                        return Err(StoreError::InvariantViolation {
                            message: format!(
                                "idempotent conversation item id belongs to another projection: {item_id}"
                            ),
                        });
                    }
                    return Ok((existing.sequence_index, false));
                }
                let next_sequence = conn.query_row(
                    "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM conversation_items WHERE conversation_id = ?1",
                    [&item.conversation_id],
                    |row| row.get::<_, i64>(0),
                )?;
                conn.execute(
                    r#"
                    INSERT INTO conversation_items
                      (item_id, conversation_id, turn_id, parent_item_id, sequence_index, kind, status,
                       author_actor_id, content_text, payload_json, metadata_json)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                    "#,
                    params![
                        item_id,
                        item.conversation_id,
                        item.turn_id,
                        item.parent_item_id,
                        next_sequence,
                        item.kind.as_str(),
                        item.status.as_str(),
                        item.author.actor_id.to_string(),
                        item.content_text,
                        serialize_json(&item.payload_json)?,
                        serialize_json(&item.metadata)?,
                    ],
                )?;
                Ok((next_sequence, true))
            })
            .await?;
        if !inserted {
            let existing = self
                .with_connection(|conn| {
                    collect_conversation_item_rows(
                        conn,
                        "WHERE item_id = ?1 LIMIT 1",
                        params![item_id],
                    )?
                    .into_iter()
                    .next()
                    .map(conversation_item_from_row)
                    .transpose()?
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: "idempotent conversation item disappeared".to_string(),
                    })
                })
                .await?;
            return Ok((existing, false));
        }
        Ok((
            ConversationItemRecord {
                item_id: item_id.clone(),
                conversation_id: item.conversation_id,
                turn_id: item.turn_id,
                sequence_index,
                cursor: conversation_item_cursor(sequence_index),
                kind: item.kind,
                status: item.status,
                content_text: item.content_text,
                payload_json: item.payload_json,
                metadata: item.metadata,
            },
            true,
        ))
    }

    /// List conversation items in replay order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn list_conversation_items(
        &self,
        conversation_id: &str,
        mode: ReplayMode,
    ) -> Result<Vec<ConversationItemRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let deleted_filter = match mode {
            ReplayMode::Visible => "AND deleted_at IS NULL",
            ReplayMode::Audit => "",
        };
        let rows = self
            .with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    format!(
                        "WHERE conversation_id = ?1 {deleted_filter} ORDER BY sequence_index ASC"
                    )
                    .as_str(),
                    params![conversation_id],
                )
            })
            .await?;
        rows.into_iter().map(conversation_item_from_row).collect()
    }

    /// Return one visible conversation item by durable item id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored enums
    /// are invalid.
    pub async fn get_visible_conversation_item(
        &self,
        item_id: &str,
    ) -> Result<Option<ConversationItemRecord>, StoreError> {
        let rows = self
            .with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE item_id = ?1
                      AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                    params![item_id],
                )
            })
            .await?;
        rows.into_iter()
            .next()
            .map(conversation_item_from_row)
            .transpose()
    }

    /// Return a bounded page of visible conversation items in transcript order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the cursor is
    /// malformed, the embedded store read fails, or stored enums are invalid.
    pub async fn list_visible_conversation_item_page(
        &self,
        conversation_id: &str,
        cursor: Option<&str>,
        limit: i64,
    ) -> Result<ConversationItemPage, StoreError> {
        self.require_conversation(conversation_id).await?;
        let limit = if limit == 0 {
            DEFAULT_TRANSCRIPT_PAGE_LIMIT
        } else {
            clamp_transcript_page_limit(limit)
        };
        let fetch_limit = limit.saturating_add(1);

        let mut rows = if let Some(cursor) = cursor {
            let before_sequence_index = sequence_index_from_conversation_item_cursor(cursor)?;
            self.with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND kind <> 'model_context_update'
                      AND sequence_index < ?2
                    ORDER BY sequence_index DESC
                    LIMIT ?3
                    "#,
                    params![conversation_id, before_sequence_index, fetch_limit],
                )
            })
            .await?
        } else {
            self.with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND kind <> 'model_context_update'
                    ORDER BY sequence_index DESC
                    LIMIT ?2
                    "#,
                    params![conversation_id, fetch_limit],
                )
            })
            .await?
        };

        let has_more_before = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
        if has_more_before {
            rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        rows.reverse();
        let items = rows
            .into_iter()
            .map(conversation_item_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        let before_cursor = items.first().map(|item| item.cursor.clone());

        Ok(ConversationItemPage {
            items,
            before_cursor,
            has_more_before,
            limit,
        })
    }

    /// Return recent text transcript items for provider context.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn list_recent_conversation_items_for_context(
        &self,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<ConversationItemRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let limit = limit.clamp(1, 40);
        let mut rows = self
            .with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND kind IN ('user_text', 'assistant_text')
                    ORDER BY sequence_index DESC
                    LIMIT ?2
                    "#,
                    params![conversation_id, limit],
                )
            })
            .await?;
        rows.reverse();
        rows.into_iter().map(conversation_item_from_row).collect()
    }

    /// Return all transcript items after a compacted context checkpoint that
    /// can contribute to model context.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn list_all_conversation_items_after_sequence_for_context(
        &self,
        conversation_id: &str,
        after_sequence_index: i64,
    ) -> Result<Vec<ConversationItemRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let rows = self
            .with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND sequence_index > ?2
                      AND kind IN ('user_text', 'assistant_text', 'tool_call', 'tool_result', 'reasoning', 'model_context_update')
                    ORDER BY sequence_index ASC
                    "#,
                    params![conversation_id, after_sequence_index],
                )
            })
            .await?;
        rows.into_iter().map(conversation_item_from_row).collect()
    }
}

/// Finite source range captured for one native-memory update.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryConversationSourceRange {
    /// Conversation id used by the query.
    pub conversation_id: String,
    /// Maximum sequence index captured before reading rows.
    pub captured_head_sequence: i64,
    /// Completed user and assistant text items in sequence order.
    pub items: Vec<ConversationItemRecord>,
}

fn collect_conversation_item_rows<P>(
    conn: &rusqlite::Connection,
    clause: &str,
    params: P,
) -> Result<Vec<ConversationItemRow>, StoreError>
where
    P: rusqlite::Params,
{
    let mut statement = conn.prepare(
        format!(
            r#"
            SELECT item_id, conversation_id, turn_id, kind, status, content_text,
              payload_json, metadata_json, sequence_index
            FROM conversation_items
            {clause}
            "#
        )
        .as_str(),
    )?;
    let rows = statement.query_map(params, conversation_item_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

pub(crate) fn load_conversation_item_tx(
    transaction: &rusqlite::Transaction<'_>,
    item_id: &str,
) -> Result<Option<ConversationItemRecord>, StoreError> {
    collect_conversation_item_rows(transaction, "WHERE item_id = ?1 LIMIT 1", params![item_id])?
        .into_iter()
        .next()
        .map(conversation_item_from_row)
        .transpose()
}

#[derive(Debug)]
struct ConversationItemRow {
    item_id: String,
    conversation_id: String,
    turn_id: Option<String>,
    sequence_index: i64,
    kind: String,
    status: String,
    content_text: Option<String>,
    payload_json: String,
    metadata_json: String,
}

fn conversation_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationItemRow> {
    Ok(ConversationItemRow {
        item_id: row.get(0)?,
        conversation_id: row.get(1)?,
        turn_id: row.get(2)?,
        kind: row.get(3)?,
        status: row.get(4)?,
        content_text: row.get(5)?,
        payload_json: row.get(6)?,
        metadata_json: row.get(7)?,
        sequence_index: row.get(8)?,
    })
}

fn conversation_item_cursor(sequence_index: i64) -> String {
    format!("{CONVERSATION_ITEM_CURSOR_PREFIX}{sequence_index}")
}

fn sequence_index_from_conversation_item_cursor(cursor: &str) -> Result<i64, StoreError> {
    let raw = cursor
        .strip_prefix(CONVERSATION_ITEM_CURSOR_PREFIX)
        .ok_or_else(|| StoreError::Schema(format!("invalid conversation item cursor: {cursor}")))?;
    let sequence_index = raw
        .parse::<i64>()
        .map_err(|_| StoreError::Schema(format!("invalid conversation item cursor: {cursor}")))?;
    if sequence_index < 1 {
        return Err(StoreError::Schema(format!(
            "invalid conversation item cursor: {cursor}"
        )));
    }
    Ok(sequence_index)
}

fn clamp_transcript_page_limit(limit: i64) -> i64 {
    limit.clamp(1, MAX_TRANSCRIPT_PAGE_LIMIT)
}

fn conversation_item_from_row(
    row: ConversationItemRow,
) -> Result<ConversationItemRecord, StoreError> {
    let cursor = conversation_item_cursor(row.sequence_index);
    Ok(ConversationItemRecord {
        item_id: row.item_id,
        conversation_id: row.conversation_id,
        turn_id: row.turn_id,
        sequence_index: row.sequence_index,
        cursor,
        kind: ConversationItemKind::parse(&row.kind).map_err(StoreError::from)?,
        status: ConversationItemStatus::parse(&row.status).map_err(StoreError::from)?,
        content_text: row.content_text,
        payload_json: deserialize_json(row.payload_json)?,
        metadata: deserialize_json(row.metadata_json)?,
    })
}
