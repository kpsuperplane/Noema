//! Local daemon runtime and web protocol types.

/// Runtime state reached an invariant violation.
pub const SYSTEM_ERROR_RUNTIME_INVARIANT: &str = "runtime_invariant_violation";

pub(crate) fn log_system_error(
    logger: &noema_home::SystemErrorLogger,
    code: &'static str,
    message: &'static str,
    context: Option<serde_json::Value>,
    error: impl ToString,
) {
    let mut event = noema_home::SystemErrorEvent::new(code, message);
    if let Some(context) = context {
        event = event.with_context(context);
    }
    logger.try_append(event.with_error_chain([error.to_string()]));
}

pub(crate) mod agent_name_tool;
pub(crate) mod agent_onboarding;
mod artifact_tool;
mod events;
mod memory;
pub(crate) mod prompts;
mod protocol;
pub(crate) mod runtime;
mod task_artifact_tool;
pub(crate) mod task_run_context;
mod task_runtime;
pub(crate) mod task_tool;
#[cfg(test)]
mod tests;

pub use events::{
    ConversationRuntimeEvent, MemoryRuntimeEvent, RuntimeEventRegistry, TaskRuntimeEvent,
    WorkRuntimeEvent,
};
pub use protocol::TurnStreamEvent;
pub use protocol::{
    AgentStatus, RuntimeError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use runtime::turn_timing::mark_turn_timing_event;
pub use runtime::{CapabilitySetupCompletion, RuntimeHandle, RuntimeSpawnConfig};
pub use task_runtime::TaskRuntimeHandle;
