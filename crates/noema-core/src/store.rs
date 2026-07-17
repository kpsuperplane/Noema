//! SQLite-backed canonical Noema store.

/// Store-backed state violated a closed Noema schema assumption.
pub const SYSTEM_ERROR_STORE_INVARIANT: &str = "store_invariant_violation";

mod agent_run_rows;
mod agent_runs;
mod agent_runtime_preferences;
mod agents;
mod artifact_metadata_port;
#[cfg(test)]
mod artifact_metadata_port_tests;
mod artifact_writes;
mod artifacts;
mod auxiliary_model_preferences;
mod context_summaries;
mod conversations;
mod error;
mod ids;
mod local_model_activation;
#[cfg(test)]
mod local_model_port_tests;
mod local_model_ports;
mod local_model_rows;
mod local_models;
#[cfg(test)]
mod local_models_tests;
mod mcp;
mod memory_repository;
mod memory_service;
mod provider_account_port;
mod provider_accounts;
mod provider_capability_bindings;
mod provider_capability_port;
mod provider_catalog_port;
#[cfg(test)]
mod provider_persistence_port_tests;
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

pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{AgentRecord, AgentSystemRole, HumanRecord, NewAgent};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, NewAuxiliaryModelPreference, TOOL_PROGRESS_AUDIT_TASK_ID,
    WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use error::StoreError;
pub use mcp::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord, NewToolCalibration,
    ToolCalibrationRecord,
};
#[cfg(test)]
pub(crate) use mcp::{NewMcpServer, NewMcpTool};
pub use runtime::{NoemaStore, StoreConfig};
