use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;

use crate::daemon::TurnStreamEvent;

use super::{
    ConversationLiveEvent, ConversationSubscriptionRegistry,
    types::{
        GraphqlAgentStatusEvent, GraphqlAssistantConnection, GraphqlAssistantTextDeltaEvent,
        GraphqlConversationEvent, GraphqlConversationItem, GraphqlConversationItemEvent,
        GraphqlConversationStarted, GraphqlLocalServiceStatus, GraphqlLocalStatus,
        GraphqlMemoryClaim, GraphqlMemoryClaimDetail, GraphqlMemoryGraph, GraphqlMemoryGraphInput,
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
    #[cfg(test)]
    test_store: Option<crate::NoemaStore>,
    subscriptions: ConversationSubscriptionRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            web_state: None,
            #[cfg(test)]
            test_store: None,
            subscriptions: ConversationSubscriptionRegistry::default(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build test state backed by a real embedded store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: crate::NoemaStore) -> Self {
        Self {
            web_state: None,
            test_store: Some(store),
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
            #[cfg(test)]
            test_store: None,
            memory_storage: GraphqlMemoryStorageStatus::Ready,
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

    fn store(&self) -> Result<&crate::NoemaStore> {
        #[cfg(test)]
        if let Some(store) = &self.test_store {
            return Ok(store);
        }

        Ok(self.web_state()?.store())
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

    /// List graph-memory claims for owner/admin inspection.
    async fn memory_claims(
        &self,
        ctx: &Context<'_>,
        query: Option<String>,
        status: Option<String>,
        predicate_id: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<GraphqlMemoryClaim>> {
        let limit = match limit {
            Some(value) if value < 1 => {
                return Err(async_graphql::Error::new(
                    "memoryClaims limit must be at least 1",
                ));
            }
            Some(value) => Some(
                usize::try_from(value)
                    .map_err(|_| async_graphql::Error::new("memoryClaims limit is too large"))?,
            ),
            None => None,
        };
        let state = ctx.data_unchecked::<GraphqlState>();
        let status = status
            .as_deref()
            .map(parse_graphql_claim_status)
            .transpose()?;
        let claims = state
            .store()?
            .list_claims(crate::MemoryClaimFilter {
                query,
                status,
                predicate_id,
                limit,
            })
            .await
            .map_err(graphql_error)?;

        Ok(claims.into_iter().map(Into::into).collect())
    }

    /// Return one graph-memory claim for owner/admin inspection.
    async fn memory_claim(
        &self,
        ctx: &Context<'_>,
        claim_id: String,
    ) -> Result<Option<GraphqlMemoryClaimDetail>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let detail = state
            .store()?
            .get_claim_detail(&claim_id)
            .await
            .map_err(graphql_error)?;
        Ok(detail.map(Into::into))
    }

    /// Return a bounded graph-memory projection for owner/admin inspection.
    async fn memory_graph(
        &self,
        ctx: &Context<'_>,
        input: Option<GraphqlMemoryGraphInput>,
    ) -> Result<GraphqlMemoryGraph> {
        let input = input.unwrap_or_default();
        let limit = parse_memory_graph_limit(input.limit)?;
        let statuses = input
            .statuses
            .map(|statuses| {
                if statuses.is_empty() {
                    return Err(async_graphql::Error::new(
                        "memoryGraph statuses must not be empty",
                    ));
                }
                statuses
                    .iter()
                    .map(|status| parse_graphql_claim_status(status))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let sensitivity = input
            .sensitivity
            .as_deref()
            .map(parse_graphql_sensitivity)
            .transpose()?;
        let state = ctx.data_unchecked::<GraphqlState>();
        let graph = state
            .store()?
            .memory_graph(crate::MemoryGraphFilter {
                query: input.query,
                statuses,
                predicate_id: input.predicate_id,
                sensitivity,
                limit,
            })
            .await
            .map_err(graphql_error)?;

        Ok(graph.into())
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

fn parse_graphql_claim_status(value: &str) -> Result<crate::ClaimStatus> {
    match value {
        "candidate" => Ok(crate::ClaimStatus::Candidate),
        "active" => Ok(crate::ClaimStatus::Active),
        "confirmed" => Ok(crate::ClaimStatus::Confirmed),
        "disputed" => Ok(crate::ClaimStatus::Disputed),
        "superseded" => Ok(crate::ClaimStatus::Superseded),
        "archived" => Ok(crate::ClaimStatus::Archived),
        "deleted" => Ok(crate::ClaimStatus::Deleted),
        _ => Err(async_graphql::Error::new(format!(
            "unknown memory claim status: {value}"
        ))),
    }
}

fn parse_graphql_sensitivity(value: &str) -> Result<crate::memory::Sensitivity> {
    match value {
        "public" => Ok(crate::memory::Sensitivity::Public),
        "normal" => Ok(crate::memory::Sensitivity::Normal),
        "private" => Ok(crate::memory::Sensitivity::Private),
        "sensitive" => Ok(crate::memory::Sensitivity::Sensitive),
        "secret" => Ok(crate::memory::Sensitivity::Secret),
        _ => Err(async_graphql::Error::new(format!(
            "unknown memory sensitivity: {value}"
        ))),
    }
}

fn parse_memory_graph_limit(limit: Option<i32>) -> Result<Option<usize>> {
    match limit {
        Some(value) if value < 1 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at least 1",
        )),
        Some(value) if value > 500 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at most 500",
        )),
        Some(value) => Ok(Some(usize::try_from(value).map_err(|_| {
            async_graphql::Error::new("memoryGraph limit is too large")
        })?)),
        None => Ok(None),
    }
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
        assert!(sdl.contains("memoryClaims"));
        assert!(sdl.contains("memoryClaim"));
        assert!(sdl.contains("memoryGraph"));
        assert!(sdl.contains("type GraphqlMemoryClaim"));
        assert!(sdl.contains("type GraphqlMemoryClaimEvidence"));
        assert!(sdl.contains("GraphqlMemoryGraph"));
        assert!(sdl.contains("GraphqlMemoryGraphInput"));
    }

    #[tokio::test]
    async fn memory_claim_query_returns_seeded_detail() {
        use crate::{
            ActorRef, ClaimStatus, ConversationItemKind, ConversationItemStatus, EntityCandidate,
            EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversation,
            NewConversationItem, NewConversationTurn, memory::Sensitivity,
            store::tests::test_store,
        };

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let item = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: Some(turn.turn_id),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some("Kevin likes trains.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("source item");
        let summary = store
            .create_or_reinforce_claim(NewClaimCandidate {
                subject: EntityCandidate::local_human(),
                object: EntityCandidate::concept("trains", "trains"),
                predicate_id: "likes".to_string(),
                fact: "Kevin likes trains.".to_string(),
                sensitivity: Sensitivity::Normal,
                status: ClaimStatus::Confirmed,
                confidence: Some(0.9),
                evidence: EvidenceCandidate {
                    source_item_id: item.item_id.clone(),
                    authority: EvidenceAuthority::ExplicitHumanStatement,
                    excerpt: Some("Kevin likes trains.".to_string()),
                },
                retrieval_hints: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("claim");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  memoryClaim(claimId: "{}") {{
                    claimId
                    fact
                    predicateLabel
                    subjectEntityName
                    objectEntityName
                    evidence {{ sourceItemId authority excerpt }}
                  }}
                }}
                "#,
                summary.claim_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["memoryClaim"]["claimId"], summary.claim_id);
        assert_eq!(data["memoryClaim"]["predicateLabel"], "likes");
        assert_eq!(data["memoryClaim"]["subjectEntityName"], "Local human");
        assert_eq!(data["memoryClaim"]["objectEntityName"], "trains");
        assert_eq!(
            data["memoryClaim"]["evidence"][0]["sourceItemId"],
            item.item_id
        );
        assert_eq!(
            data["memoryClaim"]["evidence"][0]["authority"],
            "explicit_human_statement"
        );
    }

    #[tokio::test]
    async fn memory_claims_list_redacts_non_public_facts() {
        use crate::{
            ActorRef, ClaimStatus, ConversationItemKind, ConversationItemStatus, EntityCandidate,
            EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversation,
            NewConversationItem, NewConversationTurn, memory::Sensitivity,
            store::tests::test_store,
        };

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let item = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: Some(turn.turn_id),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some("Garage code is 1234.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("source item");
        let private_note = "Garage code is 1234.";
        let summary = store
            .create_or_reinforce_claim(NewClaimCandidate {
                subject: EntityCandidate::local_human(),
                object: EntityCandidate::concept(private_note, private_note),
                predicate_id: "has_note".to_string(),
                fact: private_note.to_string(),
                sensitivity: Sensitivity::Private,
                status: ClaimStatus::Confirmed,
                confidence: Some(0.9),
                evidence: EvidenceCandidate {
                    source_item_id: item.item_id,
                    authority: EvidenceAuthority::ExplicitHumanStatement,
                    excerpt: Some(private_note.to_string()),
                },
                retrieval_hints: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("claim");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let list_response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryClaims(limit: 10) {
                    claimId
                    fact
                    factRedacted
                    subjectEntityName
                    objectEntityName
                    sensitivity
                  }
                }
                "#,
            ))
            .await;

        assert!(
            list_response.errors.is_empty(),
            "{:?}",
            list_response.errors
        );
        let data = list_response.data.into_json().expect("json");
        assert_eq!(data["memoryClaims"][0]["claimId"], summary.claim_id);
        assert_eq!(
            data["memoryClaims"][0]["fact"],
            "[redacted; use memoryClaim(claimId) for detail]"
        );
        assert_eq!(
            data["memoryClaims"][0]["subjectEntityName"],
            "[redacted; use memoryClaim(claimId) for detail]"
        );
        assert_eq!(
            data["memoryClaims"][0]["objectEntityName"],
            "[redacted; use memoryClaim(claimId) for detail]"
        );
        assert_eq!(data["memoryClaims"][0]["factRedacted"], true);
        assert_eq!(data["memoryClaims"][0]["sensitivity"], "private");

        let detail_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  memoryClaim(claimId: "{}") {{
                    fact
                    subjectEntityName
                    objectEntityName
                  }}
                }}
                "#,
                summary.claim_id
            )))
            .await;

        assert!(
            detail_response.errors.is_empty(),
            "{:?}",
            detail_response.errors
        );
        let data = detail_response.data.into_json().expect("json");
        assert_eq!(data["memoryClaim"]["fact"], private_note);
        assert_eq!(data["memoryClaim"]["subjectEntityName"], "Local human");
        assert_eq!(data["memoryClaim"]["objectEntityName"], private_note);
    }

    #[tokio::test]
    async fn memory_graph_query_returns_redacted_nodes_edges_and_summary() {
        use crate::{
            ActorRef, ClaimStatus, ConversationItemKind, ConversationItemStatus, EntityCandidate,
            EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversation,
            NewConversationItem, NewConversationTurn, memory::Sensitivity,
            store::tests::test_store,
        };

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let item = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: Some(turn.turn_id),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some("Garage code is 1234.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("source item");
        let summary = store
            .create_or_reinforce_claim(NewClaimCandidate {
                subject: EntityCandidate::local_human(),
                object: EntityCandidate::concept("garage-code", "Garage code"),
                predicate_id: "has_note".to_string(),
                fact: "Garage code is 1234.".to_string(),
                sensitivity: Sensitivity::Private,
                status: ClaimStatus::Confirmed,
                confidence: Some(0.9),
                evidence: EvidenceCandidate {
                    source_item_id: item.item_id,
                    authority: EvidenceAuthority::ExplicitHumanStatement,
                    excerpt: Some("Garage code is 1234.".to_string()),
                },
                retrieval_hints: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("claim");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { statuses: ["confirmed"], limit: 150 }) {
                    nodes { nodeId label redacted claimCount }
                    edges { claimId fact factRedacted predicateLabel sensitivity }
                    summary { returnedClaimCount returnedNodeCount limit truncated }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["memoryGraph"]["summary"]["returnedClaimCount"], 1);
        assert_eq!(data["memoryGraph"]["summary"]["limit"], 150);
        assert_eq!(data["memoryGraph"]["summary"]["truncated"], false);
        assert_eq!(data["memoryGraph"]["edges"][0]["claimId"], summary.claim_id);
        assert_eq!(
            data["memoryGraph"]["edges"][0]["fact"],
            "[redacted; use memoryClaim(claimId) for detail]"
        );
        assert_eq!(data["memoryGraph"]["edges"][0]["factRedacted"], true);
        assert_eq!(data["memoryGraph"]["nodes"][0]["redacted"], true);
    }

    #[tokio::test]
    async fn memory_graph_rejects_invalid_limit_and_status() {
        let schema = build_schema(GraphqlState::for_tests());
        let low_limit = schema
            .execute(async_graphql::Request::new(
                r#"
                { memoryGraph(input: { limit: 0 }) { summary { limit } } }
                "#,
            ))
            .await;
        assert_eq!(low_limit.errors.len(), 1);
        assert!(
            low_limit.errors[0]
                .message
                .contains("memoryGraph limit must be at least 1")
        );

        let bad_status = schema
            .execute(async_graphql::Request::new(
                r#"
                { memoryGraph(input: { statuses: ["sleepy"] }) { summary { limit } } }
                "#,
            ))
            .await;
        assert_eq!(bad_status.errors.len(), 1);
        assert!(
            bad_status.errors[0]
                .message
                .contains("unknown memory claim status: sleepy")
        );
    }

    #[tokio::test]
    async fn memory_claims_rejects_negative_limit() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryClaims(limit: -1) {
                    claimId
                  }
                }
                "#,
            ))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0]
                .message
                .contains("memoryClaims limit must be at least 1"),
            "{:?}",
            response.errors
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
