//! Governed, transport-neutral agent execution runtime.

pub mod a2ui;
pub mod acp;
mod acp_terminal_bridge;
mod agent_execution;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod contract_test_support;
mod daemon;
#[cfg(feature = "eval-support")]
pub mod eval_support;
mod file_tools;
mod search;
#[cfg(test)]
mod test_support;
mod web_backend;
mod web_fetch;

pub use daemon::{
    AgentStatus, CapabilityIntegrationKind, CapabilitySetupCompletion, ConversationRuntimeEvent,
    MemoryRuntimeEvent, RuntimeError, RuntimeEventRegistry, RuntimeHandle, RuntimeSpawnConfig,
    StartedConversation, TaskRuntimeEvent, TaskRuntimeHandle, TurnActivityStatus, TurnStreamEvent,
    TurnTranscriptItem, WorkRuntimeEvent, mark_turn_timing_event,
};
pub use file_tools::run_file_parse_worker_if_requested;
pub use web_backend::{
    WebBackendFuture, WebBackendRequest, WebBackendResolver, WebBackendResolverError,
    WebBackendResolverHandle,
};
