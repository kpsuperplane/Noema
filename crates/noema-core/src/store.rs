//! Embedded SurrealDB-backed canonical Noema store.

mod claims;
mod conversations;
mod error;
mod ids;
pub mod objects;
mod ontology;
mod provider_accounts;
mod retrieval;
mod runtime;
mod schema;

#[cfg(test)]
pub(crate) mod tests;

pub use claims::{
    ClaimStatus, ClaimSummary, ClaimWriteOutcome, EvidenceAuthority, EvidenceCandidate,
    MemoryClaimDetail, MemoryClaimEvidence, MemoryClaimFilter, MemoryClaimRecord, MemoryGraph,
    MemoryGraphEdge, MemoryGraphFilter, MemoryGraphNode, MemoryGraphSummary, NewClaimCandidate,
};
pub use error::StoreError;
pub use ontology::{EntityCandidate, EntityType, PredicateRecord};
pub use retrieval::{ClaimRetrievalResult, RetrievedClaim};
pub use runtime::{NoemaStore, StoreConfig};
