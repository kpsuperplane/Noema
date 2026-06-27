use serde_json::{Value, json};

use super::{
    ObjectRef, ObjectType,
    error::MemoryPersistenceError,
    objects::validate_object_ref_for_pool,
    postgres_helpers::{allocate_id as allocate_postgres_id, json_value},
    repository::PostgresMemoryRepository,
};

/// Live agent coordination state for a durable conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// No agent work is currently active.
    Idle,
    /// Human or external input has been accepted.
    InputReceived,
    /// The agent is producing or planning a response.
    Thinking,
    /// The agent is waiting on a tool invocation.
    ToolRunning,
    /// A newer turn is waiting for a prior turn's side effects to settle.
    WaitingForPreviousTurnCompletion,
    /// The agent is interrupting a previous turn.
    Interrupting,
    /// The conversation is in an error state.
    Error,
}

impl AgentStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::InputReceived => "input_received",
            Self::Thinking => "thinking",
            Self::ToolRunning => "tool_running",
            Self::WaitingForPreviousTurnCompletion => "waiting_for_previous_turn_completion",
            Self::Interrupting => "interrupting",
            Self::Error => "error",
        }
    }

    /// Parse a stored status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "idle" => Ok(Self::Idle),
            "input_received" => Ok(Self::InputReceived),
            "thinking" => Ok(Self::Thinking),
            "tool_running" => Ok(Self::ToolRunning),
            "waiting_for_previous_turn_completion" => Ok(Self::WaitingForPreviousTurnCompletion),
            "interrupting" => Ok(Self::Interrupting),
            "error" => Ok(Self::Error),
            _ => invalid_enum("agent_status", value),
        }
    }
}

/// Durable lifecycle state for one causal conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationTurnStatus {
    /// Input has been recorded but work has not started.
    InputReceived,
    /// The turn is actively running.
    Running,
    /// The turn is waiting for a tool result.
    WaitingForTool,
    /// The turn was interrupted by newer input.
    Interrupted,
    /// The turn completed successfully.
    Completed,
    /// The turn failed.
    Failed,
    /// The turn was cancelled.
    Cancelled,
}

impl ConversationTurnStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputReceived => "input_received",
            Self::Running => "running",
            Self::WaitingForTool => "waiting_for_tool",
            Self::Interrupted => "interrupted",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse a stored turn status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "input_received" => Ok(Self::InputReceived),
            "running" => Ok(Self::Running),
            "waiting_for_tool" => Ok(Self::WaitingForTool),
            "interrupted" => Ok(Self::Interrupted),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => invalid_enum("conversation_turn_status", value),
        }
    }
}

/// Semantic kind for a durable conversation stream item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationItemKind {
    /// Text authored by a human.
    UserText,
    /// Text authored by an assistant.
    AssistantText,
    /// Non-text activity that should appear in the transcript.
    Activity,
    /// Structured A2UI card payload.
    A2uiCard,
    /// A tool invocation request.
    ToolCall,
    /// A tool invocation result.
    ToolResult,
    /// A request for human approval.
    ApprovalRequest,
    /// A recorded approval decision.
    ApprovalResult,
    /// An error visible in conversation history.
    ErrorNotice,
}

impl ConversationItemKind {
    /// Return the stable storage string for this item kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserText => "user_text",
            Self::AssistantText => "assistant_text",
            Self::Activity => "activity",
            Self::A2uiCard => "a2ui_card",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::ApprovalRequest => "approval_request",
            Self::ApprovalResult => "approval_result",
            Self::ErrorNotice => "error_notice",
        }
    }

    /// Parse a stored item kind string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "user_text" => Ok(Self::UserText),
            "assistant_text" => Ok(Self::AssistantText),
            "activity" => Ok(Self::Activity),
            "a2ui_card" => Ok(Self::A2uiCard),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "approval_request" => Ok(Self::ApprovalRequest),
            "approval_result" => Ok(Self::ApprovalResult),
            "error_notice" => Ok(Self::ErrorNotice),
            _ => invalid_enum("conversation_item_kind", value),
        }
    }
}

/// Execution status for a durable conversation item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationItemStatus {
    /// The item has been created but work has not started.
    Pending,
    /// The item represents work in progress.
    Running,
    /// The item completed successfully.
    Completed,
    /// The item failed.
    Failed,
    /// The item was cancelled.
    Cancelled,
    /// The item was interrupted.
    Interrupted,
}

impl ConversationItemStatus {
    /// Return the stable storage string for this item status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    /// Parse a stored item status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => invalid_enum("conversation_item_status", value),
        }
    }
}

/// Input for creating a durable conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct NewConversation {
    /// Human-visible conversation title, when known.
    pub title: Option<String>,
    /// Concrete object that owns the conversation.
    pub owner: ObjectRef,
    /// Primary human participant id.
    pub primary_human_id: Option<String>,
    /// Primary agent participant id.
    pub primary_agent_id: Option<String>,
    /// Provider name used for the conversation.
    pub provider: String,
    /// Provider model name, when known.
    pub model: Option<String>,
    /// Working directory associated with the conversation.
    pub cwd: Option<String>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewConversation {
    /// Build the default local Codex chat conversation shape.
    #[must_use]
    pub fn local_chat(model: Option<String>, cwd: Option<String>) -> Self {
        Self {
            title: None,
            owner: ObjectRef::human("human:local"),
            primary_human_id: Some("human:local".to_string()),
            primary_agent_id: Some("agent:primary".to_string()),
            provider: "codex".to_string(),
            model,
            cwd,
            metadata: json!({}),
        }
    }
}

/// Persisted conversation identity returned after creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationRecord {
    /// Durable Noema conversation id.
    pub conversation_id: String,
}

/// Input for creating one causal conversation turn.
#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationTurn {
    /// Conversation that owns the turn.
    pub conversation_id: String,
    /// Optional item that triggered this turn.
    pub trigger_item_id: Option<String>,
    /// Additional structured metadata.
    pub metadata: Value,
}

/// Persisted conversation turn identity returned after creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTurnRecord {
    /// Durable Noema turn id.
    pub turn_id: String,
    /// Conversation that owns the turn.
    pub conversation_id: String,
}

/// Input for appending a durable item to a conversation stream.
#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationItem {
    /// Conversation that owns the item.
    pub conversation_id: String,
    /// Optional turn that owns the item.
    pub turn_id: Option<String>,
    /// Optional parent conversation item.
    pub parent_item_id: Option<String>,
    /// Semantic item kind.
    pub kind: ConversationItemKind,
    /// Item execution status.
    pub status: ConversationItemStatus,
    /// Concrete object that authored the item.
    pub author: ObjectRef,
    /// Readable item text, when any.
    pub content_text: Option<String>,
    /// Structured item payload.
    pub payload_json: Value,
    /// Additional structured metadata.
    pub metadata: Value,
}

/// Conversation item returned from append and replay operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationItemRecord {
    /// Durable Noema item id.
    pub item_id: String,
    /// Conversation that owns the item.
    pub conversation_id: String,
    /// Optional turn that owns the item.
    pub turn_id: Option<String>,
    /// Semantic item kind.
    pub kind: ConversationItemKind,
    /// Item execution status.
    pub status: ConversationItemStatus,
    /// Readable item text, when any.
    pub content_text: Option<String>,
    /// Structured item payload.
    pub payload_json: Value,
}

/// Replay visibility mode for conversation items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Exclude soft-deleted items.
    Visible,
    /// Include soft-deleted items for audit views.
    Audit,
}

impl PostgresMemoryRepository {
    /// Create a durable conversation row.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when referenced owner/participant
    /// rows are missing or Postgres writes fail.
    pub async fn create_conversation(
        &self,
        conversation: NewConversation,
    ) -> Result<ConversationRecord, MemoryPersistenceError> {
        let NewConversation {
            title,
            owner,
            primary_human_id,
            primary_agent_id,
            provider,
            model,
            cwd,
            metadata,
        } = conversation;

        validate_object_ref_for_pool(self.pool(), &owner).await?;
        validate_optional_object_ref_for_pool(
            self.pool(),
            ObjectType::Human,
            primary_human_id.as_deref(),
        )
        .await?;
        validate_optional_object_ref_for_pool(
            self.pool(),
            ObjectType::Agent,
            primary_agent_id.as_deref(),
        )
        .await?;

        let conversation_id = allocate_postgres_id(self.pool(), "conversation").await?;
        let record = sqlx::query_as::<_, (String,)>(
            r"
            INSERT INTO conversations (
              conversation_id, title, owner_object_type, owner_object_id,
              primary_human_id, primary_agent_id, provider, model, cwd, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING conversation_id
            ",
        )
        .bind(conversation_id)
        .bind(title.as_deref())
        .bind(owner.object_type.as_str())
        .bind(owner.object_id.as_str())
        .bind(primary_human_id.as_deref())
        .bind(primary_agent_id.as_deref())
        .bind(provider.as_str())
        .bind(model.as_deref())
        .bind(cwd.as_deref())
        .bind(json_value(metadata))
        .fetch_one(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        Ok(ConversationRecord {
            conversation_id: record.0,
        })
    }

    /// Return a human's active primary conversation, creating one when needed.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the human row is missing or
    /// Postgres reads/writes fail.
    pub async fn get_or_create_primary_conversation(
        &self,
        human_id: &str,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<ConversationRecord, MemoryPersistenceError> {
        validate_object_ref_for_pool(self.pool(), &ObjectRef::human(human_id)).await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;

        let primary_conversation_id = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT primary_conversation_id
            FROM humans
            WHERE human_id = $1
            FOR UPDATE
            "#,
        )
        .bind(human_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        if let Some(conversation_id) = primary_conversation_id {
            let active = sqlx::query_scalar::<_, i32>(
                r#"
                SELECT 1
                FROM conversations
                WHERE conversation_id = $1
                  AND owner_object_type = 'human'
                  AND owner_object_id = $2
                  AND primary_human_id = $2
                  AND lifecycle_status = 'active'
                  AND deleted_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(conversation_id.as_str())
            .bind(human_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?
            .is_some();

            if active {
                tx.commit()
                    .await
                    .map_err(MemoryPersistenceError::Database)?;
                return Ok(ConversationRecord { conversation_id });
            }
        }

        let conversation_id = allocate_postgres_id(&mut *tx, "conversation").await?;
        sqlx::query(
            r#"
            INSERT INTO conversations (
              conversation_id, title, owner_object_type, owner_object_id,
              primary_human_id, primary_agent_id, provider, model, cwd, metadata
            )
            VALUES ($1, $2, 'human', $3, $3, 'agent:primary', 'codex', $4, $5, '{}'::jsonb)
            "#,
        )
        .bind(conversation_id.as_str())
        .bind("Home")
        .bind(human_id)
        .bind(model.as_deref())
        .bind(cwd.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        sqlx::query(
            r#"
            UPDATE humans
            SET primary_conversation_id = $2,
                updated_at = now()
            WHERE human_id = $1
            "#,
        )
        .bind(human_id)
        .bind(conversation_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        tx.commit()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        Ok(ConversationRecord { conversation_id })
    }

    /// Return the next durable turn index for a conversation.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the conversation is missing or
    /// Postgres reads fail.
    pub async fn next_conversation_turn_index(
        &self,
        conversation_id: &str,
    ) -> Result<u64, MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, conversation_id)?,
        )
        .await?;

        let next = sqlx::query_scalar::<_, Option<i64>>(
            r#"
            SELECT COALESCE(MAX((metadata->>'turn_index')::bigint), 0) + 1
            FROM conversation_turns
            WHERE conversation_id = $1
              AND metadata ? 'turn_index'
            "#,
        )
        .bind(conversation_id)
        .fetch_one(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?
        .unwrap_or(1);

        Ok(u64::try_from(next).unwrap_or(1))
    }

    /// Return recent user and assistant text items for provider context.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the conversation is missing or
    /// Postgres reads fail.
    pub async fn list_recent_conversation_items_for_context(
        &self,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<ConversationItemRecord>, MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, conversation_id)?,
        )
        .await?;
        let limit = limit.clamp(1, 40);
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                Value,
            ),
        >(
            r#"
            SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json
            FROM (
              SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, created_at
              FROM conversation_items
              WHERE conversation_id = $1
                AND deleted_at IS NULL
                AND kind IN ('user_text', 'assistant_text')
              ORDER BY created_at DESC
              LIMIT $2
            ) recent
            ORDER BY created_at ASC
            "#,
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(
                |(item_id, conversation_id, turn_id, kind, status, content_text, payload_json)| {
                    Ok(ConversationItemRecord {
                        item_id,
                        conversation_id,
                        turn_id,
                        kind: ConversationItemKind::parse(&kind)?,
                        status: ConversationItemStatus::parse(&status)?,
                        content_text,
                        payload_json,
                    })
                },
            )
            .collect()
    }

    /// Create a durable turn row for an existing conversation.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the conversation or trigger
    /// item is missing, or Postgres writes fail.
    pub async fn create_conversation_turn(
        &self,
        turn: NewConversationTurn,
    ) -> Result<ConversationTurnRecord, MemoryPersistenceError> {
        let NewConversationTurn {
            conversation_id,
            trigger_item_id,
            metadata,
        } = turn;

        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, conversation_id.as_str())?,
        )
        .await?;
        if let Some(trigger_item_id) = &trigger_item_id {
            self.validate_conversation_item_in_conversation(trigger_item_id, &conversation_id)
                .await?;
        }

        let turn_id = allocate_postgres_id(self.pool(), "turn").await?;
        let record = sqlx::query_as::<_, (String, String)>(
            r"
            INSERT INTO conversation_turns (
              turn_id, conversation_id, trigger_item_id, status, started_at, metadata
            )
            VALUES ($1, $2, $3, 'input_received', now(), $4)
            RETURNING turn_id, conversation_id
            ",
        )
        .bind(turn_id)
        .bind(conversation_id.as_str())
        .bind(trigger_item_id.as_deref())
        .bind(json_value(metadata))
        .fetch_one(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        Ok(ConversationTurnRecord {
            turn_id: record.0,
            conversation_id: record.1,
        })
    }

    /// Update the live agent status for a durable conversation.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the conversation is missing or
    /// Postgres writes fail.
    pub async fn update_conversation_agent_status(
        &self,
        conversation_id: &str,
        status: AgentStatus,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, conversation_id)?,
        )
        .await?;
        sqlx::query(
            r"
            UPDATE conversations
            SET agent_status = $2,
                updated_at = now()
            WHERE conversation_id = $1
            ",
        )
        .bind(conversation_id)
        .bind(status.as_str())
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(())
    }

    /// Append a durable item to a conversation stream.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when referenced conversation, author,
    /// turn, or parent item rows are invalid, or Postgres writes fail.
    pub async fn append_conversation_item(
        &self,
        item: NewConversationItem,
    ) -> Result<ConversationItemRecord, MemoryPersistenceError> {
        self.validate_conversation_item_refs(&item).await?;

        let item_id = allocate_postgres_id(self.pool(), "item").await?;
        let record = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                Value,
            ),
        >(
            r"
            INSERT INTO conversation_items (
              item_id, conversation_id, turn_id, parent_item_id, kind, status,
              author_object_type, author_object_id, content_text, payload_json, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING item_id, conversation_id, turn_id, kind, status, content_text, payload_json
            ",
        )
        .bind(item_id)
        .bind(item.conversation_id.as_str())
        .bind(item.turn_id.as_deref())
        .bind(item.parent_item_id.as_deref())
        .bind(item.kind.as_str())
        .bind(item.status.as_str())
        .bind(item.author.object_type.as_str())
        .bind(item.author.object_id.as_str())
        .bind(item.content_text.as_deref())
        .bind(json_value(item.payload_json))
        .bind(json_value(item.metadata))
        .fetch_one(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        Ok(ConversationItemRecord {
            item_id: record.0,
            conversation_id: record.1,
            turn_id: record.2,
            kind: ConversationItemKind::parse(&record.3)?,
            status: ConversationItemStatus::parse(&record.4)?,
            content_text: record.5,
            payload_json: record.6,
        })
    }

    /// Mark a durable conversation turn completed.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the turn is missing or Postgres
    /// writes fail.
    pub async fn complete_conversation_turn(
        &self,
        turn_id: &str,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::ConversationTurn, turn_id)?,
        )
        .await?;
        sqlx::query(
            r"
            UPDATE conversation_turns
            SET status = 'completed',
                completed_at = now(),
                updated_at = now()
            WHERE turn_id = $1
            ",
        )
        .bind(turn_id)
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(())
    }

    /// Mark a durable conversation turn failed.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the turn is missing or Postgres
    /// writes fail.
    pub async fn fail_conversation_turn(
        &self,
        turn_id: &str,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::ConversationTurn, turn_id)?,
        )
        .await?;
        sqlx::query(
            r"
            UPDATE conversation_turns
            SET status = 'failed',
                completed_at = now(),
                updated_at = now()
            WHERE turn_id = $1
            ",
        )
        .bind(turn_id)
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(())
    }

    /// List conversation items in replay order.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the conversation is missing,
    /// Postgres reads fail, or stored enums are invalid.
    pub async fn list_conversation_items(
        &self,
        conversation_id: &str,
        mode: ReplayMode,
    ) -> Result<Vec<ConversationItemRecord>, MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, conversation_id)?,
        )
        .await?;
        let sql = match mode {
            ReplayMode::Visible => {
                r"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json
                FROM conversation_items
                WHERE conversation_id = $1 AND deleted_at IS NULL
                ORDER BY created_at ASC
                "
            }
            ReplayMode::Audit => {
                r"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json
                FROM conversation_items
                WHERE conversation_id = $1
                ORDER BY created_at ASC
                "
            }
        };
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                Value,
            ),
        >(sql)
        .bind(conversation_id)
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(
                |(item_id, conversation_id, turn_id, kind, status, content_text, payload_json)| {
                    Ok(ConversationItemRecord {
                        item_id,
                        conversation_id,
                        turn_id,
                        kind: ConversationItemKind::parse(&kind)?,
                        status: ConversationItemStatus::parse(&status)?,
                        content_text,
                        payload_json,
                    })
                },
            )
            .collect()
    }

    async fn validate_conversation_item_refs(
        &self,
        item: &NewConversationItem,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::new(ObjectType::Conversation, item.conversation_id.as_str())?,
        )
        .await?;
        validate_object_ref_for_pool(self.pool(), &item.author).await?;

        if let Some(turn_id) = &item.turn_id {
            let turn_conversation_id = sqlx::query_scalar::<_, String>(
                "SELECT conversation_id FROM conversation_turns WHERE turn_id = $1",
            )
            .bind(turn_id)
            .fetch_optional(self.pool())
            .await
            .map_err(MemoryPersistenceError::Database)?;
            match turn_conversation_id {
                Some(value) if value == item.conversation_id => {}
                Some(_) | None => {
                    return Err(MemoryPersistenceError::TurnConversationMismatch {
                        turn_id: turn_id.clone(),
                        conversation_id: item.conversation_id.clone(),
                    });
                }
            }
        }

        if let Some(parent_item_id) = &item.parent_item_id {
            self.validate_conversation_item_in_conversation(parent_item_id, &item.conversation_id)
                .await?;
        }

        Ok(())
    }

    async fn validate_conversation_item_in_conversation(
        &self,
        item_id: &str,
        conversation_id: &str,
    ) -> Result<(), MemoryPersistenceError> {
        let exists = sqlx::query_scalar::<_, i32>(
            r"
            SELECT 1
            FROM conversation_items
            WHERE item_id = $1 AND conversation_id = $2
            LIMIT 1
            ",
        )
        .bind(item_id)
        .bind(conversation_id)
        .fetch_optional(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?
        .is_some();
        if exists {
            Ok(())
        } else {
            Err(MemoryPersistenceError::ObjectRefNotFound {
                object_type: ObjectType::ConversationItem.as_str().to_string(),
                object_id: item_id.to_string(),
            })
        }
    }
}

async fn validate_optional_object_ref_for_pool(
    pool: &sqlx::PgPool,
    object_type: ObjectType,
    object_id: Option<&str>,
) -> Result<(), MemoryPersistenceError> {
    if let Some(object_id) = object_id {
        validate_object_ref_for_pool(pool, &ObjectRef::new(object_type, object_id)?).await
    } else {
        Ok(())
    }
}

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, MemoryPersistenceError> {
    Err(MemoryPersistenceError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}
