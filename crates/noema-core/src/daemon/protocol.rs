use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{
    NoemaPathError, NoemaPaths, WebConfig, memory_persistence::MemoryPersistenceError,
    provider::ProviderError, providers::codex::CodexProviderConfig,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// Filename used for the daemon Unix socket.
pub const DEFAULT_DAEMON_SOCKET_NAME: &str = "noema.sock";

/// Requests accepted by the Noema daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    /// Health-check request.
    Hello,
    /// Start a provider-backed conversation.
    ConversationStart {
        /// Optional model override.
        model: Option<String>,
        /// Optional working directory for the conversation.
        cwd: Option<String>,
        /// Optional conversation instructions.
        instructions: Option<String>,
    },
    /// Send one user turn to an existing conversation.
    ConversationTurn {
        /// Daemon conversation id.
        conversation_id: String,
        /// User input.
        input: String,
    },
    /// End a conversation and release provider state.
    ConversationEnd {
        /// Daemon conversation id.
        conversation_id: String,
    },
    /// Ask the daemon to shut down.
    Shutdown,
}

/// Responses emitted by the Noema daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Generic success response.
    Ok {
        /// Optional success message.
        message: Option<String>,
    },
    /// Conversation start response.
    ConversationStarted {
        /// Daemon conversation id.
        conversation_id: String,
        /// Provider used for the conversation.
        provider: String,
        /// Provider-native thread id.
        provider_thread_id: String,
    },
    /// One transcript item emitted by an in-progress turn.
    TurnTranscriptItem {
        /// Daemon conversation id.
        conversation_id: String,
        /// Transcript item to render in chat.
        item: TurnTranscriptItem,
    },
    /// Turn completion response.
    TurnCompleted {
        /// Daemon conversation id.
        conversation_id: String,
    },
    /// Error response returned over the daemon protocol.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

/// Transcript item emitted by a daemon turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TurnTranscriptItem {
    /// Assistant text.
    AssistantText {
        /// Text to render as the assistant response.
        text: String,
    },
    /// Activity notice for work that runs alongside the turn.
    Activity {
        /// Stable activity id for correlating updates.
        id: String,
        /// Activity kind, such as `memory_extraction`.
        activity_kind: String,
        /// Activity status.
        status: TurnActivityStatus,
        /// Short display title.
        title: String,
        /// Optional concise summary.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        summary: Option<String>,
        /// Structured metadata for future clients.
        #[ts(type = "Record<string, unknown>")]
        metadata: serde_json::Value,
    },
    /// Future structured UI card.
    A2uiCard {
        /// Stable card id.
        id: String,
        /// Card schema name.
        schema: String,
        /// Card payload.
        #[ts(type = "unknown")]
        payload: serde_json::Value,
    },
    /// Recoverable or terminal notice shown in the transcript.
    ErrorNotice {
        /// Human-readable message.
        message: String,
        /// Whether the chat turn itself can continue.
        recoverable: bool,
    },
}

/// Status for transcript activity notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum TurnActivityStatus {
    /// Activity started.
    Started,
    /// Activity completed.
    Completed,
    /// Activity failed.
    Failed,
}

/// Configuration required to start the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServerConfig {
    /// Unix socket path to bind.
    pub socket_path: PathBuf,
    /// Codex provider configuration used by daemon conversations.
    pub codex: CodexProviderConfig,
    /// Canonical SQLite database path.
    pub database_path: PathBuf,
    /// Local web UI configuration.
    pub web: WebConfig,
}

impl DaemonServerConfig {
    /// Create daemon server configuration.
    #[must_use]
    pub fn new(
        socket_path: PathBuf,
        codex: CodexProviderConfig,
        database_path: PathBuf,
        web: WebConfig,
    ) -> Self {
        Self {
            socket_path,
            codex,
            database_path,
            web,
        }
    }
}

/// Errors produced by daemon client and server operations.
#[derive(Debug, Error)]
pub enum DaemonError {
    /// Another daemon appears to be listening at the socket path.
    #[error("daemon is already running at {}", path.display())]
    AlreadyRunning {
        /// Existing daemon socket path.
        path: PathBuf,
    },

    /// Unix-socket operation failed.
    #[error("daemon socket error at {}: {source}", path.display())]
    Socket {
        /// Socket path involved in the operation.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Generic daemon I/O failure.
    #[error("daemon I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Local protocol serialization or state error.
    #[error("daemon protocol error: {0}")]
    Protocol(String),

    /// Remote daemon returned an error response.
    #[error("daemon returned error: {0}")]
    Remote(String),

    /// Provider operation failed.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// Memory persistence failed.
    #[error(transparent)]
    Memory(#[from] MemoryPersistenceError),

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

/// Return the default daemon socket path for the current process environment.
///
/// # Errors
///
/// Returns [`DaemonError`] when Noema path resolution fails.
pub fn default_socket_path() -> Result<PathBuf, DaemonError> {
    Ok(NoemaPaths::from_process_env()?.socket_path())
}

/// Return the daemon socket path under a home directory.
#[must_use]
pub fn socket_path_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref()
        .join(".noema")
        .join("run")
        .join(DEFAULT_DAEMON_SOCKET_NAME)
}

/// Conversation ids allocated by the daemon and provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConversation {
    /// Daemon-local conversation id.
    pub conversation_id: String,
    /// Provider-native thread id.
    pub provider_thread_id: String,
}

/// Return whether a daemon connection failure means no daemon is listening.
#[must_use]
pub fn is_connection_refused(error: &DaemonError) -> bool {
    matches!(
        error,
        DaemonError::Socket { source, .. }
            if matches!(source.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused)
    )
}
