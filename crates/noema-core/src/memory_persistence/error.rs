use std::path::PathBuf;

use crate::memory::{MemoryId, MemoryStoreError};
use thiserror::Error;

/// Errors produced by SQLite memory persistence.
#[derive(Debug, Error)]
pub enum MemoryPersistenceError {
    /// The database directory could not be created.
    #[error("failed to create database directory {}: {source}", path.display())]
    CreateDatabaseDirectory {
        /// Directory path.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },

    /// SQLite could not open the database file.
    #[error("failed to open SQLite database {}: {source}", path.display())]
    Open {
        /// Database path.
        path: PathBuf,
        /// Underlying SQLite error.
        source: rusqlite::Error,
    },

    /// The database file expected for read-only inspection does not exist.
    #[error("noema memory database does not exist: {}", path.display())]
    MissingDatabase {
        /// Missing database path.
        path: PathBuf,
    },

    /// JSON metadata could not be serialized.
    #[error("failed to serialize JSON metadata: {0}")]
    Json(#[from] serde_json::Error),

    /// A database value did not match a closed Noema vocabulary.
    #[error("invalid {kind} value in SQLite: {value}")]
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

    /// A relationship expected to exist was not found.
    #[error("relationship not found: {relationship_id}")]
    RelationshipNotFound {
        /// Missing relationship id.
        relationship_id: String,
    },

    /// A current relationship did not include supporting memory.
    #[error("active or confirmed relationship {relationship_id} requires supporting memory")]
    RelationshipRequiresSupportingMemory {
        /// Relationship id or placeholder.
        relationship_id: String,
    },

    /// A relationship's supporting memory lacks provenance.
    #[error("relationship {relationship_id} requires provenance on supporting memory {memory_id}")]
    RelationshipSupportingMemoryMissingProvenance {
        /// Relationship id or placeholder.
        relationship_id: String,
        /// Supporting memory id.
        memory_id: MemoryId,
    },

    /// Stored memory graph rows violated retrieval policy invariants.
    #[error(transparent)]
    MemoryStore(#[from] MemoryStoreError),

    /// SQLite operation failed.
    #[error("SQLite memory persistence failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
