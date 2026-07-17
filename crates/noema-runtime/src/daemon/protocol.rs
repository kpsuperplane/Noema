pub use noema_conversations::AgentStatus;
use noema_home::NoemaPathError;
use noema_providers::{
    MultipleChoiceOption, MultipleChoiceSelectionMode, ProviderError, ProviderRouteError,
};
use noema_store::StoreError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

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
    /// Assistant-authored multiple-choice prompt.
    MultipleChoicePrompt {
        /// Question or instruction shown above the options.
        prompt: String,
        /// Whether one or many options may be selected.
        selection_mode: MultipleChoiceSelectionMode,
        /// Ordered selectable options.
        options: Vec<MultipleChoiceOption>,
    },
    /// Human-authored multiple-choice selection.
    MultipleChoiceSelection {
        /// Prompt item this selection answers.
        prompt_item_id: String,
        /// Selection mode from the prompt.
        selection_mode: MultipleChoiceSelectionMode,
        /// Selected options in prompt order.
        selected_options: Vec<MultipleChoiceOption>,
    },
    /// Recoverable or terminal notice shown in the transcript.
    ErrorNotice {
        /// Human-readable message.
        message: String,
        /// Whether the chat turn itself can continue.
        recoverable: bool,
    },
    /// Reference to a governed artifact snapshot for transcript display.
    ArtifactReference {
        /// Stable artifact id.
        artifact_id: String,
        /// Optional referenced artifact version id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        artifact_version_id: Option<String>,
        /// Display title captured when the reference item was written.
        title: String,
        /// Product-defined artifact kind label.
        artifact_kind: String,
        /// Durable storage family for the referenced artifact.
        storage_kind: String,
        /// External durable URL when the artifact is externally hosted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        external_url: Option<String>,
        /// Local download route when the artifact bytes live in Noema.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        download_url: Option<String>,
        /// Optional media type for the referenced version payload.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        media_type: Option<String>,
    },
    /// Reference to a durable background task.
    TaskReference {
        /// Stable task id.
        task_id: String,
        /// Display title captured when the reference was written.
        title: String,
        /// Canonical [`noema_tasks::TaskStatus::as_str`] value.
        status: String,
        /// Current executor revision at the time of the reference.
        revision: i64,
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
pub enum RuntimeError {
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

    /// Provider selection could not be leased to an exact runtime instance.
    #[error(transparent)]
    ProviderRoute(#[from] ProviderRouteError),

    /// Embedded store operation failed.
    #[error(transparent)]
    Store(Box<StoreError>),

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

impl From<StoreError> for RuntimeError {
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

/// Transport-neutral event emitted while a runtime turn executes.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnStreamEvent {
    /// One complete durable transcript item.
    ConversationItem {
        /// Durable conversation identifier.
        conversation_id: String,
        /// Durable transcript item identifier.
        item_id: String,
        /// Durable ordered cursor, when persistence has completed.
        cursor: Option<String>,
        /// Runtime turn identifier, when the item belongs to a turn.
        turn_id: Option<String>,
        /// Safe structured item metadata.
        metadata: serde_json::Value,
        /// Transcript projection.
        item: Box<TurnTranscriptItem>,
    },
    /// Incremental assistant text for immediate display.
    AssistantTextDelta {
        /// Durable conversation identifier.
        conversation_id: String,
        /// Runtime turn identifier.
        turn_id: String,
        /// Stable stream identifier.
        stream_id: String,
        /// Provider response item index.
        response_index: usize,
        /// Incremental text.
        delta: String,
    },
    /// Conversation agent coordination state changed.
    AgentStatusChanged {
        /// Durable conversation identifier.
        conversation_id: String,
        /// New agent state.
        status: AgentStatus,
    },
}

impl TurnStreamEvent {
    #[must_use]
    /// Return the durable conversation associated with this stream event.
    pub fn conversation_id(&self) -> &str {
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
