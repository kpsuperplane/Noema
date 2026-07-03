use crate::memory::MemoryId;
use thiserror::Error;

/// Errors produced by durable memory persistence.
#[derive(Debug, Error)]
pub enum MemoryPersistenceError {
    /// JSON metadata could not be serialized.
    #[error("failed to serialize JSON metadata: {0}")]
    Json(#[from] serde_json::Error),

    /// A database value did not match a closed Noema vocabulary.
    #[error("invalid {kind} value in memory database: {value}")]
    InvalidEnum {
        /// Vocabulary kind.
        kind: &'static str,
        /// Stored value.
        value: String,
    },

    /// Unknown typed object reference kind.
    #[error("invalid object type: {value}")]
    InvalidObjectType {
        /// Rejected object type string.
        value: String,
    },

    /// Typed object reference id was empty.
    #[error("object id cannot be empty for type {object_type}")]
    EmptyObjectId {
        /// Object type string.
        object_type: String,
    },

    /// Referenced concrete object row does not exist.
    #[error("object reference not found: {object_type}:{object_id}")]
    ObjectRefNotFound {
        /// Object type string.
        object_type: String,
        /// Object id string.
        object_id: String,
    },

    /// A turn belongs to a different conversation than the item being written.
    #[error("turn {turn_id} does not belong to conversation {conversation_id}")]
    TurnConversationMismatch {
        /// Turn id.
        turn_id: String,
        /// Conversation id.
        conversation_id: String,
    },

    /// A memory expected to exist was not found.
    #[error("memory not found: {memory_id}")]
    MemoryNotFound {
        /// Missing memory id.
        memory_id: MemoryId,
    },

    /// Existing memory cannot be reinforced by the provided candidate.
    #[error("memory cannot be reinforced with this candidate: {memory_id}")]
    IncompatibleMemoryReinforcement {
        /// Rejected memory id.
        memory_id: MemoryId,
    },

    /// A provider account expected to exist was not found.
    #[error("provider account not found: {provider_account_id}")]
    ProviderAccountNotFound {
        /// Missing provider account id.
        provider_account_id: String,
    },

    /// Structured store operation failed.
    #[error(transparent)]
    Store(#[from] crate::StoreError),
}
