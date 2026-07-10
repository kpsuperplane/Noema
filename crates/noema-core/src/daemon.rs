//! Local daemon runtime and web protocol types.

mod agent_name_tool;
mod agent_onboarding;
mod artifact_tool;
#[cfg(test)]
mod browser_acceptance;
mod memory;
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
pub use runtime::RuntimeModelProvider;
pub(crate) use runtime::turn_timing::mark_graphql_turn_event;
pub(crate) use runtime::{CodexRuntimeHandle, RuntimeProviderMap};
#[cfg(test)]
pub(crate) use web_server::TestDaemonWebServer;
pub use web_server::{DaemonWebServerConfig, run_daemon_web};
