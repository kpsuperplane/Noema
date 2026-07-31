//! Durable provider-native human interaction authority.

use std::convert::TryFrom;

use noema_conversations::{
    ConversationItemKind, ConversationItemRecord, ConversationItemStatus, NewConversationItem,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::{
    NoemaStore, StoreError,
    conversations::load_conversation_item_tx,
    ids::allocate_id,
    sqlite::{deserialize_json, serialize_json},
};

const MAX_RESUME_LEASE_SECONDS: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms, missing_docs)]
pub enum ConversationInteractionKind {
    MultipleChoice,
    A2UI,
}

impl ConversationInteractionKind {
    /// Return the stable persisted kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MultipleChoice => "multiple_choice",
            Self::A2UI => "a2ui",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "multiple_choice" => Ok(Self::MultipleChoice),
            "a2ui" => Ok(Self::A2UI),
            other => crate::ids::invalid_enum("conversation_interaction_kind", other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ConversationInteractionStatus {
    Pending,
    Answered,
    Resuming,
    Completed,
    Failed,
}

impl ConversationInteractionStatus {
    /// Return the stable persisted lifecycle.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Answered => "answered",
            Self::Resuming => "resuming",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "pending" => Ok(Self::Pending),
            "answered" => Ok(Self::Answered),
            "resuming" => Ok(Self::Resuming),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => crate::ids::invalid_enum("conversation_interaction_status", other),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs, reason = "fields mirror the canonical interaction row")]
pub struct NewConversationInteraction {
    pub interaction_id: String,
    pub conversation_id: String,
    pub originating_turn_id: String,
    pub kind: ConversationInteractionKind,
    pub provider_call_id: String,
    pub canonical_tool_name: String,
    pub provider_tool_name: String,
    pub provider_kind: String,
    pub provider_account_id: String,
    pub provider_instance_key: String,
    pub selection_mode: String,
    pub credential_revision: u64,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub tool_catalog_digest: String,
    pub request: Value,
    pub projection: Value,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs, reason = "fields mirror the canonical interaction row")]
pub struct ConversationInteractionRecord {
    pub interaction_id: String,
    pub conversation_id: String,
    pub originating_turn_id: String,
    pub kind: ConversationInteractionKind,
    pub provider_call_id: String,
    pub canonical_tool_name: String,
    pub provider_tool_name: String,
    pub provider_kind: String,
    pub provider_account_id: String,
    pub provider_instance_key: String,
    pub selection_mode: String,
    pub credential_revision: u64,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub tool_catalog_digest: String,
    pub request: Value,
    pub projection: Value,
    pub provider_call_item_id: String,
    pub projection_item_id: String,
    pub revision: u64,
    pub status: ConversationInteractionStatus,
    pub resolution_item_id: Option<String>,
    pub tool_result_item_id: Option<String>,
    pub resolution: Option<Value>,
    pub client_message_id: Option<String>,
    pub resume_claim_owner: Option<String>,
    pub resume_claim_token: Option<String>,
    pub resume_claim_expires_at: Option<String>,
    pub terminal_error: Option<String>,
    pub resolved_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(missing_docs)]
impl NoemaStore {
    /// Publish the provider call, projection, interaction, and waiting turn atomically.
    ///
    /// # Errors
    /// Returns an error when validation, persistence, or the originating-turn fence fails.
    pub async fn publish_conversation_interaction(
        &self,
        interaction: NewConversationInteraction,
        provider_tool_call: NewConversationItem,
        projection: NewConversationItem,
    ) -> Result<ConversationInteractionRecord, StoreError> {
        validate_new(&interaction)?;
        validate_publication_items(&interaction, &provider_tool_call, &projection)?;
        let request_json = serialize_json(&interaction.request)?;
        let projection_json = serialize_json(&interaction.projection)?;
        let provider_call_item_id = allocate_id("item");
        let projection_item_id = allocate_id("item");
        let _append_guard = self.append_item_lock.lock().await;
        self.with_immediate_transaction_retry(|tx| {
            let provider_item = append_item_tx(tx, provider_call_item_id.clone(), provider_tool_call.clone())?;
            let projection_item = append_item_tx(tx, projection_item_id.clone(), projection.clone())?;
            tx.execute(
                "INSERT INTO conversation_interactions (interaction_id, conversation_id, originating_turn_id, kind, provider_call_id, canonical_tool_name, provider_tool_name, provider_kind, provider_account_id, provider_instance_key, selection_mode, credential_revision, model, reasoning_effort, tool_catalog_digest, request_json, projection_json, provider_call_item_id, projection_item_id, lifecycle_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'pending')",
                params![
                    interaction.interaction_id,
                    interaction.conversation_id,
                    interaction.originating_turn_id,
                    interaction.kind.as_str(),
                    interaction.provider_call_id,
                    interaction.canonical_tool_name,
                    interaction.provider_tool_name,
                    interaction.provider_kind,
                    interaction.provider_account_id,
                    interaction.provider_instance_key,
                    interaction.selection_mode,
                    i64::try_from(interaction.credential_revision).map_err(|_| conflict("credential revision exceeds SQLite integer range"))?,
                    interaction.model,
                    interaction.reasoning_effort,
                    interaction.tool_catalog_digest,
                    request_json,
                    projection_json,
                    provider_item.item_id,
                    projection_item.item_id,
                ],
            )?;
            if tx.execute(
                "UPDATE conversation_turns SET status = 'waiting_for_tool', completed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE turn_id = ?1 AND conversation_id = ?2 AND status IN ('input_received', 'running', 'waiting_for_tool')",
                params![interaction.originating_turn_id, interaction.conversation_id],
            )? != 1 {
                return Err(conflict("interaction publication lost its originating turn"));
            }
            interaction_by_id_tx(tx, &interaction.interaction_id)?.ok_or_else(|| conflict("interaction publication disappeared before commit"))
        })
        .await
    }

    /// Fetch one interaction by its stable identifier.
    ///
    /// # Errors
    /// Returns an error when the stored row cannot be read or decoded.
    pub async fn get_conversation_interaction(
        &self,
        interaction_id: &str,
    ) -> Result<Option<ConversationInteractionRecord>, StoreError> {
        self.with_connection(|connection| {
            let sql = format!("{INTERACTION_FIELDS} WHERE interaction_id = ?1");
            let mut statement = connection.prepare(&sql)?;
            statement
                .query_row([interaction_id], interaction_from_row)
                .optional()
                .map_err(StoreError::Sqlite)?
                .map(parse_row)
                .transpose()
        })
        .await
    }

    /// CAS-resolve one pending revision and append its human action/result atomically.
    ///
    /// # Errors
    /// Returns an error when validation, persistence, or the revision fence fails.
    pub async fn resolve_conversation_interaction(
        &self,
        interaction_id: &str,
        expected_revision: u64,
        client_message_id: &str,
        human_action: NewConversationItem,
        provider_tool_result: NewConversationItem,
    ) -> Result<ConversationInteractionRecord, StoreError> {
        if client_message_id.trim().is_empty() {
            return Err(conflict("interaction client message id cannot be empty"));
        }
        if provider_tool_result.kind != ConversationItemKind::ToolResult {
            return Err(conflict(
                "interaction continuation item must be a tool result",
            ));
        }
        if !matches!(
            provider_tool_result.status,
            ConversationItemStatus::Completed
                | ConversationItemStatus::Failed
                | ConversationItemStatus::Cancelled
                | ConversationItemStatus::Interrupted
        ) {
            return Err(conflict(
                "interaction continuation item must have a terminal status",
            ));
        }
        let provider_call_status = provider_tool_result.status.as_str();
        let human_action_item_id = allocate_id("item");
        let tool_result_item_id = allocate_id("item");
        let _append_guard = self.append_item_lock.lock().await;
        self.with_immediate_transaction_retry(|tx| {
            let interaction = interaction_by_id_tx(tx, interaction_id)?
                .ok_or_else(|| conflict("conversation interaction was not found"))?;
            if interaction.status != ConversationInteractionStatus::Pending
                || interaction.revision != expected_revision
            {
                return Err(conflict("conversation interaction is already resolved"));
            }
            validate_resolution_items(&interaction, &human_action, &provider_tool_result)?;
            let action = append_item_tx(tx, human_action_item_id.clone(), human_action.clone())?;
            let result = append_item_tx(tx, tool_result_item_id.clone(), provider_tool_result.clone())?;
            if tx.execute(
                "UPDATE conversation_items SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE item_id = ?1 AND conversation_id = ?3 AND kind = 'tool_call' AND status IN ('pending', 'running')",
                params![interaction.provider_call_item_id, provider_call_status, interaction.conversation_id],
            )? != 1 {
                return Err(conflict("interaction resolution lost its provider call"));
            }
            let resolution_json = serialize_json(&human_action.payload_json)?;
            if tx.execute(
                "UPDATE conversation_interactions SET revision = revision + 1, lifecycle_status = 'answered', resolution_item_id = ?2, tool_result_item_id = ?3, resolution_json = ?4, client_message_id = ?5, resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE interaction_id = ?1 AND revision = ?6 AND lifecycle_status = 'pending'",
                params![interaction_id, action.item_id, result.item_id, resolution_json, client_message_id, i64::try_from(expected_revision).map_err(|_| conflict("interaction revision exceeds SQLite integer range"))?],
            )? != 1 {
                return Err(conflict("conversation interaction resolution lost its CAS"));
            }
            interaction_by_id_tx(tx, interaction_id)?.ok_or_else(|| conflict("interaction resolution disappeared before commit"))
        })
        .await
    }

    /// Claim an answered interaction for one leased resumption attempt.
    ///
    /// # Errors
    /// Returns an error when the lease is invalid, stale, or cannot be persisted.
    pub async fn claim_conversation_interaction_resumption(
        &self,
        interaction_id: &str,
        expected_revision: u64,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<ConversationInteractionRecord, StoreError> {
        if worker_id.trim().is_empty() || !(1..=MAX_RESUME_LEASE_SECONDS).contains(&lease_seconds) {
            return Err(conflict("interaction resume lease request is invalid"));
        }
        let claim_token = allocate_id("interaction_resume");
        self.with_immediate_transaction_retry(|tx| {
            reclaim_expired_tx(tx)?;
            if tx.execute(
                "UPDATE conversation_interactions SET lifecycle_status = 'resuming', resume_claim_owner = ?3, resume_claim_token = ?4, resume_claim_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?5 || ' seconds'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE interaction_id = ?1 AND revision = ?2 AND lifecycle_status = 'answered'",
                params![interaction_id, i64::try_from(expected_revision).unwrap_or(i64::MAX), worker_id, claim_token, lease_seconds],
            )? != 1 {
                return Err(conflict("conversation interaction resumption is stale"));
            }
            interaction_by_id_tx(tx, interaction_id)?.ok_or_else(|| conflict("interaction resumption claim disappeared before commit"))
        })
        .await
    }

    /// Reclaim expired leases and return answered interactions in deterministic order.
    ///
    /// # Errors
    /// Returns an error when expired leases or resumable rows cannot be persisted or read.
    pub async fn list_resumable_conversation_interactions(
        &self,
    ) -> Result<Vec<ConversationInteractionRecord>, StoreError> {
        self.with_immediate_transaction_retry(|tx| {
            reclaim_expired_tx(tx)?;
            let mut statement = tx.prepare(&format!("{INTERACTION_FIELDS} WHERE lifecycle_status = 'answered' ORDER BY created_at, interaction_id"))?;
            statement
                .query_map([], interaction_from_row)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(parse_row)
                .collect()
        })
        .await
    }

    /// Renew one active resumption claim without changing its token.
    ///
    /// # Errors
    /// Returns an error when the lease is invalid, expired, stale, or cannot be persisted.
    pub async fn heartbeat_conversation_interaction_resumption(
        &self,
        interaction_id: &str,
        expected_revision: u64,
        claim_token: &str,
        lease_seconds: i64,
    ) -> Result<(), StoreError> {
        if claim_token.trim().is_empty() || !(1..=MAX_RESUME_LEASE_SECONDS).contains(&lease_seconds)
        {
            return Err(conflict("interaction resume heartbeat is invalid"));
        }
        self.with_immediate_transaction_retry(|tx| {
            if tx.execute(
                "UPDATE conversation_interactions SET resume_claim_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?4 || ' seconds'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE interaction_id = ?1 AND revision = ?2 AND resume_claim_token = ?3 AND lifecycle_status = 'resuming' AND resume_claim_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![interaction_id, i64::try_from(expected_revision).unwrap_or(i64::MAX), claim_token, lease_seconds],
            )? != 1 {
                return Err(conflict("conversation interaction resume heartbeat is stale"));
            }
            Ok(())
        })
        .await
    }

    /// Complete or fail the resumption guarded by its claim token.
    ///
    /// # Errors
    /// Returns an error when the result is invalid, stale, or cannot be persisted.
    pub async fn finish_conversation_interaction_resumption(
        &self,
        interaction_id: &str,
        expected_revision: u64,
        claim_token: &str,
        terminal_error: Option<&str>,
    ) -> Result<ConversationInteractionRecord, StoreError> {
        if claim_token.trim().is_empty()
            || terminal_error.is_some_and(|error| error.trim().is_empty())
        {
            return Err(conflict("interaction resume completion is invalid"));
        }
        self.with_immediate_transaction_retry(|tx| {
            let status = if terminal_error.is_some() { "failed" } else { "completed" };
            if tx.execute(
                "UPDATE conversation_interactions SET lifecycle_status = ?4, resume_claim_owner = NULL, resume_claim_token = NULL, resume_claim_expires_at = NULL, terminal_error = ?5, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE interaction_id = ?1 AND revision = ?2 AND resume_claim_token = ?3 AND lifecycle_status = 'resuming'",
                params![interaction_id, i64::try_from(expected_revision).unwrap_or(i64::MAX), claim_token, status, terminal_error],
            )? != 1 {
                return Err(conflict("conversation interaction resume claim is stale"));
            }
            interaction_by_id_tx(tx, interaction_id)?.ok_or_else(|| conflict("interaction completion disappeared before commit"))
        })
        .await
    }
}

fn validate_new(input: &NewConversationInteraction) -> Result<(), StoreError> {
    for (field, value) in [
        ("interaction_id", input.interaction_id.as_str()),
        ("conversation_id", input.conversation_id.as_str()),
        ("originating_turn_id", input.originating_turn_id.as_str()),
        ("provider_call_id", input.provider_call_id.as_str()),
        ("canonical_tool_name", input.canonical_tool_name.as_str()),
        ("provider_tool_name", input.provider_tool_name.as_str()),
        ("provider_kind", input.provider_kind.as_str()),
        ("provider_account_id", input.provider_account_id.as_str()),
        (
            "provider_instance_key",
            input.provider_instance_key.as_str(),
        ),
        ("selection_mode", input.selection_mode.as_str()),
        ("model", input.model.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(conflict(format!("interaction {field} cannot be empty")));
        }
    }
    i64::try_from(input.credential_revision)
        .map_err(|_| conflict("credential revision exceeds SQLite integer range"))?;
    Ok(())
}

fn validate_publication_items(
    input: &NewConversationInteraction,
    call: &NewConversationItem,
    projection: &NewConversationItem,
) -> Result<(), StoreError> {
    if [call, projection].iter().any(|item| {
        item.conversation_id != input.conversation_id
            || item.turn_id.as_deref() != Some(input.originating_turn_id.as_str())
    }) {
        return Err(conflict(
            "interaction publication items do not belong to the originating turn",
        ));
    }
    if call.kind != ConversationItemKind::ToolCall {
        return Err(conflict(
            "interaction publication requires a provider tool-call item",
        ));
    }
    let expected = match input.kind {
        ConversationInteractionKind::MultipleChoice => ConversationItemKind::MultipleChoicePrompt,
        ConversationInteractionKind::A2UI => ConversationItemKind::A2UICard,
    };
    if projection.kind != expected {
        return Err(conflict(
            "interaction projection kind does not match interaction kind",
        ));
    }
    Ok(())
}

fn validate_resolution_items(
    existing: &ConversationInteractionRecord,
    action: &NewConversationItem,
    result: &NewConversationItem,
) -> Result<(), StoreError> {
    if [action, result].iter().any(|item| {
        item.conversation_id != existing.conversation_id
            || item.turn_id.as_deref() != Some(existing.originating_turn_id.as_str())
    }) {
        return Err(conflict(
            "interaction resolution items do not belong to the originating turn",
        ));
    }
    Ok(())
}

fn reclaim_expired_tx(tx: &Transaction<'_>) -> Result<usize, StoreError> {
    tx.execute("UPDATE conversation_interactions SET lifecycle_status = 'completed', resume_claim_owner = NULL, resume_claim_token = NULL, resume_claim_expires_at = NULL, completed_at = COALESCE(completed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE lifecycle_status IN ('answered', 'resuming') AND EXISTS (SELECT 1 FROM conversation_turns WHERE turn_id = originating_turn_id AND status = 'completed')", [])?;
    tx.execute("UPDATE conversation_interactions SET lifecycle_status = 'answered', resume_claim_owner = NULL, resume_claim_token = NULL, resume_claim_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE lifecycle_status = 'resuming' AND resume_claim_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [])
        .map_err(StoreError::Sqlite)
}

const INTERACTION_FIELDS: &str = "SELECT interaction_id, conversation_id, originating_turn_id, kind, provider_call_id, canonical_tool_name, provider_tool_name, provider_kind, provider_account_id, provider_instance_key, selection_mode, credential_revision, model, reasoning_effort, tool_catalog_digest, request_json, projection_json, provider_call_item_id, projection_item_id, revision, lifecycle_status, resolution_item_id, tool_result_item_id, resolution_json, client_message_id, resume_claim_owner, resume_claim_token, resume_claim_expires_at, terminal_error, resolved_at, completed_at, created_at, updated_at FROM conversation_interactions";

struct InteractionRow {
    values: [String; 9],
    provider_kind: String,
    provider_instance_key: String,
    selection_mode: String,
    credential_revision: i64,
    model: String,
    reasoning_effort: Option<String>,
    tool_catalog_digest: String,
    request_json: String,
    projection_json: String,
    provider_call_item_id: String,
    projection_item_id: String,
    revision: i64,
    status: String,
    resolution_item_id: Option<String>,
    tool_result_item_id: Option<String>,
    resolution_json: Option<String>,
    client_message_id: Option<String>,
    resume_claim_owner: Option<String>,
    resume_claim_token: Option<String>,
    resume_claim_expires_at: Option<String>,
    terminal_error: Option<String>,
    resolved_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

fn interaction_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InteractionRow> {
    Ok(InteractionRow {
        values: [
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(8)?,
            row.get(12)?,
        ],
        provider_kind: row.get(7)?,
        provider_instance_key: row.get(9)?,
        selection_mode: row.get(10)?,
        credential_revision: row.get(11)?,
        model: row.get(12)?,
        reasoning_effort: row.get(13)?,
        tool_catalog_digest: row.get(14)?,
        request_json: row.get(15)?,
        projection_json: row.get(16)?,
        provider_call_item_id: row.get(17)?,
        projection_item_id: row.get(18)?,
        revision: row.get(19)?,
        status: row.get(20)?,
        resolution_item_id: row.get(21)?,
        tool_result_item_id: row.get(22)?,
        resolution_json: row.get(23)?,
        client_message_id: row.get(24)?,
        resume_claim_owner: row.get(25)?,
        resume_claim_token: row.get(26)?,
        resume_claim_expires_at: row.get(27)?,
        terminal_error: row.get(28)?,
        resolved_at: row.get(29)?,
        completed_at: row.get(30)?,
        created_at: row.get(31)?,
        updated_at: row.get(32)?,
    })
}

fn parse_row(row: InteractionRow) -> Result<ConversationInteractionRecord, StoreError> {
    Ok(ConversationInteractionRecord {
        interaction_id: row.values[0].clone(),
        conversation_id: row.values[1].clone(),
        originating_turn_id: row.values[2].clone(),
        kind: ConversationInteractionKind::parse(&row.values[3])?,
        provider_call_id: row.values[4].clone(),
        canonical_tool_name: row.values[5].clone(),
        provider_tool_name: row.values[6].clone(),
        provider_kind: row.provider_kind,
        provider_account_id: row.values[7].clone(),
        provider_instance_key: row.provider_instance_key,
        selection_mode: row.selection_mode,
        credential_revision: u64::try_from(row.credential_revision).map_err(|_| {
            StoreError::Schema("negative interaction credential revision".to_string())
        })?,
        model: row.model,
        reasoning_effort: row.reasoning_effort,
        tool_catalog_digest: row.tool_catalog_digest,
        request: deserialize_json(row.request_json)?,
        projection: deserialize_json(row.projection_json)?,
        provider_call_item_id: row.provider_call_item_id,
        projection_item_id: row.projection_item_id,
        revision: u64::try_from(row.revision)
            .map_err(|_| StoreError::Schema("negative interaction revision".to_string()))?,
        status: ConversationInteractionStatus::parse(&row.status)?,
        resolution_item_id: row.resolution_item_id,
        tool_result_item_id: row.tool_result_item_id,
        resolution: row.resolution_json.map(deserialize_json).transpose()?,
        client_message_id: row.client_message_id,
        resume_claim_owner: row.resume_claim_owner,
        resume_claim_token: row.resume_claim_token,
        resume_claim_expires_at: row.resume_claim_expires_at,
        terminal_error: row.terminal_error,
        resolved_at: row.resolved_at,
        completed_at: row.completed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn interaction_by_id_tx(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<Option<ConversationInteractionRecord>, StoreError> {
    let sql = format!("{INTERACTION_FIELDS} WHERE interaction_id = ?1");
    tx.query_row(&sql, [id], interaction_from_row)
        .optional()
        .map_err(StoreError::Sqlite)?
        .map(parse_row)
        .transpose()
}

fn append_item_tx(
    tx: &Transaction<'_>,
    item_id: String,
    item: NewConversationItem,
) -> Result<ConversationItemRecord, StoreError> {
    let sequence: i64 = tx.query_row("SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM conversation_items WHERE conversation_id = ?1", [&item.conversation_id], |row| row.get(0))?;
    tx.execute("INSERT INTO conversation_items (item_id, conversation_id, turn_id, parent_item_id, sequence_index, kind, status, author_actor_id, content_text, payload_json, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![item_id, item.conversation_id, item.turn_id, item.parent_item_id, sequence, item.kind.as_str(), item.status.as_str(), item.author.actor_id.to_string(), item.content_text, serialize_json(&item.payload_json)?, serialize_json(&item.metadata)?])?;
    load_conversation_item_tx(tx, &item_id)?
        .ok_or_else(|| conflict("interaction item disappeared before commit"))
}

fn conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
