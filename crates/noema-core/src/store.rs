//! SQLite-backed canonical Noema store.

/// Store-backed state violated a closed Noema schema assumption.
pub const SYSTEM_ERROR_STORE_INVARIANT: &str = "store_invariant_violation";

mod agent_runs;
mod agent_runtime_preferences;
mod agents;
mod artifacts;
mod auxiliary_model_preferences;
mod context_summaries;
mod conversations;
mod error;
mod ids;
mod local_model_activation;
mod local_model_rows;
mod local_models;
#[cfg(test)]
mod local_models_tests;
mod mcp;
mod memory_service;
mod provider_accounts;
mod provider_capability_bindings;
mod run_items;
mod runtime;
mod schema;
mod schema_upgrade;
mod sqlite;
mod task_controls;
mod task_events;
mod task_execution_policy;
mod task_model_pools;
mod task_reads;
mod tasks;

#[cfg(test)]
pub(crate) mod tests;

pub use agent_runs::{AgentRunHeartbeat, AgentRunRecord, NewAgentRun};
pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{AgentRecord, AgentSystemRole, HumanRecord, NewAgent};
pub use artifacts::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
    validate_external_artifact_url,
};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, NewAuxiliaryModelPreference, TOOL_PROGRESS_AUDIT_TASK_ID,
    WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use error::StoreError;
pub use mcp::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord, NewMcpServer,
    NewMcpTool, NewToolCalibration, ToolCalibrationRecord,
};
pub use memory_service::{
    MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};
pub use provider_accounts::{NewProviderAccount, ProviderAccountCatalogEntry};
pub use provider_capability_bindings::ProviderCapabilityBindingRecord;
pub use run_items::{AgentRunItemRecord, AgentRunItemStatus, NewAgentRunItem};
pub use runtime::{NoemaStore, StoreConfig};
pub use task_events::TaskEventRecord;
pub use task_model_pools::{NewTaskModelPoolEntry, TaskModelPoolEntry};
pub use tasks::{TaskRecord, TaskReviewRecord, TaskSubmissionArtifactRecord, TaskSubmissionRecord};
