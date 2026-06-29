//! Local daemon protocol and Unix-socket runtime.

mod agent_onboarding;
mod client;
mod memory_pipeline;
mod memory_tool;
mod prompts;
mod protocol;
mod runtime;
mod server;
#[cfg(test)]
mod tests;
pub(crate) mod web;

pub use client::DaemonClient;
pub(crate) use protocol::TurnStreamEvent;
pub use protocol::{
    AgentStatus, DEFAULT_DAEMON_SOCKET_NAME, DaemonError, DaemonRequest, DaemonResponse,
    DaemonServerConfig, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
    default_socket_path, is_connection_refused, socket_path_for_home,
};
pub use server::run_daemon;
