//! Embedded SurrealDB-backed canonical Noema store.

mod agent_runtime_preferences;
mod agents;
mod auxiliary_model_preferences;
mod claims;
mod context_summaries;
mod conversations;
mod error;
mod ids;
mod mcp;
mod ontology;
mod provider_accounts;
mod provider_capability_bindings;
mod retrieval;
mod runtime;
mod schema;

#[cfg(test)]
pub(crate) mod tests;

pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{AgentRecord, NewAgent};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, NewAuxiliaryModelPreference, TOOL_PROGRESS_AUDIT_TASK_ID,
    WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use claims::{
    ClaimStatus, ClaimSummary, ClaimWriteOutcome, ConsolidationMatch, ConsolidationMatchRequest,
    EvidenceAuthority, EvidenceCandidate, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewClaimCandidate, RelatedClaimCandidate,
    RelatedClaimRecord, SupersedeClaimCandidate,
};
pub use context_summaries::{ConversationContextSummaryRecord, NewConversationContextSummary};
pub use error::StoreError;
pub use mcp::{
    McpApprovalRequestRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    NewTrustedIdentitySelector, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};
pub use ontology::{
    EntityCandidate, EntityType, PredicateProposalCandidate, PredicateProposalFilter,
    PredicateProposalRecord, PredicateRecord,
};
pub use provider_accounts::{NewProviderAccount, ProviderAccountCatalogEntry};
pub use provider_capability_bindings::ProviderCapabilityBindingRecord;
pub use retrieval::{ClaimRetrievalResult, RetrievedClaim};
pub use runtime::{NoemaStore, StoreConfig};
