use noema_conversations::{ConversationTurnRecord, NewConversationTurn};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use super::{NoemaStore, StoreError};
use crate::{
    ids::{allocate_id, now_string},
    sqlite::{deserialize_json, serialize_json},
};

impl NoemaStore {
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
            let metadata = deserialize_json::<Value>(metadata_json)?;
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
        let metadata_json = serialize_json(&turn.metadata)?;
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
        let metadata_json = serialize_json(&turn.metadata)?;
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
