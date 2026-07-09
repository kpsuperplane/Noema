//! SQLite-backed canonical Noema store.

mod agent_runtime_preferences;
mod agents;
mod artifacts;
mod auxiliary_model_preferences;
mod context_summaries;
mod conversations;
mod error;
mod ids;
mod mcp;
mod memory_service;
mod provider_accounts;
mod provider_capability_bindings;
mod runtime;
mod schema;
mod sqlite;

#[cfg(test)]
pub(crate) mod tests;

pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{AgentRecord, HumanRecord, NewAgent};
pub use artifacts::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, NewAuxiliaryModelPreference, TOOL_PROGRESS_AUDIT_TASK_ID,
    WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use context_summaries::{ConversationContextSummaryRecord, NewConversationContextSummary};
pub use error::StoreError;
pub use mcp::{
    McpApprovalRequestRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    NewTrustedIdentitySelector, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};
pub use memory_service::{
    MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};
pub use provider_accounts::{NewProviderAccount, ProviderAccountCatalogEntry};
pub use provider_capability_bindings::ProviderCapabilityBindingRecord;
pub use runtime::{NoemaStore, StoreConfig};
