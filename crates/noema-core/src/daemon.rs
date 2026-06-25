//! Local daemon protocol and Unix-socket runtime.

mod client;
mod memory_pipeline;
mod protocol;
mod runtime;
mod server;
#[cfg(test)]
mod tests;

pub use client::DaemonClient;
pub use protocol::{
    DEFAULT_DAEMON_SOCKET_NAME, DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig,
    StartedConversation, TurnActivityStatus, TurnTranscriptItem, default_socket_path,
    is_connection_refused, socket_path_for_home,
};
pub use server::run_daemon;
