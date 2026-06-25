//! SQLite-backed durable memory repository.
//!
//! This module is the small persistence slice used by chat integration. It
//! initializes `db/noema.sqlite`, creates the canonical memory tables needed
//! for chat-created memories, writes provenance and participants, and exposes
//! a recent-memory listing for CLI inspection.

mod context_packets;
mod error;
mod helpers;
mod models;
mod objects;
mod queries;
mod repository;
mod schema;
#[cfg(test)]
mod tests;

pub use error::MemoryPersistenceError;
pub use models::{
    ChatMemorySource, MemoryAuthorityLevel, MemoryExtractionMethod, MemorySummary, MemoryType,
    NewChatMemoryCandidate, NewChatTurn, NewMemoryParticipant, NewMemorySubject,
    NewRelationshipClaim,
};
pub use objects::{ObjectRef, ObjectType};
pub use repository::SqliteMemoryRepository;
