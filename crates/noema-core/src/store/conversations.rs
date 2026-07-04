use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    ConversationItemKind, ConversationItemPage, ConversationItemRecord, ConversationItemStatus,
    ConversationRecord, ConversationTurnRecord, NewConversation, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus as AgentStatus, ReplayMode,
};

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, now_string, record_fragment},
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
        let mut response = self
            .db
            .query("SELECT primary_conversation_id FROM humans WHERE human_id = $human_id LIMIT 1;")
            .bind(("human_id", human_id.to_string()))
            .await?;
        let rows: Vec<PrimaryConversationRow> = response.take(0)?;
        if let Some(Some(conversation_id)) = rows
            .into_iter()
            .next()
            .map(|row| row.primary_conversation_id)
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
        self.db
            .query(
                r#"
                UPDATE humans SET
                  primary_conversation_id = $conversation_id,
                  updated_at = time::now()
                WHERE human_id = $human_id;
                "#,
            )
            .bind(("human_id", human_id.to_string()))
            .bind(("conversation_id", record.conversation_id.clone()))
            .await?
            .check()?;
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
        let mut response = self
            .db
            .query("SELECT primary_conversation_id FROM humans WHERE human_id = $human_id LIMIT 1;")
            .bind(("human_id", human_id.to_string()))
            .await?;
        let rows: Vec<PrimaryConversationRow> = response.take(0)?;
        let Some(Some(conversation_id)) = rows
            .into_iter()
            .next()
            .map(|row| row.primary_conversation_id)
        else {
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
        let mut response = self
            .db
            .query(
                "SELECT metadata FROM conversation_turns WHERE conversation_id = $conversation_id;",
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .await?;
        let rows: Vec<MetadataRow> = response.take(0)?;
        Ok(rows
            .iter()
            .filter_map(|row| row.metadata.get("turn_index").and_then(Value::as_u64))
            .max()
            .unwrap_or(0)
            .saturating_add(1))
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
        self.db
            .query(
                r#"
                CREATE type::record('conversation_turns', $record_id) SET
                  turn_id = $turn_id,
                  conversation_id = $conversation_id,
                  trigger_item_id = $trigger_item_id,
                  status = 'input_received',
                  metadata = $metadata,
                  started_at = time::now(),
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&turn_id)))
            .bind(("turn_id", turn_id.clone()))
            .bind(("conversation_id", turn.conversation_id.clone()))
            .bind(("trigger_item_id", turn.trigger_item_id))
            .bind(("metadata", turn.metadata))
            .await?
            .check()?;
        Ok(ConversationTurnRecord {
            turn_id,
            conversation_id: turn.conversation_id,
        })
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
        let item_id = allocate_id("item");
        let sequence_index = self.next_item_sequence_index(&item.conversation_id).await?;
        self.db
            .query(
                r#"
                CREATE type::record('conversation_items', $record_id) SET
                  item_id = $item_id,
                  conversation_id = $conversation_id,
                  turn_id = $turn_id,
                  parent_item_id = $parent_item_id,
                  sequence_index = $sequence_index,
                  kind = $kind,
                  status = $status,
                  author_actor_id = $author_actor_id,
                  content_text = $content_text,
                  payload_json = $payload_json,
                  metadata = $metadata,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&item_id)))
            .bind(("item_id", item_id.clone()))
            .bind(("conversation_id", item.conversation_id.clone()))
            .bind(("turn_id", item.turn_id.clone()))
            .bind(("parent_item_id", item.parent_item_id))
            .bind(("sequence_index", sequence_index))
            .bind(("kind", item.kind.as_str().to_string()))
            .bind(("status", item.status.as_str().to_string()))
            .bind(("author_actor_id", item.author.actor_id.to_string()))
            .bind(("content_text", item.content_text.clone()))
            .bind(("payload_json", item.payload_json.clone()))
            .bind(("metadata", item.metadata))
            .await?
            .check()?;
        Ok(ConversationItemRecord {
            item_id,
            conversation_id: item.conversation_id,
            turn_id: item.turn_id,
            sequence_index,
            cursor: conversation_item_cursor(sequence_index),
            kind: item.kind,
            status: item.status,
            content_text: item.content_text,
            payload_json: item.payload_json,
        })
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
            ReplayMode::Visible => "AND deleted_at = NONE",
            ReplayMode::Audit => "",
        };
        let mut response = self
            .db
            .query(format!(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id {deleted_filter}
                ORDER BY sequence_index ASC;
                "#
            ))
            .bind(("conversation_id", conversation_id.to_string()))
            .await?;
        let rows: Vec<ConversationItemRow> = response.take(0)?;
        rows.into_iter().map(conversation_item_from_row).collect()
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
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                    FROM conversation_items
                    WHERE conversation_id = $conversation_id
                      AND deleted_at = NONE
                      AND sequence_index < $before_sequence_index
                    ORDER BY sequence_index DESC
                    LIMIT $limit;
                    "#,
                )
                .bind(("conversation_id", conversation_id.to_string()))
                .bind(("before_sequence_index", before_sequence_index))
                .bind(("limit", fetch_limit))
                .await?;
            response.take::<Vec<ConversationItemRow>>(0)?
        } else {
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                    FROM conversation_items
                    WHERE conversation_id = $conversation_id
                      AND deleted_at = NONE
                    ORDER BY sequence_index DESC
                    LIMIT $limit;
                    "#,
                )
                .bind(("conversation_id", conversation_id.to_string()))
                .bind(("limit", fetch_limit))
                .await?;
            response.take::<Vec<ConversationItemRow>>(0)?
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id
                  AND deleted_at = NONE
                  AND kind IN ['user_text', 'assistant_text']
                ORDER BY sequence_index DESC
                LIMIT $limit;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("limit", limit))
            .await?;
        let mut rows: Vec<ConversationItemRow> = response.take(0)?;
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id
                  AND deleted_at = NONE
                  AND sequence_index > $after_sequence_index
                  AND kind IN ['user_text', 'assistant_text']
                ORDER BY sequence_index ASC
                LIMIT $limit;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("after_sequence_index", after_sequence_index))
            .bind(("limit", limit))
            .await?;
        let rows: Vec<ConversationItemRow> = response.take(0)?;
        rows.into_iter().map(conversation_item_from_row).collect()
    }

    /// Return all text transcript items after a compacted context checkpoint.
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id
                  AND deleted_at = NONE
                  AND sequence_index > $after_sequence_index
                  AND kind IN ['user_text', 'assistant_text']
                ORDER BY sequence_index ASC;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("after_sequence_index", after_sequence_index))
            .await?;
        let rows: Vec<ConversationItemRow> = response.take(0)?;
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
        self.db
            .query(
                r#"
                UPDATE conversations SET
                  agent_status = $status,
                  updated_at = time::now()
                WHERE conversation_id = $conversation_id;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("status", status.as_str().to_string()))
            .await?
            .check()?;
        Ok(())
    }

    async fn create_conversation_with_id(
        &self,
        conversation_id: String,
        conversation: NewConversation,
    ) -> Result<ConversationRecord, StoreError> {
        self.db
            .query(
                r#"
                CREATE type::record('conversations', $record_id) SET
                  conversation_id = $conversation_id,
                  title = $title,
                  owner_object_type = $owner_object_type,
                  owner_object_id = $owner_object_id,
                  primary_human_id = $primary_human_id,
                  primary_agent_id = $primary_agent_id,
                  provider = $provider,
                  model = $model,
                  cwd = $cwd,
                  lifecycle_status = 'active',
                  agent_status = 'idle',
                  metadata = $metadata,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&conversation_id)))
            .bind(("conversation_id", conversation_id.clone()))
            .bind(("title", conversation.title))
            .bind((
                "owner_object_type",
                conversation.owner.object_type.as_str().to_string(),
            ))
            .bind(("owner_object_id", conversation.owner.object_id.to_string()))
            .bind(("primary_human_id", conversation.primary_human_id))
            .bind(("primary_agent_id", conversation.primary_agent_id))
            .bind(("provider", conversation.provider))
            .bind(("model", conversation.model))
            .bind(("cwd", conversation.cwd))
            .bind(("metadata", conversation.metadata))
            .await?
            .check()?;
        Ok(ConversationRecord { conversation_id })
    }

    async fn primary_conversation_matches_human(
        &self,
        conversation_id: &str,
        human_id: &str,
    ) -> Result<bool, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT conversation_id
                FROM conversations
                WHERE conversation_id = $conversation_id
                  AND owner_object_type = 'human'
                  AND owner_object_id = $human_id
                  AND primary_human_id = $human_id
                  AND lifecycle_status = 'active'
                  AND deleted_at = NONE
                LIMIT 1;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("human_id", human_id.to_string()))
            .await?;
        let rows: Vec<ConversationIdRow> = response.take(0)?;
        Ok(!rows.is_empty())
    }

    async fn conversation_exists(&self, conversation_id: &str) -> Result<bool, StoreError> {
        let mut response = self
            .db
            .query(
                "SELECT conversation_id FROM conversations WHERE conversation_id = $conversation_id AND lifecycle_status = 'active' AND deleted_at = NONE LIMIT 1;",
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .await?;
        let rows: Vec<ConversationIdRow> = response.take(0)?;
        Ok(!rows.is_empty())
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
        let mut response = self
            .db
            .query("SELECT turn_id FROM conversation_turns WHERE turn_id = $turn_id LIMIT 1;")
            .bind(("turn_id", turn_id.to_string()))
            .await?;
        let rows: Vec<TurnIdRow> = response.take(0)?;
        if rows.is_empty() {
            Err(StoreError::ConversationTurnNotFound {
                turn_id: turn_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn require_turn_for_conversation(
        &self,
        turn_id: &str,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        let mut response = self
            .db
            .query("SELECT turn_id, conversation_id FROM conversation_turns WHERE turn_id = $turn_id LIMIT 1;")
            .bind(("turn_id", turn_id.to_string()))
            .await?;
        let rows: Vec<TurnRefRow> = response.take(0)?;
        let Some(row) = rows.first() else {
            return Err(StoreError::ConversationTurnNotFound {
                turn_id: turn_id.to_string(),
            });
        };
        if row.conversation_id == conversation_id {
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
        let mut response = self
            .db
            .query("SELECT item_id, conversation_id FROM conversation_items WHERE item_id = $item_id AND deleted_at = NONE LIMIT 1;")
            .bind(("item_id", item_id.to_string()))
            .await?;
        let rows: Vec<ConversationItemRefRow> = response.take(0)?;
        let Some(row) = rows.first() else {
            return Err(StoreError::ConversationItemNotFound {
                item_id: item_id.to_string(),
            });
        };
        if row.conversation_id == conversation_id {
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
        self.db
            .query(
                r#"
                UPDATE conversation_turns SET
                  status = $status,
                  completed_at = $completed_at,
                  updated_at = time::now()
                WHERE turn_id = $turn_id;
                "#,
            )
            .bind(("turn_id", turn_id.to_string()))
            .bind(("status", status.to_string()))
            .bind(("completed_at", Some(now_string())))
            .await?
            .check()?;
        Ok(())
    }

    async fn next_item_sequence_index(&self, conversation_id: &str) -> Result<i64, StoreError> {
        let mut response = self
            .db
            .query(
                "SELECT sequence_index FROM conversation_items WHERE conversation_id = $conversation_id ORDER BY sequence_index DESC LIMIT 1;",
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .await?;
        let rows: Vec<SequenceRow> = response.take(0)?;
        Ok(rows
            .first()
            .map_or(1, |row| row.sequence_index.saturating_add(1)))
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct PrimaryConversationRow {
    primary_conversation_id: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ConversationIdRow {
    #[allow(dead_code)]
    conversation_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct TurnIdRow {
    #[allow(dead_code)]
    turn_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct TurnRefRow {
    #[allow(dead_code)]
    turn_id: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ConversationItemRefRow {
    #[allow(dead_code)]
    item_id: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct SequenceRow {
    sequence_index: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct MetadataRow {
    metadata: Value,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ConversationItemRow {
    item_id: String,
    conversation_id: String,
    turn_id: Option<String>,
    sequence_index: i64,
    kind: String,
    status: String,
    content_text: Option<String>,
    payload_json: Value,
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
        payload_json: row.payload_json,
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
