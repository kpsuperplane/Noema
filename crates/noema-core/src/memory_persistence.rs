//! SQLite-backed durable memory repository.
//!
//! This module is the small persistence slice used by chat integration. It
//! initializes `db/noema.sqlite`, creates the canonical memory tables needed
//! for chat-created memories, writes provenance and participants, and exposes
//! a recent-memory listing for CLI inspection.

mod context_packets;
mod conversations;
mod error;
mod helpers;
pub(crate) mod models;
mod objects;
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
pub use repository::SqliteMemoryRepository;
