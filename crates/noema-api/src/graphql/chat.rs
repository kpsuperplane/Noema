use async_graphql::{Enum, InputObject, Json, Result, SimpleObject, Union};
use futures_util::Stream;
use serde_json::Value;

use noema_runtime::{
    AgentStatus, ConversationRuntimeEvent, RuntimeEventRegistry, TurnActivityStatus,
    TurnStreamEvent, TurnTranscriptItem, mark_turn_timing_event,
};

use super::{errors::graphql_error, schema::GraphqlState};

mod events;
mod operations;

pub(super) use events::conversation_events;
#[cfg(test)]
use events::publish_turn_terminal_events;
pub(super) use operations::{
    conversation_transcript_page, ensure_primary_conversation, latest_conversation_transcript_page,
    primary_conversation, send_conversation_turn, send_multiple_choice_selection,
};

/// Agent status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "AgentStatus")]
pub enum GraphqlAgentStatus {
    /// No agent work is active.
    Idle,
    /// Input has been accepted.
    InputReceived,
    /// Agent is producing or planning.
    Thinking,
    /// Agent is waiting on a tool invocation.
    ToolRunning,
    /// A newer turn is waiting for a previous turn.
    WaitingForPreviousTurnCompletion,
    /// Agent is interrupting a previous turn.
    Interrupting,
    /// Conversation is in an error state.
    Error,
}

graphql_enum_from!(AgentStatus => GraphqlAgentStatus {
    Idle => Idle,
    InputReceived => InputReceived,
    Thinking => Thinking,
    ToolRunning => ToolRunning,
    WaitingForPreviousTurnCompletion => WaitingForPreviousTurnCompletion,
    Interrupting => Interrupting,
    Error => Error,
});

/// Turn activity status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "TurnActivityStatus")]
pub enum GraphqlTurnActivityStatus {
    /// Activity started.
    Started,
    /// Activity completed.
    Completed,
    /// Activity failed.
    Failed,
}

graphql_enum_from!(TurnActivityStatus => GraphqlTurnActivityStatus {
    Started => Started,
    Completed => Completed,
    Failed => Failed,
});

/// User text transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "UserText")]
pub struct GraphqlUserText {
    /// Text authored by the user.
    pub text: String,
}

/// Assistant text transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AssistantText")]
pub struct GraphqlAssistantText {
    /// Text to render as the assistant response.
    pub text: String,
}

/// Activity transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Activity")]
pub struct GraphqlActivity {
    /// Stable activity id.
    pub id: String,
    /// Activity kind.
    pub activity_kind: String,
    /// Activity status.
    pub status: GraphqlTurnActivityStatus,
    /// Short display title.
    pub title: String,
    /// Optional summary.
    pub summary: Option<String>,
    /// Structured metadata as JSON.
    pub metadata: Json<Value>,
}

/// Structured card transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "A2UiCard")]
pub struct GraphqlA2uiCard {
    /// Stable card id.
    pub id: String,
    /// Card schema.
    pub schema: String,
    /// Card payload as JSON.
    pub payload: Json<Value>,
}

/// Multiple-choice selection mode exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "MultipleChoiceSelectionMode")]
pub enum GraphqlMultipleChoiceSelectionMode {
    /// One option may be selected.
    PickOne,
    /// One or more options may be selected.
    PickMany,
}

graphql_enum_from!(noema_providers::MultipleChoiceSelectionMode => GraphqlMultipleChoiceSelectionMode {
    PickOne => PickOne,
    PickMany => PickMany,
});

/// One multiple-choice option.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MultipleChoiceOption")]
pub struct GraphqlMultipleChoiceOption {
    /// Stable semantic option id.
    pub id: String,
    /// Human-visible label.
    pub label: String,
}

impl From<noema_providers::MultipleChoiceOption> for GraphqlMultipleChoiceOption {
    fn from(option: noema_providers::MultipleChoiceOption) -> Self {
        Self {
            id: option.id,
            label: option.label,
        }
    }
}

/// Assistant multiple-choice transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MultipleChoicePrompt")]
pub struct GraphqlMultipleChoicePrompt {
    /// Question or instruction shown above the options.
    pub prompt: String,
    /// Whether one or many options may be selected.
    pub selection_mode: GraphqlMultipleChoiceSelectionMode,
    /// Ordered selectable options.
    pub options: Vec<GraphqlMultipleChoiceOption>,
}

/// Human multiple-choice selection transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MultipleChoiceSelection")]
pub struct GraphqlMultipleChoiceSelection {
    /// Prompt item this selection answers.
    pub prompt_item_id: String,
    /// Selection mode from the prompt.
    pub selection_mode: GraphqlMultipleChoiceSelectionMode,
    /// Selected options in prompt order.
    pub selected_options: Vec<GraphqlMultipleChoiceOption>,
}

/// Error notice transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ErrorNotice")]
pub struct GraphqlErrorNotice {
    /// Human-readable error message.
    pub message: String,
    /// Whether the chat turn can continue.
    pub recoverable: bool,
}

/// Artifact reference transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactReference")]
pub struct GraphqlArtifactReference {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Optional referenced artifact version id.
    pub artifact_version_id: Option<String>,
    /// Display title captured in the transcript row.
    pub title: String,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage family for the referenced artifact.
    pub storage_kind: String,
    /// External durable URL when the artifact is externally hosted.
    pub external_url: Option<String>,
    /// Local download route when the artifact bytes live in Noema.
    pub download_url: Option<String>,
    /// Optional media type for the referenced version payload.
    pub media_type: Option<String>,
}

/// Durable background task reference transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReference")]
pub struct GraphqlTaskReference {
    /// Stable task id.
    pub task_id: String,
}

/// Transcript item union.
#[derive(Clone, Debug, Union)]
#[graphql(name = "TranscriptItem")]
pub enum GraphqlTranscriptItem {
    /// User text.
    UserText(GraphqlUserText),
    /// Assistant text.
    AssistantText(GraphqlAssistantText),
    /// Activity row.
    Activity(GraphqlActivity),
    /// Structured card.
    A2uiCard(GraphqlA2uiCard),
    /// Multiple-choice prompt.
    MultipleChoicePrompt(GraphqlMultipleChoicePrompt),
    /// Multiple-choice selection.
    MultipleChoiceSelection(GraphqlMultipleChoiceSelection),
    /// Error notice.
    ErrorNotice(GraphqlErrorNotice),
    /// Artifact reference.
    ArtifactReference(GraphqlArtifactReference),
    /// Background task reference.
    TaskReference(GraphqlTaskReference),
}

impl From<TurnTranscriptItem> for GraphqlTranscriptItem {
    fn from(item: TurnTranscriptItem) -> Self {
        match item {
            TurnTranscriptItem::UserText { text } => Self::UserText(GraphqlUserText { text }),
            TurnTranscriptItem::AssistantText { text } => {
                Self::AssistantText(GraphqlAssistantText { text })
            }
            TurnTranscriptItem::Activity {
                id,
                activity_kind,
                status,
                title,
                summary,
                metadata,
            } => Self::Activity(GraphqlActivity {
                id,
                activity_kind,
                status: status.into(),
                title,
                summary,
                metadata: Json(metadata),
            }),
            TurnTranscriptItem::A2uiCard {
                id,
                schema,
                payload,
            } => Self::A2uiCard(GraphqlA2uiCard {
                id,
                schema,
                payload: Json(payload),
            }),
            TurnTranscriptItem::MultipleChoicePrompt {
                prompt,
                selection_mode,
                options,
            } => Self::MultipleChoicePrompt(GraphqlMultipleChoicePrompt {
                prompt,
                selection_mode: selection_mode.into(),
                options: options
                    .into_iter()
                    .map(GraphqlMultipleChoiceOption::from)
                    .collect(),
            }),
            TurnTranscriptItem::MultipleChoiceSelection {
                prompt_item_id,
                selection_mode,
                selected_options,
            } => Self::MultipleChoiceSelection(GraphqlMultipleChoiceSelection {
                prompt_item_id,
                selection_mode: selection_mode.into(),
                selected_options: selected_options
                    .into_iter()
                    .map(GraphqlMultipleChoiceOption::from)
                    .collect(),
            }),
            TurnTranscriptItem::ErrorNotice {
                message,
                recoverable,
            } => Self::ErrorNotice(GraphqlErrorNotice {
                message,
                recoverable,
            }),
            TurnTranscriptItem::ArtifactReference {
                artifact_id,
                artifact_version_id,
                title,
                artifact_kind,
                storage_kind,
                external_url,
                download_url,
                media_type,
            } => Self::ArtifactReference(GraphqlArtifactReference {
                artifact_id,
                artifact_version_id,
                title,
                artifact_kind,
                storage_kind,
                external_url,
                download_url,
                media_type,
            }),
            TurnTranscriptItem::TaskReference { task_id } => {
                Self::TaskReference(GraphqlTaskReference { task_id })
            }
        }
    }
}

/// Primary conversation identity.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "PrimaryConversation", complex)]
pub struct GraphqlPrimaryConversation {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Provider used for the conversation.
    pub provider: String,
}

#[async_graphql::ComplexObject]
impl GraphqlPrimaryConversation {
    /// Latest visible transcript page for this conversation.
    async fn latest_transcript_page(
        &self,
        ctx: &async_graphql::Context<'_>,
        limit: Option<i32>,
    ) -> Result<GraphqlConversationTranscriptPage> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        latest_conversation_transcript_page(state, principal, &self.conversation_id, limit).await
    }
}

/// Input for reading a visible transcript page.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ConversationTranscriptPageInput")]
pub struct GraphqlConversationTranscriptPageInput {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Opaque cursor. When omitted, reads the latest page.
    pub cursor: Option<String>,
    /// Page size. Defaults to 80 and must be within 1..=200.
    pub limit: Option<i32>,
}

/// Visible transcript page.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationTranscriptPage")]
pub struct GraphqlConversationTranscriptPage {
    /// Visible transcript items in display order.
    pub items: Vec<GraphqlConversationItem>,
    /// Paging metadata for older reads.
    pub page_info: GraphqlConversationTranscriptPageInfo,
}

/// Visible transcript page metadata.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationTranscriptPageInfo")]
pub struct GraphqlConversationTranscriptPageInfo {
    /// Cursor before the first returned item.
    pub before_cursor: Option<String>,
    /// Whether more visible items exist before this page.
    pub has_more_before: bool,
    /// Applied page size.
    pub limit: i32,
}

/// One visible conversation item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationItem")]
pub struct GraphqlConversationItem {
    /// Durable conversation item id.
    pub item_id: String,
    /// Opaque durable pagination cursor.
    pub cursor: String,
    /// Durable conversation turn id.
    pub turn_id: Option<String>,
    /// Structured durable item metadata.
    pub metadata: Json<Value>,
    /// Transcript item to render.
    pub item: GraphqlTranscriptItem,
}

impl From<crate::graphql::ConversationReplayItem> for GraphqlConversationItem {
    fn from(item: crate::graphql::ConversationReplayItem) -> Self {
        Self {
            item_id: item.item_id,
            cursor: item.cursor,
            turn_id: item.turn_id,
            metadata: Json(item.metadata),
            item: item.item.into(),
        }
    }
}

/// Input for sending a conversation turn.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SendConversationTurnInput")]
pub struct GraphqlSendConversationTurnInput {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// User input.
    pub input: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Input for sending a multiple-choice selection.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SendMultipleChoiceSelectionInput")]
pub struct GraphqlSendMultipleChoiceSelectionInput {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Durable multiple-choice prompt item id.
    pub prompt_item_id: String,
    /// Selected prompt option ids.
    pub selected_option_ids: Vec<String>,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Acceptance response for a queued conversation turn.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TurnAccepted")]
pub struct GraphqlTurnAccepted {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Conversation item event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationItemEvent")]
pub struct GraphqlConversationItemEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
    /// Durable conversation item id.
    pub item_id: String,
    /// Opaque durable pagination cursor, absent for transient runtime rows.
    pub cursor: Option<String>,
    /// Durable conversation turn id.
    pub turn_id: Option<String>,
    /// Structured durable item metadata.
    pub metadata: Json<Value>,
    /// Transcript item to render.
    pub item: GraphqlTranscriptItem,
}

/// Ephemeral assistant text delta event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AssistantTextDeltaEvent")]
pub struct GraphqlAssistantTextDeltaEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Durable conversation turn id.
    pub turn_id: String,
    /// Runtime stream id.
    pub stream_id: String,
    /// Zero-based response item index within the provider response.
    pub response_index: usize,
    /// Assistant text delta.
    pub delta: String,
}

/// Agent status event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentStatusEvent")]
pub struct GraphqlAgentStatusEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Current agent status.
    pub status: GraphqlAgentStatus,
}

/// Turn completion event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TurnCompletedEvent")]
pub struct GraphqlTurnCompletedEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Subscription readiness event delivered before live turn events.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "SubscriptionReadyEvent")]
pub struct GraphqlSubscriptionReadyEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
}

/// Conversation subscription event union.
#[derive(Clone, Debug, Union)]
#[graphql(name = "ConversationEvent")]
pub enum GraphqlConversationEvent {
    /// Subscription readiness event.
    SubscriptionReady(GraphqlSubscriptionReadyEvent),
    /// Conversation item event.
    ConversationItem(Box<GraphqlConversationItemEvent>),
    /// Ephemeral assistant text delta event.
    AssistantTextDelta(GraphqlAssistantTextDeltaEvent),
    /// Agent status changed event.
    AgentStatusChanged(GraphqlAgentStatusEvent),
    /// Turn completed event.
    TurnCompleted(GraphqlTurnCompletedEvent),
}

include!("chat_tests.rs");
