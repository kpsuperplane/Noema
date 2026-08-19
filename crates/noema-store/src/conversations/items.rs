use noema_conversations::{
    ConversationItemKind, ConversationItemPage, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem, ReplayMode,
};
use rusqlite::{Transaction, params};

use super::{NoemaStore, StoreError};
use crate::{
    ids::allocate_id,
    sqlite::{deserialize_json, serialize_json},
};

const CONVERSATION_ITEM_CURSOR_PREFIX: &str = "conversation_item:";
const DEFAULT_TRANSCRIPT_PAGE_LIMIT: i64 = 80;
const MAX_TRANSCRIPT_PAGE_LIMIT: i64 = 200;

impl NoemaStore {
    /// Save one final tool result and finish its exact call in one transaction.
    ///
    /// A repeat with the same result returns the first saved result. A repeat
    /// with different data fails.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the result is not final, the call does not
    /// match, the repeat has different data, or SQLite cannot save the change.
    pub async fn finish_conversation_tool_call(
        &self,
        call_item_id: &str,
        result: NewConversationItem,
    ) -> Result<(ConversationItemRecord, bool), StoreError> {
        if result.kind != ConversationItemKind::ToolResult
            || !matches!(
                result.status,
                ConversationItemStatus::Completed
                    | ConversationItemStatus::Failed
                    | ConversationItemStatus::Cancelled
                    | ConversationItemStatus::Interrupted
            )
        {
            return Err(invariant("tool result must have a final status"));
        }
        let result_item_id = format!(
            "item:tool_result:{}",
            call_item_id.strip_prefix("item:").unwrap_or(call_item_id)
        );
        let _append_guard = self.append_item_lock.lock().await;
        self.with_immediate_transaction_retry(|tx| {
            let call = load_conversation_item(tx, call_item_id)?
                .ok_or_else(|| invariant("tool call was not found"))?;
            if call.kind != ConversationItemKind::ToolCall
                || call.conversation_id != result.conversation_id
                || call.turn_id != result.turn_id
            {
                return Err(invariant("tool result does not match its call"));
            }
            if let Some(saved) = load_conversation_item(tx, &result_item_id)? {
                if saved.kind == result.kind
                    && saved.status == result.status
                    && saved.conversation_id == result.conversation_id
                    && saved.turn_id == result.turn_id
                    && saved.content_text == result.content_text
                    && saved.payload_json == result.payload_json
                    && saved.metadata == result.metadata
                    && call.status == result.status
                {
                    return Ok((saved, false));
                }
                return Err(invariant("repeated tool result has different data"));
            }
            if !matches!(
                call.status,
                ConversationItemStatus::Pending | ConversationItemStatus::Running
            ) {
                return Err(invariant("tool call already has a different final state"));
            }
            let saved = append_conversation_item_tx(tx, result_item_id.clone(), result.clone())?;
            if tx.execute(
                "UPDATE conversation_items SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE item_id = ?1 AND status IN ('pending', 'running')",
                params![call_item_id, result.status.as_str()],
            )? != 1
            {
                return Err(invariant("tool call changed before its result was saved"));
            }
            Ok((saved, true))
        })
        .await
    }

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

    /// Capture one finite conversation head and its completed memory context in
    /// a single SQLite snapshot. Human text and exact tool results are evidence;
    /// assistant text is bounded context for the memory model.
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
                  AND (kind IN ('user_text', 'assistant_text', 'tool_result')
                    OR (kind = 'activity'
                      AND json_extract(metadata_json, '$.source') = 'provider_action'
                      AND json_extract(payload_json, '$.activity_kind') = 'tool_result'))
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
        self.append_conversation_item_with_provider_text(allocate_id("item"), item, None)
            .await
    }

    /// Append one provider-generated assistant item with its provider text.
    ///
    /// The provider text is saved only when it differs from readable text.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the item is not readable assistant text, or
    /// when its ownership references or embedded store write are invalid.
    pub async fn append_provider_conversation_item(
        &self,
        item: NewConversationItem,
        provider_text: String,
    ) -> Result<ConversationItemRecord, StoreError> {
        if item.kind != ConversationItemKind::AssistantText || item.content_text.is_none() {
            return Err(invariant(
                "provider conversation item must contain assistant text",
            ));
        }
        let provider_text =
            (item.content_text.as_deref() != Some(provider_text.as_str())).then_some(provider_text);
        self.append_conversation_item_with_provider_text(allocate_id("item"), item, provider_text)
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
        self.append_conversation_item_with_id_and_provider_text_if_absent(item_id, item, None)
            .await
    }

    async fn append_conversation_item_with_provider_text(
        &self,
        item_id: String,
        item: NewConversationItem,
        provider_content_text: Option<String>,
    ) -> Result<ConversationItemRecord, StoreError> {
        self.append_conversation_item_with_id_and_provider_text_if_absent(
            item_id,
            item,
            provider_content_text,
        )
        .await
        .map(|(record, _)| record)
    }

    async fn append_conversation_item_with_id_and_provider_text_if_absent(
        &self,
        item_id: String,
        item: NewConversationItem,
        provider_content_text: Option<String>,
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
                       author_actor_id, content_text, provider_content_text, payload_json, metadata_json)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
                        provider_content_text,
                        serialize_json(&item.payload_json)?,
                        serialize_json(&item.metadata)?,
                    ],
                )?;
                Ok((next_sequence, true))
            })
            .await?;
        let record = self
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
                    message: "conversation item disappeared after append".to_string(),
                })
            })
            .await?;
        debug_assert_eq!(record.sequence_index, sequence_index);
        Ok((record, inserted))
    }

    #[cfg(test)]
    pub(crate) async fn stored_provider_content_text(
        &self,
        item_id: &str,
    ) -> Result<Option<String>, StoreError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT provider_content_text FROM conversation_items WHERE item_id = ?1",
                    [item_id],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
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

    /// Return unresolved chat-driven MCP setup results in newest-first order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or stored rows are invalid.
    pub async fn list_pending_mcp_setup_items(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationItemRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        let rows = self
            .with_connection(|connection| {
                collect_conversation_item_rows(
                    connection,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND json_extract(payload_json, '$.metadata.action.name') = 'mcp.connect_service'
                      AND json_extract(payload_json, '$.metadata.action.success') = 1
                      AND json_extract(payload_json, '$.metadata.action.payload.status')
                        IN ('needs_auth', 'authentication_available', 'ready_for_policy')
                      AND json_extract(payload_json, '$.metadata.action.payload.intervention_resolution') IS NULL
                    ORDER BY sequence_index DESC
                    LIMIT ?2
                    "#,
                    params![conversation_id, limit],
                )
            })
            .await?;
        rows.into_iter().map(conversation_item_from_row).collect()
    }

    /// Mark one exact chat-driven MCP setup result resolved by a configured server.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store update fails.
    pub async fn resolve_mcp_setup_item(
        &self,
        conversation_id: &str,
        item_id: &str,
        mcp_server_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                r#"
                UPDATE conversation_items
                SET payload_json = json_set(
                  payload_json,
                  '$.metadata.action.payload.intervention_resolution',
                  json_object('mcp_server_id', ?3)
                )
                WHERE conversation_id = ?1
                  AND item_id = ?2
                  AND deleted_at IS NULL
                  AND json_extract(payload_json, '$.metadata.action.name') = 'mcp.connect_service'
                  AND json_extract(payload_json, '$.metadata.action.success') = 1
                  AND json_extract(payload_json, '$.metadata.action.payload.intervention_resolution') IS NULL
                "#,
                params![conversation_id, item_id, mcp_server_id],
            )?;
            Ok(changed == 1)
        })
        .await
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

    /// Return all transcript items after a compacted context checkpoint that
    /// can contribute content or structured replay eligibility to model context.
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
                      AND (kind IN ('user_text', 'assistant_text', 'tool_call', 'tool_result', 'reasoning', 'model_context_update')
                        OR (kind = 'activity'
                          AND json_extract(metadata_json, '$.source') = 'provider_action'
                          AND json_extract(payload_json, '$.metadata.action.name') = 'web.search'))
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
    /// Completed human text, assistant context, and tool evidence in sequence order.
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
              payload_json, metadata_json, sequence_index, created_at
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

pub(crate) fn load_conversation_item(
    connection: &rusqlite::Connection,
    item_id: &str,
) -> Result<Option<ConversationItemRecord>, StoreError> {
    collect_conversation_item_rows(connection, "WHERE item_id = ?1 LIMIT 1", params![item_id])?
        .into_iter()
        .next()
        .map(conversation_item_from_row)
        .transpose()
}

pub(crate) fn append_conversation_item_tx(
    tx: &Transaction<'_>,
    item_id: String,
    item: NewConversationItem,
) -> Result<ConversationItemRecord, StoreError> {
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM conversation_items WHERE conversation_id = ?1",
        [&item.conversation_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO conversation_items (item_id, conversation_id, turn_id, parent_item_id, sequence_index, kind, status, author_actor_id, content_text, payload_json, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![item_id, item.conversation_id, item.turn_id, item.parent_item_id, sequence, item.kind.as_str(), item.status.as_str(), item.author.actor_id.to_string(), item.content_text, serialize_json(&item.payload_json)?, serialize_json(&item.metadata)?],
    )?;
    load_conversation_item(tx, &item_id)?
        .ok_or_else(|| invariant("conversation item disappeared before commit"))
}

fn invariant(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
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
    created_at: String,
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
        created_at: row.get(9)?,
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
        created_at: row.created_at,
    })
}
