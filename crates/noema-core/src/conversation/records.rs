use serde_json::{Value, json};

use super::status::{ConversationItemKind, ConversationItemStatus};
use crate::{ActorRef, ObjectRef};

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
        Self::local_chat_for_provider("codex", model, cwd)
    }

    /// Build the default local chat conversation shape for a runtime provider.
    #[must_use]
    pub fn local_chat_for_provider(
        provider: impl Into<String>,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Self {
        Self {
            title: None,
            owner: ObjectRef::human("human:local"),
            primary_human_id: Some("human:local".to_string()),
            primary_agent_id: Some("agent:primary".to_string()),
            provider: provider.into(),
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
    /// Actor that authored the item.
    pub author: ActorRef,
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
    /// Append-order sequence inside the conversation.
    pub sequence_index: i64,
    /// Opaque pagination cursor derived from append order.
    pub cursor: String,
    /// Semantic item kind.
    pub kind: ConversationItemKind,
    /// Item execution status.
    pub status: ConversationItemStatus,
    /// Readable item text, when any.
    pub content_text: Option<String>,
    /// Structured item payload.
    pub payload_json: Value,
}

/// Bounded visible conversation item page returned to product replay callers.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationItemPage {
    /// Visible items in ascending transcript order.
    pub items: Vec<ConversationItemRecord>,
    /// Cursor to pass when fetching the next older page.
    pub before_cursor: Option<String>,
    /// Whether older visible items exist before this page.
    pub has_more_before: bool,
    /// Effective clamped item limit.
    pub limit: i64,
}

/// Replay visibility mode for conversation items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Exclude soft-deleted items.
    Visible,
    /// Include soft-deleted items for audit views.
    Audit,
}
