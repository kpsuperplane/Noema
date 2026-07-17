//! Local daemon runtime and web protocol types.

/// Runtime state reached an invariant violation.
pub const SYSTEM_ERROR_RUNTIME_INVARIANT: &str = "runtime_invariant_violation";

pub(crate) mod agent_name_tool;
pub(crate) mod agent_onboarding;
mod artifact_tool;
mod events;
mod memory;
pub(crate) mod prompts;
mod protocol;
pub(crate) mod runtime;
mod task_artifact_tool;
pub(crate) mod task_delivery;
pub(crate) mod task_run_context;
mod task_runtime;
pub(crate) mod task_tool;
#[cfg(test)]
mod tests;

pub use events::{ConversationRuntimeEvent, RuntimeEventRegistry, TaskRuntimeEvent};
pub use protocol::TurnStreamEvent;
pub use protocol::{
    AgentStatus, RuntimeError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use runtime::turn_timing::mark_turn_timing_event;
pub use runtime::{RuntimeHandle, RuntimeSpawnConfig};
pub use task_delivery::deliver_task_status_event;
pub use task_runtime::TaskRuntimeHandle;
