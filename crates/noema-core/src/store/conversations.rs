use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::Value;

use crate::{
    ConversationItemKind, ConversationItemPage, ConversationItemRecord, ConversationItemStatus,
    ConversationRecord, ConversationTurnRecord, NewConversation, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus as AgentStatus, ReplayMode,
};

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, now_string},
    sqlite::{json_from_string, json_to_string},
};

const CONVERSATION_ITEM_CURSOR_PREFIX: &str = "conversation_item:";
const DEFAULT_TRANSCRIPT_PAGE_LIMIT: i64 = 80;
const MAX_TRANSCRIPT_PAGE_LIMIT: i64 = 200;

impl NoemaStore {
    /// Create a durable conversation row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn create_conversation(
        &self,
        conversation: NewConversation,
    ) -> Result<ConversationRecord, StoreError> {
        let conversation_id = allocate_id("conversation");
        self.create_conversation_with_id(conversation_id, conversation)
            .await
    }

    /// Return a human's active primary conversation, creating one when needed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read or write fails.
    pub async fn get_or_create_primary_conversation(
        &self,
        human_id: &str,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<ConversationRecord, StoreError> {
        self.get_or_create_primary_conversation_for_provider(human_id, "codex", model, cwd)
            .await
    }

    /// Return a human's active primary conversation, creating one for the provider when needed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read or write fails.
    pub async fn get_or_create_primary_conversation_for_provider(
        &self,
        human_id: &str,
        provider: &str,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<ConversationRecord, StoreError> {
        self.ensure_default_actors().await?;
        let primary_conversation_id = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT primary_conversation_id FROM humans WHERE human_id = ?1 LIMIT 1",
                    [human_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map(|row| row.flatten())
                .map_err(StoreError::Sqlite)
            })
            .await?;
        if let Some(conversation_id) = primary_conversation_id
            && self
                .primary_conversation_matches_human(&conversation_id, human_id)
                .await?
        {
            return Ok(ConversationRecord { conversation_id });
        }

        let record = self
            .create_conversation_with_id(
                allocate_id("conversation"),
                NewConversation::local_chat_for_provider(provider, model, cwd),
            )
            .await?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE humans
                SET primary_conversation_id = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE human_id = ?1
                "#,
                params![human_id, record.conversation_id],
            )?;
            Ok(())
        })
        .await?;
        Ok(record)
    }

    /// Return a human's active primary conversation without creating one.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn primary_conversation_for_human(
        &self,
        human_id: &str,
    ) -> Result<Option<ConversationRecord>, StoreError> {
        let conversation_id = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT primary_conversation_id FROM humans WHERE human_id = ?1 LIMIT 1",
                    [human_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map(|row| row.flatten())
                .map_err(StoreError::Sqlite)
            })
            .await?;
        let Some(conversation_id) = conversation_id else {
            return Ok(None);
        };
        if !self
            .primary_conversation_matches_human(&conversation_id, human_id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(ConversationRecord { conversation_id }))
    }

    /// Return the next durable turn index for a conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store read fails.
    pub async fn next_conversation_turn_index(
        &self,
        conversation_id: &str,
    ) -> Result<u64, StoreError> {
        self.require_conversation(conversation_id).await?;
        let metadata_values = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    "SELECT metadata_json FROM conversation_turns WHERE conversation_id = ?1",
                )?;
                let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        let mut max_index = 0;
        for metadata_json in metadata_values {
            let metadata = json_from_string(metadata_json)?;
            if let Some(index) = metadata.get("turn_index").and_then(Value::as_u64) {
                max_index = max_index.max(index);
            }
        }
        Ok(max_index.saturating_add(1))
    }

    /// Create a durable turn row for an existing conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store write fails.
    pub async fn create_conversation_turn(
        &self,
        turn: NewConversationTurn,
    ) -> Result<ConversationTurnRecord, StoreError> {
        self.require_conversation(&turn.conversation_id).await?;
        if let Some(trigger_item_id) = &turn.trigger_item_id {
            self.require_conversation_item_for_conversation(trigger_item_id, &turn.conversation_id)
                .await?;
        }
        let turn_id = allocate_id("turn");
        let metadata_json = json_to_string(&turn.metadata)?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO conversation_turns
                  (turn_id, conversation_id, trigger_item_id, status, metadata_json, started_at, updated_at)
                VALUES (?1, ?2, ?3, 'input_received', ?4,
                  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                params![turn_id, turn.conversation_id, turn.trigger_item_id, metadata_json],
            )?;
            Ok(())
        })
        .await?;
        Ok(ConversationTurnRecord {
            turn_id,
            conversation_id: turn.conversation_id,
        })
    }

    /// Idempotently create a turn with a caller-derived stable id.
    ///
    /// This is used by durable projections whose worker may be interrupted
    /// after creating a turn but before appending its first item.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the turn id is invalid, a referenced
    /// conversation or trigger item is missing, or the embedded store write
    /// fails.
    pub async fn create_conversation_turn_with_id_if_absent(
        &self,
        turn_id: String,
        turn: NewConversationTurn,
    ) -> Result<(ConversationTurnRecord, bool), StoreError> {
        if turn_id.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "conversation turn id cannot be empty".to_string(),
            });
        }
        self.require_conversation(&turn.conversation_id).await?;
        if let Some(trigger_item_id) = &turn.trigger_item_id {
            self.require_conversation_item_for_conversation(trigger_item_id, &turn.conversation_id)
                .await?;
        }
        let metadata_json = json_to_string(&turn.metadata)?;
        let (conversation_id, inserted) = self
            .with_connection(|conn| {
                if let Some(existing_conversation_id) = conn
                    .query_row(
                        "SELECT conversation_id FROM conversation_turns WHERE turn_id = ?1 LIMIT 1",
                        [&turn_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                {
                    if existing_conversation_id != turn.conversation_id {
                        return Err(StoreError::InvariantViolation {
                            message: format!(
                                "idempotent conversation turn belongs to another conversation: {turn_id}"
                            ),
                        });
                    }
                    return Ok((existing_conversation_id, false));
                }
                conn.execute(
                    r#"
                    INSERT INTO conversation_turns
                      (turn_id, conversation_id, trigger_item_id, status, metadata_json, started_at, updated_at)
                    VALUES (?1, ?2, ?3, 'input_received', ?4,
                      strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                      strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    "#,
                    params![turn_id, turn.conversation_id, turn.trigger_item_id, metadata_json],
                )?;
                Ok((turn.conversation_id.clone(), true))
            })
            .await?;
        Ok((
            ConversationTurnRecord {
                turn_id,
                conversation_id,
            },
            inserted,
        ))
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
                        json_to_string(&item.payload_json)?,
                        json_to_string(&item.metadata)?,
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

    /// Return text transcript items after a compacted context checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn list_conversation_items_after_sequence_for_context(
        &self,
        conversation_id: &str,
        after_sequence_index: i64,
        limit: i64,
    ) -> Result<Vec<ConversationItemRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let limit = limit.clamp(1, 80);
        let rows = self
            .with_connection(|conn| {
                collect_conversation_item_rows(
                    conn,
                    r#"
                    WHERE conversation_id = ?1
                      AND deleted_at IS NULL
                      AND sequence_index > ?2
                      AND kind IN ('user_text', 'assistant_text')
                    ORDER BY sequence_index ASC
                    LIMIT ?3
                    "#,
                    params![conversation_id, after_sequence_index, limit],
                )
            })
            .await?;
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
                      AND kind IN ('user_text', 'assistant_text', 'tool_call', 'tool_result', 'reasoning')
                    ORDER BY sequence_index ASC
                    "#,
                    params![conversation_id, after_sequence_index],
                )
            })
            .await?;
        rows.into_iter().map(conversation_item_from_row).collect()
    }

    /// Mark a durable conversation turn completed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the turn is missing or the embedded store
    /// write fails.
    pub async fn complete_conversation_turn(&self, turn_id: &str) -> Result<(), StoreError> {
        self.update_turn_status(turn_id, "completed").await
    }

    /// Mark a durable conversation turn failed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the turn is missing or the embedded store
    /// write fails.
    pub async fn fail_conversation_turn(&self, turn_id: &str) -> Result<(), StoreError> {
        self.update_turn_status(turn_id, "failed").await
    }

    /// Update the live agent status for a durable conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store write fails.
    pub async fn update_conversation_agent_status(
        &self,
        conversation_id: &str,
        status: AgentStatus,
    ) -> Result<(), StoreError> {
        self.require_conversation(conversation_id).await?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE conversations
                SET agent_status = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                "#,
                params![conversation_id, status.as_str()],
            )?;
            Ok(())
        })
        .await
    }

    /// Recover in-flight conversation state after process shutdown cancels runtime work.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the transactional recovery write fails.
    pub(crate) async fn recover_shutdown_cancelled_work(
        &self,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute(
                r#"
                UPDATE conversation_items
                SET status = 'cancelled',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                  AND status IN ('pending', 'running')
                  AND turn_id IN (
                    SELECT turn_id
                    FROM conversation_turns
                    WHERE conversation_id = ?1
                      AND status IN ('input_received', 'running', 'waiting_for_tool')
                  )
                "#,
                [conversation_id],
            )?;
            transaction.execute(
                r#"
                UPDATE conversation_turns
                SET status = 'cancelled',
                    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                  AND status IN ('input_received', 'running', 'waiting_for_tool')
                "#,
                [conversation_id],
            )?;
            transaction.execute(
                r#"
                UPDATE conversations
                SET agent_status = 'idle',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                "#,
                [conversation_id],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn create_conversation_with_id(
        &self,
        conversation_id: String,
        conversation: NewConversation,
    ) -> Result<ConversationRecord, StoreError> {
        let metadata_json = json_to_string(&conversation.metadata)?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO conversations
                  (conversation_id, title, owner_object_type, owner_object_id, primary_human_id,
                   primary_agent_id, provider, model, cwd, lifecycle_status, agent_status, metadata_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active', 'idle', ?10)
                "#,
                params![
                    conversation_id,
                    conversation.title,
                    conversation.owner.object_type.as_str(),
                    conversation.owner.object_id.to_string(),
                    conversation.primary_human_id,
                    conversation.primary_agent_id,
                    conversation.provider,
                    conversation.model,
                    conversation.cwd,
                    metadata_json,
                ],
            )?;
            Ok(())
        })
        .await?;
        Ok(ConversationRecord { conversation_id })
    }

    async fn primary_conversation_matches_human(
        &self,
        conversation_id: &str,
        human_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT EXISTS(
                  SELECT 1
                  FROM conversations
                  WHERE conversation_id = ?1
                    AND owner_object_type = 'human'
                    AND owner_object_id = ?2
                    AND primary_human_id = ?2
                    AND lifecycle_status = 'active'
                    AND deleted_at IS NULL
                )
                "#,
                params![conversation_id, human_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    async fn conversation_exists(&self, conversation_id: &str) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT EXISTS(
                  SELECT 1
                  FROM conversations
                  WHERE conversation_id = ?1
                    AND lifecycle_status = 'active'
                    AND deleted_at IS NULL
                )
                "#,
                [conversation_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    pub(crate) async fn require_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        if self.conversation_exists(conversation_id).await? {
            Ok(())
        } else {
            Err(StoreError::ConversationNotFound {
                conversation_id: conversation_id.to_string(),
            })
        }
    }

    async fn require_turn(&self, turn_id: &str) -> Result<(), StoreError> {
        let exists = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE turn_id = ?1)",
                    [turn_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(StoreError::Sqlite)
            })
            .await?;
        if exists {
            Ok(())
        } else {
            Err(StoreError::ConversationTurnNotFound {
                turn_id: turn_id.to_string(),
            })
        }
    }

    async fn require_turn_for_conversation(
        &self,
        turn_id: &str,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT conversation_id FROM conversation_turns WHERE turn_id = ?1 LIMIT 1",
                    [turn_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        let Some(row_conversation_id) = row else {
            return Err(StoreError::ConversationTurnNotFound {
                turn_id: turn_id.to_string(),
            });
        };
        if row_conversation_id == conversation_id {
            Ok(())
        } else {
            Err(StoreError::ConversationTurnConversationMismatch {
                turn_id: turn_id.to_string(),
                conversation_id: conversation_id.to_string(),
            })
        }
    }

    async fn require_conversation_item_for_conversation(
        &self,
        item_id: &str,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    r#"
                    SELECT conversation_id
                    FROM conversation_items
                    WHERE item_id = ?1
                      AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                    [item_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        let Some(row_conversation_id) = row else {
            return Err(StoreError::ConversationItemNotFound {
                item_id: item_id.to_string(),
            });
        };
        if row_conversation_id == conversation_id {
            Ok(())
        } else {
            Err(StoreError::ConversationItemConversationMismatch {
                item_id: item_id.to_string(),
                conversation_id: conversation_id.to_string(),
            })
        }
    }

    async fn update_turn_status(&self, turn_id: &str, status: &str) -> Result<(), StoreError> {
        self.require_turn(turn_id).await?;
        let completed_at = now_string();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE conversation_turns
                SET status = ?2,
                    completed_at = ?3,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE turn_id = ?1
                "#,
                params![turn_id, status, completed_at],
            )?;
            Ok(())
        })
        .await
    }
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
        kind: ConversationItemKind::parse(&row.kind).map_err(memory_enum_error)?,
        status: ConversationItemStatus::parse(&row.status).map_err(memory_enum_error)?,
        content_text: row.content_text,
        payload_json: json_from_string(row.payload_json)?,
        metadata: json_from_string(row.metadata_json)?,
    })
}

fn memory_enum_error(error: crate::MemoryPersistenceError) -> StoreError {
    match error {
        crate::MemoryPersistenceError::InvalidEnum { kind, value } => {
            StoreError::InvalidEnum { kind, value }
        }
        other => StoreError::Schema(other.to_string()),
    }
}
