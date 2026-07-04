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

/// Error notice transcript item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ErrorNotice")]
pub struct GraphqlErrorNotice {
    /// Human-readable error message.
    pub message: String,
    /// Whether the chat turn can continue.
    pub recoverable: bool,
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
    /// Error notice.
    ErrorNotice(GraphqlErrorNotice),
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
            TurnTranscriptItem::ErrorNotice {
                message,
                recoverable,
            } => Self::ErrorNotice(GraphqlErrorNotice {
                message,
                recoverable,
            }),
        }
    }
}

/// Started conversation with replay payload.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationStarted")]
pub struct GraphqlConversationStarted {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Provider used for the conversation.
    pub provider: String,
    /// Visible replay items.
    pub replay: Vec<GraphqlConversationItem>,
}

/// One visible conversation item.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationItem")]
pub struct GraphqlConversationItem {
    /// Durable conversation item id.
    pub item_id: String,
    /// Durable conversation turn id.
    pub turn_id: Option<String>,
    /// Transcript item to render.
    pub item: GraphqlTranscriptItem,
}

impl From<crate::daemon::web::ConversationReplayItem> for GraphqlConversationItem {
    fn from(item: crate::daemon::web::ConversationReplayItem) -> Self {
        Self {
            item_id: item.item_id,
            turn_id: item.turn_id,
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

pub(super) async fn start_primary_conversation(
    state: &GraphqlState,
    cwd: Option<String>,
) -> Result<GraphqlConversationStarted> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store, runtime.provider_kind()).await?;
    let account = store
        .active_provider_account(&provider_kind)
        .await
        .map_err(graphql_error)?;
    if !crate::daemon::web::is_user_onboarded_for_chat(account) {
        return Err(async_graphql::Error::new(
            "Noema onboarding is incomplete. Connect a provider account before starting chat.",
        ));
    }

    let started = runtime
        .start_primary_conversation(cwd)
        .await
        .map_err(graphql_error)?;
    let replay_records =
        crate::daemon::web::visible_conversation_replay(store, &started.conversation_id)
            .await
            .map_err(graphql_error)?;
    let mut replay = Vec::new();
    for record in replay_records {
        if let Some(item) =
            crate::daemon::web::web_conversation_item_from_record(record).map_err(graphql_error)?
        {
            replay.push(GraphqlConversationItem::from(item));
        }
    }

    Ok(GraphqlConversationStarted {
        conversation_id: started.conversation_id,
        provider: provider_kind,
        replay,
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
                            turn_id,
                            metadata,
                            item,
                        } => {
                    yield GraphqlConversationEvent::ConversationItem(
                        Box::new(GraphqlConversationItemEvent {
                            conversation_id,
                            client_message_id,
                            item_id,
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
                            delta,
                        } => {
                    yield GraphqlConversationEvent::AssistantTextDelta(
                        GraphqlAssistantTextDeltaEvent {
                            conversation_id,
                            turn_id,
                            stream_id,
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
                TurnTranscriptItem::ErrorNotice { .. } => ("error_notice", None, None),
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
            delta,
        } => mark_graphql_turn_event(
            "graphql_publish_assistant_delta",
            conversation_id,
            client_message_id,
            serde_json::json!({
                "turn_id": turn_id,
                "stream_id": stream_id,
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
