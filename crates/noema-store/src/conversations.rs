mod items;
mod runtime;
mod turns;

pub(crate) use items::load_conversation_item_tx;

use noema_conversations::{ConversationRecord, NewConversation};
use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, ids::allocate_id, sqlite::json_to_string};

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
                .conversation_is_owned_by_human(&conversation_id, human_id)
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
            .conversation_is_owned_by_human(&conversation_id, human_id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(ConversationRecord { conversation_id }))
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

    /// Return whether one active conversation belongs to the given human.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn conversation_is_owned_by_human(
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
}
