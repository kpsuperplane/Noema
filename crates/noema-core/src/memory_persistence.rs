//! Durable memory persistence repositories.
//!
//! This module is the persistence slice used by chat integration and memory
//! inspection. Postgres is the canonical structured store.

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
mod provider_accounts;
mod queries;
mod repository;

pub use conversations::{
    AgentStatus, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    ConversationRecord, ConversationTurnRecord, ConversationTurnStatus, NewConversation,
    NewConversationItem, NewConversationTurn, ReplayMode,
};
pub use error::MemoryPersistenceError;
pub use models::{
    MemoryAuthorityLevel, MemoryExtractionMethod, MemorySummary, MemoryType, NewMemoryCandidate,
    NewMemoryParticipant, NewMemorySubject, ObjectProvenanceSource,
};
pub use objects::{ObjectRef, ObjectType};
pub use provenance::{DeleteConversationItem, NewObjectProvenanceEdge};
pub use provider_accounts::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
pub use repository::PostgresMemoryRepository;
