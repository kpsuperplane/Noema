//! Local daemon runtime and web protocol types.

/// Runtime state reached an invariant violation.
pub const SYSTEM_ERROR_RUNTIME_INVARIANT: &str = "runtime_invariant_violation";

mod agent_name_tool;
mod agent_onboarding;
mod artifact_tool;
#[cfg(feature = "local-model-evals")]
#[doc(hidden)]
pub mod eval_support;
mod memory;
mod prompts;
mod protocol;
mod runtime;
mod task_artifact_tool;
pub(crate) mod task_delivery;
mod task_run_context;
mod task_runtime;
pub(crate) mod task_tool;
#[cfg(test)]
mod tests;

pub(crate) use protocol::TurnStreamEvent;
pub use protocol::{
    AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use runtime::RuntimeModelProvider;
pub(crate) use runtime::turn_timing::mark_graphql_turn_event;
pub(crate) use runtime::{CodexRuntimeHandle, RuntimeProviderMap};
pub(crate) use task_runtime::TaskRuntimeHandle;
