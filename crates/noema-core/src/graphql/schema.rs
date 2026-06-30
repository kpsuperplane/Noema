use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;

use super::{
    ConversationSubscriptionRegistry, GraphqlRuntimeState,
    chat::{
        self, GraphqlConversationEvent, GraphqlConversationStarted,
        GraphqlSendConversationTurnInput, GraphqlTurnAccepted,
    },
    local_status::{self, GraphqlLocalStatus, GraphqlMemoryStorageStatus},
    memory::{
        self, GraphqlMemoryClaim, GraphqlMemoryClaimDetail, GraphqlMemoryGraph,
        GraphqlMemoryGraphInput, GraphqlPredicateProposal,
    },
    onboarding::{
        self, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
        GraphqlStartProviderAuthAttemptInput,
    },
    provider_accounts::{self, GraphqlProviderAccount},
};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Shared state available to GraphQL resolvers.
#[derive(Clone)]
pub struct GraphqlState {
    runtime_state: GraphqlRuntimeState,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests(),
        }
    }

    /// Build test state backed by a real embedded store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: crate::NoemaStore) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store(store),
        }
    }

    /// Build state backed by the shared Noema runtime host.
    #[must_use]
    pub fn from_runtime_host(host: &crate::NoemaRuntimeHost) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::from_host(host),
        }
    }

    pub(crate) fn runtime(&self) -> Result<&crate::daemon::CodexRuntimeHandle> {
        self.runtime_state.runtime()
    }

    pub(crate) fn store(&self) -> Result<&crate::NoemaStore> {
        self.runtime_state.store()
    }

    pub(crate) fn optional_store(&self) -> Option<&crate::NoemaStore> {
        self.runtime_state.optional_store()
    }

    pub(crate) fn provider_auth(&self) -> Result<&crate::provider::auth::ProviderAuthManager> {
        self.runtime_state.provider_auth()
    }

    pub(crate) fn paths(&self) -> Result<&crate::NoemaPaths> {
        self.runtime_state.paths()
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        self.runtime_state.subscriptions()
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.runtime_state.memory_storage()
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
    async fn local_status(&self, ctx: &Context<'_>) -> Result<GraphqlLocalStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_status::local_status(state).await
    }

    /// Return onboarding status.
    async fn onboarding_status(&self, ctx: &Context<'_>) -> Result<GraphqlOnboardingStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::onboarding_status(state).await
    }

    /// Return a short-lived provider auth attempt.
    async fn provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlProviderAuthAttempt>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::provider_auth_attempt(state, attempt_id).await
    }

    /// List provider account metadata safe to show in Settings.
    async fn provider_accounts(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlProviderAccount>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::provider_accounts(state).await
    }

    /// List graph-memory claims for memory-management inspection.
    async fn memory_claims(
        &self,
        ctx: &Context<'_>,
        query: Option<String>,
        status: Option<String>,
        predicate_id: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<GraphqlMemoryClaim>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_claims(state, query, status, predicate_id, limit).await
    }

    /// Return one graph-memory claim for memory-management inspection.
    async fn memory_claim(
        &self,
        ctx: &Context<'_>,
        claim_id: String,
    ) -> Result<Option<GraphqlMemoryClaimDetail>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_claim(state, claim_id).await
    }

    /// List predicate proposals for memory-management inspection.
    async fn memory_predicate_proposals(
        &self,
        ctx: &Context<'_>,
        status: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<GraphqlPredicateProposal>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_predicate_proposals(state, status, limit).await
    }

    /// Return one predicate proposal for memory-management inspection.
    async fn memory_predicate_proposal(
        &self,
        ctx: &Context<'_>,
        proposal_id: String,
    ) -> Result<Option<GraphqlPredicateProposal>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_predicate_proposal(state, proposal_id).await
    }

    /// Return a bounded graph-memory projection for local memory-management inspection.
    async fn memory_graph(
        &self,
        ctx: &Context<'_>,
        input: Option<GraphqlMemoryGraphInput>,
    ) -> Result<GraphqlMemoryGraph> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_graph(state, input).await
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
        onboarding::start_provider_auth_attempt(state, input).await
    }

    /// Start or resume the primary conversation.
    async fn start_primary_conversation(
        &self,
        ctx: &Context<'_>,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<GraphqlConversationStarted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::start_primary_conversation(state, model, cwd).await
    }

    /// Send a conversation turn.
    async fn send_conversation_turn(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendConversationTurnInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::send_conversation_turn(state, input).await
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
        chat::conversation_events(state.subscriptions().clone(), conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{daemon::TurnStreamEvent, graphql::subscriptions::ConversationLiveEvent};
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
        assert!(sdl.contains("memoryPredicateProposals"));
        assert!(sdl.contains("memoryPredicateProposal"));
        assert!(sdl.contains("memoryGraph"));
        assert!(sdl.contains("providerAccounts"));
        assert!(sdl.contains("type GraphqlProviderAccount"));
        assert!(sdl.contains("type GraphqlMemoryClaim"));
        assert!(sdl.contains("type GraphqlMemoryClaimEvidence"));
        assert!(sdl.contains("type GraphqlPredicateProposal"));
        assert!(sdl.contains("GraphqlMemoryGraph"));
        assert!(sdl.contains("GraphqlMemoryGraphInput"));
    }

    #[test]
    fn graphql_state_for_tests_has_runtime_state_accessors() {
        let state = GraphqlState::for_tests();
        assert!(state.optional_store().is_none());
        assert_eq!(
            state.memory_storage(),
            crate::graphql::local_status::GraphqlMemoryStorageStatus::Ready
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_returns_safe_metadata() {
        use crate::{ProviderAccountStatus, store::tests::test_store};

        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                ProviderAccountStatus::Authenticated,
                Some("codex_ok"),
                Some("Codex credentials are usable"),
            )
            .await
            .expect("status update");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    isActive
                    isDefault
                    lastCheckedAt
                    lastAuthenticatedAt
                    lastErrorCode
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let account = &data["providerAccounts"][0];
        assert_eq!(account["providerKind"], "codex");
        assert_eq!(account["accountKey"], "default");
        assert_eq!(account["displayName"], "Codex");
        assert_eq!(account["authMethod"], "oauth_device_code");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert_eq!(account["isActive"], true);
        assert_eq!(account["isDefault"], true);
        assert_eq!(account["lastErrorCode"], "codex_ok");
        assert_eq!(
            account["lastErrorMessage"],
            "Codex credentials are usable"
        );

        let json_text = serde_json::to_string(&data).expect("provider json");
        assert!(!json_text.contains("provider_account:codex:default"));
        assert!(!json_text.contains("auth.json"));
        assert!(!json_text.contains("codex_tokens.json"));
        assert!(!json_text.contains("api_key"));
        assert!(!json_text.contains("token"));
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
    async fn predicate_proposal_query_returns_seeded_candidate() {
        use crate::{
            ActorRef, ConversationItemKind, ConversationItemStatus, NewConversation,
            NewConversationItem, NewConversationTurn, PredicateProposalCandidate,
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
                content_text: Some("Kevin collects model trains.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("source item");
        let proposal = store
            .create_predicate_proposal(PredicateProposalCandidate {
                label: "collects".to_string(),
                description: "The subject collects the object.".to_string(),
                proposed_predicate: json!({
                    "label": "collects",
                    "allowed_use_modes": ["answer", "personalize"],
                }),
                source_item_id: Some(item.item_id.clone()),
                proposed_claim: json!({
                    "fact": "Kevin collects model trains.",
                    "subject": "human:local",
                    "object": "concept:model_trains",
                }),
            })
            .await
            .expect("proposal");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  memoryPredicateProposal(proposalId: "{}") {{
                    proposalId
                    label
                    description
                    status
                    sourceItemId
                    proposedPredicate
                    proposedClaim
                  }}
                  memoryPredicateProposals(status: "candidate", limit: 5) {{
                    proposalId
                    label
                    status
                    sourceItemId
                    proposedPredicate
                    proposedClaim
                  }}
                }}
                "#,
                proposal.proposal_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["memoryPredicateProposal"]["proposalId"],
            proposal.proposal_id
        );
        assert_eq!(data["memoryPredicateProposal"]["label"], "collects");
        assert_eq!(
            data["memoryPredicateProposal"]["description"],
            "The subject collects the object."
        );
        assert_eq!(data["memoryPredicateProposal"]["status"], "candidate");
        assert_eq!(
            data["memoryPredicateProposal"]["sourceItemId"],
            item.item_id
        );
        assert_eq!(
            data["memoryPredicateProposal"]["proposedPredicate"]["label"],
            "collects"
        );
        assert_eq!(
            data["memoryPredicateProposal"]["proposedClaim"]["fact"],
            "Kevin collects model trains."
        );
        assert_eq!(
            data["memoryPredicateProposals"][0]["proposalId"],
            proposal.proposal_id
        );
        assert_eq!(data["memoryPredicateProposals"][0]["label"], "collects");
    }

    #[tokio::test]
    async fn memory_predicate_proposals_rejects_invalid_limit() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                { memoryPredicateProposals(limit: 0) { proposalId } }
                "#,
            ))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0]
                .message
                .contains("memoryPredicateProposals limit must be at least 1")
        );
    }

    #[tokio::test]
    async fn memory_graph_query_returns_readable_nodes_edges_and_summary() {
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
                    nodes { nodeId entityId label redacted claimCount }
                    edges { claimId sourceNodeId targetNodeId fact factRedacted predicateLabel sensitivity }
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
            "Garage code is 1234."
        );
        assert_eq!(data["memoryGraph"]["edges"][0]["factRedacted"], false);
        assert_eq!(data["memoryGraph"]["nodes"][0]["redacted"], false);

        let graph_json = serde_json::to_string(&data["memoryGraph"]).expect("graph json");
        assert!(!graph_json.contains("garage-code"), "{graph_json}");
        assert!(!graph_json.contains("garage_code"), "{graph_json}");
        assert!(graph_json.contains("Garage code"), "{graph_json}");
        assert!(graph_json.contains("1234"), "{graph_json}");

        let nodes = data["memoryGraph"]["nodes"].as_array().expect("nodes");
        let edge = &data["memoryGraph"]["edges"][0];
        assert!(
            nodes
                .iter()
                .any(|node| node["nodeId"] == edge["sourceNodeId"])
        );
        assert!(
            nodes
                .iter()
                .any(|node| node["nodeId"] == edge["targetNodeId"])
        );
        for node in nodes {
            assert!(
                node["nodeId"]
                    .as_str()
                    .expect("node id")
                    .starts_with("memory-node:")
            );
            assert_eq!(node["entityId"], node["nodeId"]);
        }
    }

    #[tokio::test]
    async fn memory_graph_query_uses_opaque_ids_for_public_nodes() {
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
        store
            .create_or_reinforce_claim(NewClaimCandidate {
                subject: EntityCandidate::local_human(),
                object: EntityCandidate::concept("trains", "trains"),
                predicate_id: "likes".to_string(),
                fact: "Kevin likes trains.".to_string(),
                sensitivity: Sensitivity::Public,
                status: ClaimStatus::Confirmed,
                confidence: Some(0.9),
                evidence: EvidenceCandidate {
                    source_item_id: item.item_id,
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
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { statuses: ["confirmed"], limit: 150 }) {
                    nodes { nodeId entityId label redacted }
                    edges { sourceNodeId targetNodeId fact factRedacted }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let graph_json = serde_json::to_string(&data["memoryGraph"]).expect("graph json");
        for raw_id in [
            "human:local",
            "concept:trains",
            "entity:human:local",
            "entity:concept:trains",
        ] {
            assert!(!graph_json.contains(raw_id), "{graph_json}");
        }

        let nodes = data["memoryGraph"]["nodes"].as_array().expect("nodes");
        let edge = &data["memoryGraph"]["edges"][0];
        assert!(
            nodes
                .iter()
                .any(|node| node["nodeId"] == edge["sourceNodeId"])
        );
        assert!(
            nodes
                .iter()
                .any(|node| node["nodeId"] == edge["targetNodeId"])
        );
        for node in nodes {
            assert!(
                node["nodeId"]
                    .as_str()
                    .expect("node id")
                    .starts_with("memory-node:")
            );
            assert_eq!(node["entityId"], node["nodeId"]);
        }
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

        let high_limit = schema
            .execute(async_graphql::Request::new(
                r#"
                { memoryGraph(input: { limit: 501 }) { summary { limit } } }
                "#,
            ))
            .await;
        assert_eq!(high_limit.errors.len(), 1);
        assert!(
            high_limit.errors[0]
                .message
                .contains("memoryGraph limit must be at most 500")
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

        let bad_sensitivity = schema
            .execute(async_graphql::Request::new(
                r#"
                { memoryGraph(input: { sensitivity: "spicy" }) { summary { limit } } }
                "#,
            ))
            .await;
        assert_eq!(bad_sensitivity.errors.len(), 1);
        assert!(
            bad_sensitivity.errors[0]
                .message
                .contains("unknown memory sensitivity: spicy")
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
