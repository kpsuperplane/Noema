//! Local daemon runtime and web protocol types.

mod agent_name_tool;
mod agent_onboarding;
mod memory_pipeline;
mod memory_tool;
mod prompts;
mod protocol;
mod runtime;
#[cfg(test)]
mod tests;
pub(crate) mod web;
mod web_server;

pub(crate) use protocol::TurnStreamEvent;
pub use protocol::{
    AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub(crate) use runtime::CodexRuntimeHandle;
#[cfg(test)]
pub(crate) use runtime::RuntimeModelProvider;
pub use web_server::{DaemonWebServerConfig, run_daemon_web};
