//! Durable memory persistence repositories.
//!
//! This module is the small persistence slice used by chat integration. It
//! currently keeps the SQLite runtime repository while introducing the
//! Postgres schema bootstrap used by the server migration path.

mod context_packets;
mod conversations;
mod error;
mod helpers;
pub(crate) mod models;
mod objects;
mod postgres_context_graph;
mod postgres_helpers;
mod postgres_schema;
#[cfg(test)]
mod postgres_tests;
mod provenance;
mod queries;
mod repository;
mod schema;
#[cfg(test)]
mod tests;

pub use conversations::{
    AgentStatus, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    ConversationRecord, ConversationTurnRecord, ConversationTurnStatus, NewConversation,
    NewConversationItem, NewConversationTurn, ReplayMode,
};
pub use error::MemoryPersistenceError;
pub use models::{
    MemoryAuthorityLevel, MemoryExtractionMethod, MemorySummary, MemoryType, NewMemoryCandidate,
    NewMemoryParticipant, NewMemorySubject, NewRelationshipClaim, ObjectProvenanceSource,
};
pub use objects::{ObjectRef, ObjectType};
pub use provenance::{DeleteConversationItem, NewObjectProvenanceEdge};
pub use repository::{PostgresMemoryRepository, SqliteMemoryRepository};
