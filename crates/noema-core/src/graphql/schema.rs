use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;

use crate::daemon::TurnStreamEvent;

use super::{
    ConversationLiveEvent, ConversationSubscriptionRegistry,
    types::{
        GraphqlAgentStatusEvent, GraphqlAssistantConnection, GraphqlAssistantTextDeltaEvent,
        GraphqlConversationEvent, GraphqlConversationItem, GraphqlConversationItemEvent,
        GraphqlConversationStarted, GraphqlLocalServiceStatus, GraphqlLocalStatus,
        GraphqlMemoryStorageStatus, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
        GraphqlSendConversationTurnInput, GraphqlStartProviderAuthAttemptInput,
        GraphqlSubscriptionReadyEvent, GraphqlTurnAccepted, GraphqlTurnCompletedEvent,
    },
};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Shared state available to GraphQL resolvers.
#[derive(Clone)]
pub struct GraphqlState {
    web_state: Option<crate::daemon::web::WebState>,
    subscriptions: ConversationSubscriptionRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            web_state: None,
            subscriptions: ConversationSubscriptionRegistry::default(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state backed by the local web server runtime.
    #[must_use]
    pub(crate) fn from_web_state(web_state: crate::daemon::web::WebState) -> Self {
        Self {
            subscriptions: web_state.subscriptions().clone(),
            web_state: Some(web_state),
            memory_storage: GraphqlMemoryStorageStatus::Unavailable,
        }
    }

    pub(crate) fn web_state(&self) -> Result<&crate::daemon::web::WebState> {
        self.web_state
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema web state is unavailable"))
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        &self.subscriptions
    }
}

/// Build the Noema GraphQL schema.
#[must_use]
pub fn build_schema(state: GraphqlState) -> GraphqlSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state)
        .finish()
}

/// Root GraphQL query object.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Return local Noema status.
    async fn local_status(&self, ctx: &Context<'_>) -> GraphqlLocalStatus {
        let state = ctx.data_unchecked::<GraphqlState>();
        GraphqlLocalStatus {
            local_service: GraphqlLocalServiceStatus::Running,
            assistant_connection: GraphqlAssistantConnection::Codex,
            memory_storage: state.memory_storage,
        }
    }

    /// Return onboarding status.
    async fn onboarding_status(&self, ctx: &Context<'_>) -> Result<GraphqlOnboardingStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let account = web
            .store()
            .active_provider_account("codex")
            .await
            .map_err(graphql_error)?;
        let account = crate::daemon::web::reconcile_onboarding_provider_account(
            web.store(),
            web.paths(),
            account,
        )
        .await
        .map_err(graphql_error)?;

        Ok(crate::onboarding_status_from_account(account).into())
    }

    /// Return a short-lived provider auth attempt.
    async fn provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlProviderAuthAttempt>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let attempt = web
            .provider_auth()
            .poll_attempt(&attempt_id)
            .await
            .map_err(graphql_error)?;
        if let Some(attempt) = &attempt {
            crate::daemon::web::persist_provider_account_status_from_attempt(web.store(), attempt)
                .await
                .map_err(graphql_error)?;
        }
        Ok(attempt.map(Into::into))
    }
}

/// Root GraphQL mutation object.
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Start a provider auth attempt.
    async fn start_provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartProviderAuthAttemptInput,
    ) -> Result<GraphqlProviderAuthAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let request = crate::daemon::web::ProviderAuthStartRequest {
            provider_kind: input.provider_kind,
            provider_account_id: input.provider_account_id,
            method: input.method.into(),
        };
        let attempt = crate::daemon::web::start_provider_auth_attempt_view(web, request)
            .await
            .map_err(|error| async_graphql::Error::new(error.message()))?;
        Ok(attempt.into())
    }

    /// Start or resume the primary conversation.
    async fn start_primary_conversation(
        &self,
        ctx: &Context<'_>,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<GraphqlConversationStarted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let account = web
            .store()
            .active_provider_account("codex")
            .await
            .map_err(graphql_error)?;
        if !crate::daemon::web::is_user_onboarded_for_chat(account) {
            return Err(async_graphql::Error::new(
                "Noema onboarding is incomplete. Connect a provider account before starting chat.",
            ));
        }

        let started = web
            .runtime()
            .start_primary_conversation(model, cwd)
            .await
            .map_err(graphql_error)?;
        let replay_records =
            crate::daemon::web::visible_conversation_replay(web.store(), &started.conversation_id)
                .await
                .map_err(graphql_error)?;
        let mut replay = Vec::new();
        for record in replay_records {
            if let Some(item) = crate::daemon::web::web_conversation_item_from_record(record)
                .map_err(graphql_error)?
            {
                replay.push(GraphqlConversationItem::from(item));
            }
        }

        Ok(GraphqlConversationStarted {
            conversation_id: started.conversation_id,
            provider: "codex".to_string(),
            replay,
        })
    }

    /// Send a conversation turn.
    async fn send_conversation_turn(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendConversationTurnInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let runtime = web.runtime().clone();
        let subscriptions = state.subscriptions().clone();
        let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
        let conversation_id = input.conversation_id.clone();
        let client_message_id = input.client_message_id.clone();
        let published_client_message_id = client_message_id.clone();
        let input_text = input.input;
        let completion_conversation_id = conversation_id.clone();

        tokio::spawn(async move {
            let completion = runtime.turn(completion_conversation_id, input_text, item_tx);
            tokio::pin!(completion);
            loop {
                tokio::select! {
                    Some(event) = item_rx.recv() => {
                        subscriptions.publish(ConversationLiveEvent::Turn {
                            client_message_id: published_client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    result = &mut completion => {
                        while let Ok(event) = item_rx.try_recv() {
                            subscriptions.publish(ConversationLiveEvent::Turn {
                                client_message_id: published_client_message_id.clone(),
                                event: Box::new(event),
                            });
                        }
                        publish_turn_terminal_events(
                            &subscriptions,
                            conversation_id,
                            published_client_message_id,
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
}

/// Root GraphQL subscription object.
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Stream conversation events.
    async fn conversation_events(
        &self,
        ctx: &Context<'_>,
        conversation_id: String,
    ) -> impl Stream<Item = GraphqlConversationEvent> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let mut rx = state.subscriptions().subscribe(&conversation_id);

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
}

fn graphql_error(error: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

fn publish_turn_terminal_events(
    subscriptions: &ConversationSubscriptionRegistry,
    conversation_id: String,
    client_message_id: Option<String>,
    result: std::result::Result<(), crate::DaemonError>,
) {
    if let Err(error) = result {
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

    subscriptions.publish(ConversationLiveEvent::Completed {
        conversation_id,
        client_message_id,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use serde_json::json;

    #[test]
    fn schema_sdl_exposes_initial_noema_fields() {
        let schema = build_schema(GraphqlState::for_tests());
        let sdl = schema.sdl();

        assert!(sdl.contains("type Query"));
        assert!(sdl.contains("localStatus"));
        assert!(sdl.contains("onboardingStatus"));
        assert!(sdl.contains("type Mutation"));
        assert!(sdl.contains("startProviderAuthAttempt"));
        assert!(sdl.contains("startPrimaryConversation"));
        assert!(sdl.contains("sendConversationTurn"));
        assert!(sdl.contains("type Subscription"));
        assert!(sdl.contains("conversationEvents"));
        assert!(sdl.contains("GraphqlAssistantTextDeltaEvent"));
    }

    #[tokio::test]
    async fn runtime_turn_error_publishes_error_notice_and_completion() {
        let subscriptions = ConversationSubscriptionRegistry::default();
        let mut rx = subscriptions.subscribe("conversation_1");

        publish_turn_terminal_events(
            &subscriptions,
            "conversation_1".to_string(),
            Some("client_1".to_string()),
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
    async fn conversation_events_emits_ready_before_live_events() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on GraphqlSubscriptionReadyEvent {
                  conversationId
                }
                ... on GraphqlTurnCompletedEvent {
                  conversationId
                  clientMessageId
                }
              }
            }
            "#,
        ));

        let response = stream.next().await.expect("ready response");
        let data = response.data.into_json().expect("ready json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("GraphqlSubscriptionReadyEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/conversationId")
                .and_then(serde_json::Value::as_str),
            Some("conversation_1")
        );

        subscriptions.publish(ConversationLiveEvent::Completed {
            conversation_id: "conversation_1".to_string(),
            client_message_id: Some("client_1".to_string()),
        });
        let response = stream.next().await.expect("completion response");
        let data = response.data.into_json().expect("completion json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("GraphqlTurnCompletedEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/clientMessageId")
                .and_then(serde_json::Value::as_str),
            Some("client_1")
        );
    }

    #[tokio::test]
    async fn subscription_streams_assistant_text_delta_event() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on GraphqlAssistantTextDeltaEvent {
                  conversationId
                  turnId
                  streamId
                  delta
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "GraphqlSubscriptionReadyEvent"
        );

        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AssistantTextDelta {
                conversation_id: "conversation_1".to_string(),
                turn_id: "turn_1".to_string(),
                stream_id: "assistant_stream:turn_1:initial".to_string(),
                delta: "Hel".to_string(),
            }),
        });

        let response = stream.next().await.expect("delta response");
        let data = response.data.into_json().expect("delta json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "GraphqlAssistantTextDeltaEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["turnId"], "turn_1");
        assert_eq!(event["streamId"], "assistant_stream:turn_1:initial");
        assert_eq!(event["delta"], "Hel");
    }

    #[tokio::test]
    async fn subscription_streams_conversation_item_metadata() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on GraphqlConversationItemEvent {
                  conversationId
                  itemId
                  metadata
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "GraphqlSubscriptionReadyEvent"
        );

        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "item_1".to_string(),
                turn_id: Some("turn_1".to_string()),
                metadata: json!({"stream_id":"assistant_stream:turn_1:initial"}),
                item: Box::new(crate::TurnTranscriptItem::AssistantText {
                    text: "Hello".to_string(),
                }),
            }),
        });

        let response = stream.next().await.expect("item response");
        let data = response.data.into_json().expect("item json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "GraphqlConversationItemEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["itemId"], "item_1");
        assert_eq!(
            event["metadata"],
            json!({"stream_id":"assistant_stream:turn_1:initial"})
        );
    }
}
