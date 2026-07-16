use async_graphql::{Enum, InputObject, Json, Result, SimpleObject, Union};
use futures_util::Stream;
use serde_json::Value;

use crate::{
    AgentStatus, TurnActivityStatus, TurnTranscriptItem,
    daemon::{TurnStreamEvent, mark_graphql_turn_event},
};

use super::{
    ConversationLiveEvent, ConversationSubscriptionRegistry, errors::graphql_error,
    schema::GraphqlState,
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

impl From<AgentStatus> for GraphqlAgentStatus {
    fn from(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Idle => Self::Idle,
            AgentStatus::InputReceived => Self::InputReceived,
            AgentStatus::Thinking => Self::Thinking,
            AgentStatus::ToolRunning => Self::ToolRunning,
            AgentStatus::WaitingForPreviousTurnCompletion => Self::WaitingForPreviousTurnCompletion,
            AgentStatus::Interrupting => Self::Interrupting,
            AgentStatus::Error => Self::Error,
        }
    }
}

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

impl From<TurnActivityStatus> for GraphqlTurnActivityStatus {
    fn from(status: TurnActivityStatus) -> Self {
        match status {
            TurnActivityStatus::Started => Self::Started,
            TurnActivityStatus::Completed => Self::Completed,
            TurnActivityStatus::Failed => Self::Failed,
        }
    }
}

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

impl From<noema_providers::MultipleChoiceSelectionMode> for GraphqlMultipleChoiceSelectionMode {
    fn from(mode: noema_providers::MultipleChoiceSelectionMode) -> Self {
        match mode {
            noema_providers::MultipleChoiceSelectionMode::PickOne => Self::PickOne,
            noema_providers::MultipleChoiceSelectionMode::PickMany => Self::PickMany,
        }
    }
}

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
    /// Display title captured when the reference was written.
    pub title: String,
    /// Canonical task status string from [`crate::TaskStatus::as_str`].
    pub status: String,
    /// Executor revision represented by this reference.
    pub revision: i64,
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
            TurnTranscriptItem::TaskReference {
                task_id,
                title,
                status,
                revision,
            } => Self::TaskReference(GraphqlTaskReference {
                task_id,
                title,
                status,
                revision,
            }),
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
        latest_conversation_transcript_page(state, &self.conversation_id, limit).await
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

pub(super) async fn primary_conversation(
    state: &GraphqlState,
) -> Result<Option<GraphqlPrimaryConversation>> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store, runtime.provider_kind()).await?;
    let conversation = store
        .primary_conversation_for_human("human:local")
        .await
        .map_err(graphql_error)?;
    Ok(conversation.map(|conversation| GraphqlPrimaryConversation {
        conversation_id: conversation.conversation_id,
        provider: provider_kind,
    }))
}

pub(super) async fn ensure_primary_conversation(
    state: &GraphqlState,
    cwd: Option<String>,
) -> Result<GraphqlPrimaryConversation> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store, runtime.provider_kind()).await?;
    let account = state
        .provider_account_operations()?
        .active_accounts()
        .await
        .map_err(graphql_error)?
        .into_iter()
        .find(|account| account.provider_kind == provider_kind && account.is_default);
    if !crate::graphql::is_user_onboarded_for_chat(account) {
        return Err(async_graphql::Error::new(
            "Noema onboarding is incomplete. Finish local model setup or connect a provider account before starting chat.",
        ));
    }

    let started = runtime
        .start_primary_conversation(cwd)
        .await
        .map_err(graphql_error)?;

    Ok(GraphqlPrimaryConversation {
        conversation_id: started.conversation_id,
        provider: provider_kind,
    })
}

pub(super) async fn conversation_transcript_page(
    state: &GraphqlState,
    input: GraphqlConversationTranscriptPageInput,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(
        state,
        &input.conversation_id,
        input.cursor.as_deref(),
        input.limit,
    )
    .await
}

pub(super) async fn latest_conversation_transcript_page(
    state: &GraphqlState,
    conversation_id: &str,
    limit: Option<i32>,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(state, conversation_id, None, limit).await
}

async fn visible_conversation_transcript_page(
    state: &GraphqlState,
    conversation_id: &str,
    cursor: Option<&str>,
    limit: Option<i32>,
) -> Result<GraphqlConversationTranscriptPage> {
    let limit = limit.unwrap_or(80);
    if limit < 1 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at least 1",
        ));
    }
    if limit > 200 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at most 200",
        ));
    }

    let page = state
        .store()?
        .list_visible_conversation_item_page(conversation_id, cursor, i64::from(limit))
        .await
        .map_err(graphql_error)?;
    let mut items = Vec::new();
    for record in page.items {
        if let Some(item) =
            crate::graphql::web_conversation_item_from_record(record).map_err(graphql_error)?
        {
            items.push(GraphqlConversationItem::from(item));
        }
    }

    Ok(GraphqlConversationTranscriptPage {
        items,
        page_info: GraphqlConversationTranscriptPageInfo {
            before_cursor: page.before_cursor,
            has_more_before: page.has_more_before,
            limit: i32::try_from(page.limit).unwrap_or(i32::MAX),
        },
    })
}

async fn primary_agent_provider_kind(
    store: &crate::NoemaStore,
    default_provider_kind: &str,
) -> Result<String> {
    Ok(store
        .get_agent_runtime_preference("agent:primary")
        .await
        .map_err(graphql_error)?
        .map(|preference| preference.provider_kind)
        .unwrap_or_else(|| default_provider_kind.to_string()))
}

pub(super) async fn send_conversation_turn(
    state: &GraphqlState,
    input: GraphqlSendConversationTurnInput,
) -> Result<GraphqlTurnAccepted> {
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let published_client_message_id = client_message_id.clone();
    let input_text = input.input;
    let completion_conversation_id = conversation_id.clone();
    mark_graphql_turn_event(
        "graphql_turn_received",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "input_chars": input_text.chars().count(),
        }),
    );

    tokio::spawn(async move {
        mark_graphql_turn_event(
            "graphql_runtime_task_started",
            &completion_conversation_id,
            published_client_message_id.as_deref(),
            serde_json::json!({}),
        );
        let completion = runtime.turn_with_client_message_id(
            completion_conversation_id,
            input_text,
            item_tx,
            published_client_message_id.clone(),
        );
        tokio::pin!(completion);
        let mut published_error_notice = false;
        loop {
            tokio::select! {
                Some(event) = item_rx.recv() => {
                    if turn_event_is_error_notice(&event) {
                        published_error_notice = true;
                    }
                    mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                    subscriptions.publish(ConversationLiveEvent::Turn {
                        client_message_id: published_client_message_id.clone(),
                        event: Box::new(event),
                    });
                }
                result = &mut completion => {
                    while let Ok(event) = item_rx.try_recv() {
                        if turn_event_is_error_notice(&event) {
                            published_error_notice = true;
                        }
                        mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                        subscriptions.publish(ConversationLiveEvent::Turn {
                            client_message_id: published_client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    publish_turn_terminal_events(
                        &subscriptions,
                        conversation_id,
                        published_client_message_id,
                        published_error_notice,
                        result,
                    );
                    break;
                }
            }
        }
    });

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

pub(super) async fn send_multiple_choice_selection(
    state: &GraphqlState,
    input: GraphqlSendMultipleChoiceSelectionInput,
) -> Result<GraphqlTurnAccepted> {
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let published_client_message_id = client_message_id.clone();
    let completion_conversation_id = conversation_id.clone();
    let prompt_item_id = input.prompt_item_id.clone();
    let selected_option_ids = input.selected_option_ids.clone();
    mark_graphql_turn_event(
        "graphql_multiple_choice_selection_received",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "prompt_item_id": prompt_item_id,
            "selected_option_count": selected_option_ids.len(),
        }),
    );

    tokio::spawn(async move {
        mark_graphql_turn_event(
            "graphql_runtime_task_started",
            &completion_conversation_id,
            published_client_message_id.as_deref(),
            serde_json::json!({}),
        );
        let completion = runtime.select_multiple_choice_with_client_message_id(
            completion_conversation_id,
            prompt_item_id,
            selected_option_ids,
            item_tx,
            published_client_message_id.clone(),
        );
        tokio::pin!(completion);
        let mut published_error_notice = false;
        loop {
            tokio::select! {
                Some(event) = item_rx.recv() => {
                    if turn_event_is_error_notice(&event) {
                        published_error_notice = true;
                    }
                    mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                    subscriptions.publish(ConversationLiveEvent::Turn {
                        client_message_id: published_client_message_id.clone(),
                        event: Box::new(event),
                    });
                }
                result = &mut completion => {
                    while let Ok(event) = item_rx.try_recv() {
                        if turn_event_is_error_notice(&event) {
                            published_error_notice = true;
                        }
                        mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                        subscriptions.publish(ConversationLiveEvent::Turn {
                            client_message_id: published_client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    publish_turn_terminal_events(
                        &subscriptions,
                        conversation_id,
                        published_client_message_id,
                        published_error_notice,
                        result,
                    );
                    break;
                }
            }
        }
    });

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

pub(super) fn conversation_events(
    subscriptions: ConversationSubscriptionRegistry,
    conversation_id: String,
) -> impl Stream<Item = GraphqlConversationEvent> {
    let mut rx = subscriptions.subscribe(&conversation_id);

    async_stream::stream! {
        yield GraphqlConversationEvent::SubscriptionReady(
            GraphqlSubscriptionReadyEvent {
                conversation_id: conversation_id.clone(),
            },
        );

        while let Ok(event) = rx.recv().await {
            match event {
                ConversationLiveEvent::Turn {
                    client_message_id,
                    event,
                } => match *event {
                    crate::daemon::TurnStreamEvent::ConversationItem {
                            conversation_id,
                            item_id,
                            cursor,
                            turn_id,
                            metadata,
                            item,
                        } => {
                    yield GraphqlConversationEvent::ConversationItem(
                        Box::new(GraphqlConversationItemEvent {
                            conversation_id,
                            client_message_id,
                            item_id,
                            cursor,
                            turn_id,
                            metadata: async_graphql::Json(metadata),
                            item: (*item).into(),
                        }),
                    );
                }
                    crate::daemon::TurnStreamEvent::AgentStatusChanged {
                            conversation_id,
                            status,
                        } => {
                    yield GraphqlConversationEvent::AgentStatusChanged(
                        GraphqlAgentStatusEvent {
                            conversation_id,
                            status: status.into(),
                        },
                    );
                }
                        crate::daemon::TurnStreamEvent::AssistantTextDelta {
                            conversation_id,
                            turn_id,
                            stream_id,
                            response_index,
                            delta,
                        } => {
                    yield GraphqlConversationEvent::AssistantTextDelta(
                        GraphqlAssistantTextDeltaEvent {
                            conversation_id,
                            turn_id,
                            stream_id,
                            response_index,
                            delta,
                        },
                    );
                }
                },
                ConversationLiveEvent::Completed {
                    conversation_id,
                    client_message_id,
                } => {
                    yield GraphqlConversationEvent::TurnCompleted(
                        GraphqlTurnCompletedEvent {
                            conversation_id,
                            client_message_id,
                        },
                    );
                }
            }
        }
    }
}

fn publish_turn_terminal_events(
    subscriptions: &ConversationSubscriptionRegistry,
    conversation_id: String,
    client_message_id: Option<String>,
    published_error_notice: bool,
    result: std::result::Result<(), crate::DaemonError>,
) {
    if let Err(ref error) = result
        && !published_error_notice
    {
        let error_item_id = client_message_id.as_ref().map_or_else(
            || format!("graphql_runtime_error:{conversation_id}:uncorrelated"),
            |client_message_id| {
                format!("graphql_runtime_error:{conversation_id}:{client_message_id}")
            },
        );
        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: client_message_id.clone(),
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: conversation_id.clone(),
                item_id: error_item_id,
                cursor: None,
                turn_id: None,
                metadata: serde_json::json!({}),
                item: Box::new(crate::TurnTranscriptItem::ErrorNotice {
                    message: error.to_string(),
                    recoverable: false,
                }),
            }),
        });
    }

    mark_graphql_turn_event(
        "graphql_turn_completed",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "success": result.is_ok(),
            "published_error_notice": published_error_notice,
        }),
    );
    subscriptions.publish(ConversationLiveEvent::Completed {
        conversation_id,
        client_message_id,
    });
}

fn mark_graphql_published_turn_event(event: &TurnStreamEvent, client_message_id: Option<&str>) {
    match event {
        TurnStreamEvent::ConversationItem {
            conversation_id,
            item_id,
            turn_id,
            item,
            ..
        } => {
            let (item_kind, activity_kind, status) = match item.as_ref() {
                TurnTranscriptItem::UserText { .. } => ("user_text", None, None),
                TurnTranscriptItem::AssistantText { .. } => ("assistant_text", None, None),
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status,
                    ..
                } => (
                    "activity",
                    Some(activity_kind.as_str()),
                    Some(activity_status_label(*status)),
                ),
                TurnTranscriptItem::A2uiCard { schema, .. } => {
                    ("a2ui_card", Some(schema.as_str()), None)
                }
                TurnTranscriptItem::MultipleChoicePrompt { .. } => {
                    ("multiple_choice_prompt", None, None)
                }
                TurnTranscriptItem::MultipleChoiceSelection { .. } => {
                    ("multiple_choice_selection", None, None)
                }
                TurnTranscriptItem::ErrorNotice { .. } => ("error_notice", None, None),
                TurnTranscriptItem::ArtifactReference { .. } => ("artifact_reference", None, None),
                TurnTranscriptItem::TaskReference { status, .. } => {
                    ("task_reference", None, Some(status.as_str()))
                }
            };
            mark_graphql_turn_event(
                "graphql_publish_conversation_item",
                conversation_id,
                client_message_id,
                serde_json::json!({
                    "item_id": item_id,
                    "turn_id": turn_id,
                    "item_kind": item_kind,
                    "activity_kind": activity_kind,
                    "status": status,
                }),
            );
        }
        TurnStreamEvent::AssistantTextDelta {
            conversation_id,
            turn_id,
            stream_id,
            response_index,
            delta,
        } => mark_graphql_turn_event(
            "graphql_publish_assistant_delta",
            conversation_id,
            client_message_id,
            serde_json::json!({
                "turn_id": turn_id,
                "stream_id": stream_id,
                "response_index": response_index,
                "delta_chars": delta.chars().count(),
            }),
        ),
        TurnStreamEvent::AgentStatusChanged {
            conversation_id,
            status,
        } => mark_graphql_turn_event(
            "graphql_publish_agent_status",
            conversation_id,
            client_message_id,
            serde_json::json!({
                "status": format!("{status:?}"),
            }),
        ),
    }
}

fn activity_status_label(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}

fn turn_event_is_error_notice(event: &TurnStreamEvent) -> bool {
    matches!(
        event,
        TurnStreamEvent::ConversationItem {
            item,
            ..
        } if matches!(item.as_ref(), crate::TurnTranscriptItem::ErrorNotice { .. })
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn agent_status_converts_to_graphql() {
        assert_eq!(
            GraphqlAgentStatus::from(AgentStatus::Thinking),
            GraphqlAgentStatus::Thinking
        );
    }

    #[test]
    fn transcript_item_converts_to_graphql_user_text() {
        let item = GraphqlTranscriptItem::from(TurnTranscriptItem::UserText {
            text: "hello".to_string(),
        });

        match item {
            GraphqlTranscriptItem::UserText(value) => assert_eq!(value.text, "hello"),
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn transcript_item_converts_to_graphql_artifact_reference() {
        let item = GraphqlTranscriptItem::from(TurnTranscriptItem::ArtifactReference {
            artifact_id: "artifact_1".to_string(),
            artifact_version_id: Some("artifact_version_1".to_string()),
            title: "Noema notes".to_string(),
            artifact_kind: "document".to_string(),
            storage_kind: "external_url".to_string(),
            external_url: Some("https://notion.so/noema-notes".to_string()),
            download_url: None,
            media_type: Some("text/html".to_string()),
        });

        match item {
            GraphqlTranscriptItem::ArtifactReference(value) => {
                assert_eq!(value.artifact_id, "artifact_1");
                assert_eq!(
                    value.artifact_version_id.as_deref(),
                    Some("artifact_version_1")
                );
                assert_eq!(value.title, "Noema notes");
                assert_eq!(value.artifact_kind, "document");
                assert_eq!(value.storage_kind, "external_url");
                assert_eq!(
                    value.external_url.as_deref(),
                    Some("https://notion.so/noema-notes")
                );
                assert!(value.download_url.is_none());
                assert_eq!(value.media_type.as_deref(), Some("text/html"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn transcript_item_converts_to_graphql_task_reference() {
        let item = GraphqlTranscriptItem::from(TurnTranscriptItem::TaskReference {
            task_id: "task_1".to_string(),
            title: "Research providers".to_string(),
            status: crate::TaskStatus::Reviewing.as_str().to_string(),
            revision: 2,
        });

        match item {
            GraphqlTranscriptItem::TaskReference(value) => {
                assert_eq!(value.task_id, "task_1");
                assert_eq!(value.title, "Research providers");
                assert_eq!(value.status, "reviewing");
                assert_eq!(value.revision, 2);
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn activity_status_converts_to_graphql() {
        assert_eq!(
            GraphqlTurnActivityStatus::from(TurnActivityStatus::Completed),
            GraphqlTurnActivityStatus::Completed
        );
    }

    #[tokio::test]
    async fn runtime_turn_error_publishes_error_notice_and_completion() {
        let subscriptions = ConversationSubscriptionRegistry::default();
        let mut rx = subscriptions.subscribe("conversation_1");

        publish_turn_terminal_events(
            &subscriptions,
            "conversation_1".to_string(),
            Some("client_1".to_string()),
            false,
            Err(crate::DaemonError::Remote("provider failed".to_string())),
        );

        let event = rx.recv().await.expect("error notice event");
        let ConversationLiveEvent::Turn {
            client_message_id,
            event,
        } = event
        else {
            panic!("expected turn event");
        };
        assert_eq!(client_message_id.as_deref(), Some("client_1"));
        let crate::daemon::TurnStreamEvent::ConversationItem {
            conversation_id,
            item_id,
            metadata,
            item,
            ..
        } = *event
        else {
            panic!("expected conversation item");
        };
        assert_eq!(conversation_id, "conversation_1");
        assert_eq!(item_id, "graphql_runtime_error:conversation_1:client_1");
        assert_eq!(metadata, json!({}));
        let crate::TurnTranscriptItem::ErrorNotice {
            message,
            recoverable,
        } = *item
        else {
            panic!("expected error notice");
        };
        assert!(message.contains("provider failed"));
        assert!(!recoverable);

        let event = rx.recv().await.expect("completion event");
        let ConversationLiveEvent::Completed {
            conversation_id,
            client_message_id,
        } = event
        else {
            panic!("expected completion event");
        };
        assert_eq!(conversation_id, "conversation_1");
        assert_eq!(client_message_id.as_deref(), Some("client_1"));
    }

    #[tokio::test]
    async fn runtime_turn_error_does_not_duplicate_published_error_notice() {
        let subscriptions = ConversationSubscriptionRegistry::default();
        let mut rx = subscriptions.subscribe("conversation_1");
        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: Some("client_1".to_string()),
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "item:persisted_error".to_string(),
                cursor: None,
                turn_id: Some("turn_1".to_string()),
                metadata: json!({}),
                item: Box::new(crate::TurnTranscriptItem::ErrorNotice {
                    message: "provider failed".to_string(),
                    recoverable: false,
                }),
            }),
        });

        publish_turn_terminal_events(
            &subscriptions,
            "conversation_1".to_string(),
            Some("client_1".to_string()),
            true,
            Err(crate::DaemonError::Remote("provider failed".to_string())),
        );

        let mut error_notice_count = 0;
        let mut completion_count = 0;
        while let Ok(event) = rx.try_recv() {
            match event {
                ConversationLiveEvent::Turn { event, .. } => {
                    if matches!(
                        *event,
                        TurnStreamEvent::ConversationItem {
                            item,
                            ..
                        } if matches!(
                            item.as_ref(),
                            crate::TurnTranscriptItem::ErrorNotice { .. }
                        )
                    ) {
                        error_notice_count += 1;
                    }
                }
                ConversationLiveEvent::Completed { .. } => {
                    completion_count += 1;
                }
            }
        }

        assert_eq!(error_notice_count, 1);
        assert_eq!(completion_count, 1);
    }
}
