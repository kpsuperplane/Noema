//! Embedded SurrealDB-backed canonical Noema store.

mod claims;
mod conversations;
mod error;
mod ids;
pub mod objects;
mod ontology;
mod provider_accounts;
mod runtime;
mod schema;

#[cfg(test)]
mod tests;

pub use claims::{
    ClaimStatus, ClaimSummary, EvidenceAuthority, EvidenceCandidate, NewClaimCandidate,
};
pub use error::StoreError;
pub use ontology::{EntityCandidate, EntityType, PredicateRecord};
pub use runtime::{NoemaStore, StoreConfig};
