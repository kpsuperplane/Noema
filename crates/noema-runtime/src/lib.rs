//! Governed, transport-neutral agent execution runtime.

/// Shared execution roles and role-aware tool dispatch policy.
pub mod agent_execution;
mod daemon;
#[cfg(feature = "eval-support")]
pub mod eval_support;
mod search;
#[cfg(test)]
mod test_support;
mod web_backend;
mod web_fetch;

pub use daemon::{
    AgentStatus, ConversationRuntimeEvent, RuntimeError, RuntimeEventRegistry, RuntimeHandle,
    RuntimeSpawnConfig, StartedConversation, TaskRuntimeEvent, TaskRuntimeHandle,
    TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem, deliver_task_status_event,
    mark_turn_timing_event,
};
pub use web_backend::{
    WebBackendFuture, WebBackendRequest, WebBackendResolver, WebBackendResolverError,
    WebBackendResolverHandle,
};
