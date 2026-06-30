//! Embedded SurrealDB-backed canonical Noema store.

mod agents;
mod claims;
mod conversations;
mod error;
mod ids;
mod mcp;
pub mod objects;
mod ontology;
mod provider_accounts;
mod retrieval;
mod runtime;
mod schema;

#[cfg(test)]
pub(crate) mod tests;

pub use agents::{AgentRecord, NewAgent};
pub use claims::{
    ClaimStatus, ClaimSummary, ClaimWriteOutcome, ConsolidationMatch, ConsolidationMatchRequest,
    EvidenceAuthority, EvidenceCandidate, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewClaimCandidate, RelatedClaimCandidate,
    RelatedClaimRecord, SupersedeClaimCandidate,
};
pub use error::StoreError;
pub use mcp::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord, NewMcpServer,
    NewMcpTool, NewTrustedIdentitySelector, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};
pub use ontology::{
    EntityCandidate, EntityType, PredicateProposalCandidate, PredicateProposalFilter,
    PredicateProposalRecord, PredicateRecord,
};
pub use retrieval::{ClaimRetrievalResult, RetrievedClaim};
pub use runtime::{NoemaStore, StoreConfig};
