use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;

use super::{
    ConversationLiveEvent, ConversationSubscriptionRegistry,
    types::{
        GraphqlAgentStatusEvent, GraphqlAssistantConnection, GraphqlConversationEvent,
        GraphqlConversationItem, GraphqlConversationItemEvent, GraphqlConversationStarted,
        GraphqlLocalServiceStatus, GraphqlLocalStatus, GraphqlMemoryStorageStatus,
        GraphqlOnboardingStatus, GraphqlProviderAuthAttempt, GraphqlSendConversationTurnInput,
        GraphqlStartProviderAuthAttemptInput, GraphqlTurnAccepted, GraphqlTurnCompletedEvent,
    },
};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Shared state available to GraphQL resolvers.
#[derive(Clone)]
pub struct GraphqlState {
    web_state: Option<crate::daemon::web::WebState>,
    subscriptions: ConversationSubscriptionRegistry,
    memory_storage_ready: bool,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            web_state: None,
            subscriptions: ConversationSubscriptionRegistry::default(),
            memory_storage_ready: true,
        }
    }

    /// Build state backed by the local web server runtime.
    #[must_use]
    pub(crate) fn from_web_state(web_state: crate::daemon::web::WebState) -> Self {
        Self {
            subscriptions: web_state.subscriptions().clone(),
            web_state: Some(web_state),
            memory_storage_ready: true,
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
        let memory_storage = if state.memory_storage_ready {
            GraphqlMemoryStorageStatus::Ready
        } else {
            GraphqlMemoryStorageStatus::Initializing
        };

        GraphqlLocalStatus {
            local_service: GraphqlLocalServiceStatus::Running,
            assistant_connection: GraphqlAssistantConnection::Codex,
            memory_storage,
        }
    }

    /// Return onboarding status.
    async fn onboarding_status(&self, ctx: &Context<'_>) -> Result<GraphqlOnboardingStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let web = state.web_state()?;
        let account = web
            .memory_repository()
            .active_provider_account("codex")
            .await
            .map_err(graphql_error)?;
        let account = crate::daemon::web::reconcile_onboarding_provider_account(
            web.memory_repository(),
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
            crate::daemon::web::persist_provider_account_status_from_attempt(
                web.memory_repository(),
                attempt,
            )
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
        let request = crate::frontend_protocol::StartProviderAuthAttemptRequest {
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
            .memory_repository()
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
        let replay_records = crate::daemon::web::visible_conversation_replay(
            web.memory_repository(),
            &started.conversation_id,
        )
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
                        if result.is_ok() {
                            subscriptions.publish(ConversationLiveEvent::Completed {
                                conversation_id,
                                client_message_id: published_client_message_id,
                            });
                        }
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
                                item,
                            } => {
                        yield GraphqlConversationEvent::ConversationItem(
                            Box::new(GraphqlConversationItemEvent {
                                conversation_id,
                                client_message_id,
                                item_id,
                                turn_id,
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
