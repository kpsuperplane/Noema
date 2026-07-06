use crate::{NoemaPathError, StoreError, provider::ProviderError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// Live agent coordination state exported by daemon and web protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AgentStatus {
    /// No agent work is currently active.
    Idle,
    /// Human or external input has been accepted.
    InputReceived,
    /// The agent is producing or planning a response.
    Thinking,
    /// The agent is waiting on a tool invocation.
    ToolRunning,
    /// A newer turn is waiting for a prior turn's side effects to settle.
    WaitingForPreviousTurnCompletion,
    /// The agent is interrupting a previous turn.
    Interrupting,
    /// The conversation is in an error state.
    Error,
}

impl From<crate::PersistedAgentStatus> for AgentStatus {
    fn from(status: crate::PersistedAgentStatus) -> Self {
        match status {
            crate::PersistedAgentStatus::Idle => Self::Idle,
            crate::PersistedAgentStatus::InputReceived => Self::InputReceived,
            crate::PersistedAgentStatus::Thinking => Self::Thinking,
            crate::PersistedAgentStatus::ToolRunning => Self::ToolRunning,
            crate::PersistedAgentStatus::WaitingForPreviousTurnCompletion => {
                Self::WaitingForPreviousTurnCompletion
            }
            crate::PersistedAgentStatus::Interrupting => Self::Interrupting,
            crate::PersistedAgentStatus::Error => Self::Error,
        }
    }
}

/// Transcript item emitted by a daemon turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TurnTranscriptItem {
    /// User text acknowledged by durable persistence.
    UserText {
        /// Text authored by the user.
        text: String,
    },
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

/// Errors produced by daemon runtime and web operations.
#[derive(Debug, Error)]
pub enum DaemonError {
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

    /// Embedded store operation failed.
    #[error(transparent)]
    Store(Box<StoreError>),

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

impl From<StoreError> for DaemonError {
    fn from(source: StoreError) -> Self {
        Self::Store(Box::new(source))
    }
}

/// Durable conversation id allocated by Noema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConversation {
    /// Durable Noema conversation id.
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TurnStreamEvent {
    ConversationItem {
        conversation_id: String,
        item_id: String,
        cursor: Option<String>,
        turn_id: Option<String>,
        metadata: serde_json::Value,
        item: Box<TurnTranscriptItem>,
    },
    AssistantTextDelta {
        conversation_id: String,
        turn_id: String,
        stream_id: String,
        response_index: usize,
        delta: String,
    },
    AgentStatusChanged {
        conversation_id: String,
        status: AgentStatus,
    },
}

impl TurnStreamEvent {
    #[must_use]
    pub(crate) fn conversation_id(&self) -> &str {
        match self {
            Self::ConversationItem {
                conversation_id, ..
            }
            | Self::AssistantTextDelta {
                conversation_id, ..
            }
            | Self::AgentStatusChanged {
                conversation_id, ..
            } => conversation_id,
        }
    }
}
