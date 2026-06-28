use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::Value;
use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
};
use tokio::sync::Mutex;

use crate::{
    ConversationItemKind, ConversationItemRecord, ConversationItemStatus, ConversationRecord,
    ConversationTurnRecord, NewConversation, NewConversationItem, NewConversationTurn,
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod, ReplayMode,
    memory_persistence::AgentStatus,
};

use super::{
    error::StoreError,
    schema::{NOEMA_DATABASE, NOEMA_NAMESPACE, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Configuration for the embedded Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Directory used by the embedded database.
    pub path: PathBuf,
}

impl StoreConfig {
    /// Build store config from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.db_dir(),
        }
    }
}

/// Server-owned embedded canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    db: Surreal<Db>,
    append_item_lock: Arc<Mutex<()>>,
}

impl NoemaStore {
    /// Open and bootstrap the embedded store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database directory cannot be prepared or
    /// SurrealDB cannot be opened or bootstrapped.
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        fs::create_dir_all(&config.path).map_err(StoreError::PreparePath)?;
        let db = Surreal::new::<RocksDb>(config.path.as_path()).await?;
        db.use_ns(NOEMA_NAMESPACE).use_db(NOEMA_DATABASE).await?;
        debug_assert_eq!(STORE_SCHEMA_VERSION, 1);
        db.query(STORE_SCHEMA_SQL).await?.check()?;
        Ok(Self {
            db,
            append_item_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Access the embedded SurrealDB client for repository modules.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// Return the current schema marker version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the schema marker cannot be queried.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        #[derive(serde::Deserialize)]
        struct Row {
            version: i64,
        }

        let row: Option<Row> = self.db.select(("schema_state", "current")).await?;
        row.map(|row| row.version)
            .ok_or_else(|| StoreError::Schema("missing schema_state:current".to_string()))
    }

    /// Close the embedded store handle.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SurrealDB fails to invalidate the local
    /// connection.
    pub async fn close(self) -> Result<(), StoreError> {
        self.db.invalidate().await?;
        Ok(())
    }

    /// Create or refresh the built-in local human and primary agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPSERT type::thing('humans', 'human_local') SET
                  human_id = 'human:local',
                  display_name = 'Local Human',
                  updated_at = time::now();
                UPSERT type::thing('agents', 'agent_primary') SET
                  agent_id = 'agent:primary',
                  display_name = 'Noema',
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        Ok(())
    }

    /// Create or return the default Codex provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        if let Some(account) = self
            .get_provider_account("provider_account:codex:default")
            .await?
        {
            return Ok(account);
        }

        self.db
            .query(
                r#"
                UPSERT type::thing('provider_accounts', 'codex_default') SET
                  provider_account_id = 'provider_account:codex:default',
                  provider_kind = 'codex',
                  account_key = 'default',
                  display_name = 'Codex',
                  auth_method = 'oauth_device_code',
                  is_active = true,
                  is_default = true,
                  status = 'unknown',
                  metadata = {},
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        self.get_provider_account("provider_account:codex:default")
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: "provider_account:codex:default".to_string(),
            })
    }

    /// Return the active default account for one provider.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_provider_account(
        &self,
        provider_kind: &str,
    ) -> Result<Option<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE provider_kind = $provider_kind
                  AND is_active = true
                  AND is_default = true
                LIMIT 1;
                "#,
            )
            .bind(("provider_kind", provider_kind.to_string()))
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(provider_account_from_row)
            .transpose()
    }

    /// Return one provider account by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<Option<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE provider_account_id = $provider_account_id
                LIMIT 1;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(provider_account_from_row)
            .transpose()
    }

    /// Update safe provider account status metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account is missing or the embedded store
    /// write fails.
    pub async fn update_provider_account_status(
        &self,
        provider_account_id: &str,
        status: ProviderAccountStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), StoreError> {
        let Some(account) = self.get_provider_account(provider_account_id).await? else {
            return Err(StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            });
        };
        let checked_at = now_string();
        let last_authenticated_at = if status == ProviderAccountStatus::Authenticated {
            Some(checked_at.clone())
        } else {
            account.last_authenticated_at
        };
        self.db
            .query(
                r#"
                UPDATE provider_accounts SET
                  status = $status,
                  last_checked_at = $last_checked_at,
                  last_authenticated_at = $last_authenticated_at,
                  last_error_code = $error_code,
                  last_error_message = $error_message,
                  updated_at = time::now()
                WHERE provider_account_id = $provider_account_id;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .bind(("status", provider_status_str(status).to_string()))
            .bind(("last_checked_at", Some(checked_at)))
            .bind(("last_authenticated_at", last_authenticated_at))
            .bind(("error_code", error_code.map(ToString::to_string)))
            .bind(("error_message", error_message.map(ToString::to_string)))
            .await?
            .check()?;
        Ok(())
    }

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
            && self.conversation_exists(&conversation_id).await?
        {
            return Ok(ConversationRecord { conversation_id });
        }

        let record = self
            .create_conversation_with_id(
                allocate_id("conversation"),
                NewConversation::local_chat(model, cwd),
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
        let turn_id = allocate_id("turn");
        self.db
            .query(
                r#"
                CREATE type::thing('conversation_turns', $record_id) SET
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
                CREATE type::thing('conversation_items', $record_id) SET
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
                CREATE type::thing('conversations', $record_id) SET
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

    async fn require_conversation(&self, conversation_id: &str) -> Result<(), StoreError> {
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

#[derive(Debug, Deserialize)]
struct ProviderAccountRow {
    provider_account_id: String,
    provider_kind: String,
    account_key: String,
    display_name: String,
    auth_method: String,
    is_active: bool,
    is_default: bool,
    status: String,
    last_checked_at: Option<String>,
    last_authenticated_at: Option<String>,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct PrimaryConversationRow {
    primary_conversation_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConversationIdRow {
    #[allow(dead_code)]
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
struct TurnIdRow {
    #[allow(dead_code)]
    turn_id: String,
}

#[derive(Debug, Deserialize)]
struct TurnRefRow {
    #[allow(dead_code)]
    turn_id: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
struct ConversationItemRefRow {
    #[allow(dead_code)]
    item_id: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
struct SequenceRow {
    sequence_index: i64,
}

#[derive(Debug, Deserialize)]
struct MetadataRow {
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct ConversationItemRow {
    item_id: String,
    conversation_id: String,
    turn_id: Option<String>,
    kind: String,
    status: String,
    content_text: Option<String>,
    payload_json: Value,
}

fn provider_account_from_row(row: ProviderAccountRow) -> Result<ProviderAccountRecord, StoreError> {
    Ok(ProviderAccountRecord {
        provider_account_id: row.provider_account_id,
        provider_kind: row.provider_kind,
        account_key: row.account_key,
        display_name: row.display_name,
        auth_method: parse_provider_auth_method(&row.auth_method)?,
        is_active: row.is_active,
        is_default: row.is_default,
        status: parse_provider_account_status(&row.status)?,
        last_checked_at: row.last_checked_at,
        last_authenticated_at: row.last_authenticated_at,
        last_error_code: row.last_error_code,
        last_error_message: row.last_error_message,
        metadata: row.metadata,
    })
}

fn conversation_item_from_row(
    row: ConversationItemRow,
) -> Result<ConversationItemRecord, StoreError> {
    Ok(ConversationItemRecord {
        item_id: row.item_id,
        conversation_id: row.conversation_id,
        turn_id: row.turn_id,
        kind: ConversationItemKind::parse(&row.kind).map_err(memory_enum_error)?,
        status: ConversationItemStatus::parse(&row.status).map_err(memory_enum_error)?,
        content_text: row.content_text,
        payload_json: row.payload_json,
    })
}

fn parse_provider_auth_method(value: &str) -> Result<ProviderAuthMethod, StoreError> {
    match value {
        "oauth_device_code" => Ok(ProviderAuthMethod::OauthDeviceCode),
        "secret_input" => Ok(ProviderAuthMethod::SecretInput),
        "external_manual" => Ok(ProviderAuthMethod::ExternalManual),
        "none" => Ok(ProviderAuthMethod::None),
        _ => invalid_enum("provider_auth_method", value),
    }
}

fn parse_provider_account_status(value: &str) -> Result<ProviderAccountStatus, StoreError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        _ => invalid_enum("provider_account_status", value),
    }
}

const fn provider_status_str(status: ProviderAccountStatus) -> &'static str {
    match status {
        ProviderAccountStatus::Unknown => "unknown",
        ProviderAccountStatus::Checking => "checking",
        ProviderAccountStatus::Authenticated => "authenticated",
        ProviderAccountStatus::Unauthenticated => "unauthenticated",
        ProviderAccountStatus::Unavailable => "unavailable",
    }
}

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
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

fn allocate_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}:{nanos:x}{counter:x}")
}

fn record_fragment(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}
