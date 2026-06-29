use async_graphql::{Enum, InputObject, Json, SimpleObject, Union};
use serde_json::Value;

use crate::{
    AgentStatus, ClaimStatus, MemoryClaimDetail, MemoryClaimEvidence, MemoryClaimRecord,
    MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryGraphSummary, OnboardingStatus,
    ProviderAccountStatus, ProviderAuthMethod, TurnActivityStatus, TurnTranscriptItem,
    memory::Sensitivity,
    provider_auth::{ProviderAuthAttemptStatus, ProviderAuthAttemptView},
};

/// Local service status shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlLocalServiceStatus {
    /// The local Noema service is running.
    Running,
}

/// Assistant connection exposed to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlAssistantConnection {
    /// The daemon is using Codex for chat.
    Codex,
}

/// Memory storage readiness shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlMemoryStorageStatus {
    /// The canonical memory store is ready.
    Ready,
    /// The canonical memory store is initializing.
    Initializing,
    /// Graph memory writes and retrieval are not available yet.
    Unavailable,
}

/// Local status returned by `Query.localStatus`.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlLocalStatus {
    /// Local service status.
    pub local_service: GraphqlLocalServiceStatus,
    /// Assistant connection status.
    pub assistant_connection: GraphqlAssistantConnection,
    /// Memory storage status.
    pub memory_storage: GraphqlMemoryStorageStatus,
}

/// Graph-memory claim exposed for owner/admin inspection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryClaim {
    /// Stable claim id.
    pub claim_id: String,
    /// Conservative fact preview. Non-public facts are redacted in list views.
    pub fact: String,
    /// Whether the fact was redacted at the GraphQL boundary.
    pub fact_redacted: bool,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

impl From<MemoryClaimRecord> for GraphqlMemoryClaim {
    fn from(claim: MemoryClaimRecord) -> Self {
        let fact_redacted = claim.sensitivity != Sensitivity::Public;
        Self {
            claim_id: claim.claim_id,
            fact: graphql_list_fact(&claim.fact, claim.sensitivity),
            fact_redacted,
            predicate_id: claim.predicate_id,
            predicate_label: claim.predicate_label,
            subject_entity_id: claim.subject_entity_id,
            subject_entity_name: graphql_list_display_name(
                claim.subject_entity_name,
                claim.sensitivity,
            ),
            subject_entity_type: claim.subject_entity_type,
            object_entity_id: claim.object_entity_id,
            object_entity_name: claim
                .object_entity_name
                .map(|name| graphql_list_display_name(name, claim.sensitivity)),
            object_entity_type: claim.object_entity_type,
            status: claim_status_label(claim.status).to_string(),
            sensitivity: sensitivity_label(claim.sensitivity).to_string(),
            confidence: claim.confidence,
            evidence_count: claim.evidence_count,
            created_at: claim.created_at,
            updated_at: claim.updated_at,
        }
    }
}

fn graphql_list_fact(fact: &str, sensitivity: Sensitivity) -> String {
    match sensitivity {
        Sensitivity::Public => fact.to_string(),
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memoryClaim(claimId) for detail]".to_string(),
    }
}

fn graphql_list_display_name(name: String, sensitivity: Sensitivity) -> String {
    match sensitivity {
        Sensitivity::Public => name,
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memoryClaim(claimId) for detail]".to_string(),
    }
}

fn claim_status_label(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Candidate => "candidate",
        ClaimStatus::Active => "active",
        ClaimStatus::Confirmed => "confirmed",
        ClaimStatus::Disputed => "disputed",
        ClaimStatus::Superseded => "superseded",
        ClaimStatus::Archived => "archived",
        ClaimStatus::Deleted => "deleted",
    }
}

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

/// Supporting evidence exposed for explicit owner/admin claim detail inspection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryClaimEvidence {
    /// Stable evidence relation id if available.
    pub evidence_id: Option<String>,
    /// Source conversation item id if available.
    pub source_item_id: Option<String>,
    /// Evidence authority.
    pub authority: String,
    /// Evidence excerpt when available.
    pub excerpt: Option<String>,
    /// Observation timestamp when available.
    pub observed_at: Option<String>,
    /// Evidence creation timestamp.
    pub created_at: String,
}

impl From<MemoryClaimEvidence> for GraphqlMemoryClaimEvidence {
    fn from(evidence: MemoryClaimEvidence) -> Self {
        Self {
            evidence_id: evidence.evidence_id,
            source_item_id: evidence.source_item_id,
            authority: evidence.authority,
            excerpt: evidence.excerpt,
            observed_at: evidence.observed_at,
            created_at: evidence.created_at,
        }
    }
}

/// Graph-memory claim detail exposed for owner/admin inspection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryClaimDetail {
    /// Stable claim id.
    pub claim_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Support evidence rows.
    pub evidence: Vec<GraphqlMemoryClaimEvidence>,
}

impl From<MemoryClaimDetail> for GraphqlMemoryClaimDetail {
    fn from(detail: MemoryClaimDetail) -> Self {
        let claim = detail.claim;
        Self {
            claim_id: claim.claim_id,
            fact: claim.fact,
            predicate_id: claim.predicate_id,
            predicate_label: claim.predicate_label,
            subject_entity_id: claim.subject_entity_id,
            subject_entity_name: claim.subject_entity_name,
            subject_entity_type: claim.subject_entity_type,
            object_entity_id: claim.object_entity_id,
            object_entity_name: claim.object_entity_name,
            object_entity_type: claim.object_entity_type,
            status: claim_status_label(claim.status).to_string(),
            sensitivity: sensitivity_label(claim.sensitivity).to_string(),
            confidence: claim.confidence,
            evidence_count: claim.evidence_count,
            created_at: claim.created_at,
            updated_at: claim.updated_at,
            evidence: detail.evidence.into_iter().map(Into::into).collect(),
        }
    }
}

/// Input filters for bounded graph-memory inspection.
#[derive(Clone, Debug, Default, InputObject)]
pub struct GraphqlMemoryGraphInput {
    /// Optional text query matched by the store read model.
    pub query: Option<String>,
    /// Optional claim lifecycle statuses. Defaults are owned by the store.
    pub statuses: Option<Vec<String>>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional exact sensitivity filter.
    pub sensitivity: Option<String>,
    /// Optional bounded result limit.
    pub limit: Option<i32>,
}

/// Bounded graph-memory projection for owner/admin inspection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraph {
    /// Entity nodes incident to returned claim edges.
    pub nodes: Vec<GraphqlMemoryGraphNode>,
    /// Claim edges connecting returned entity nodes.
    pub edges: Vec<GraphqlMemoryGraphEdge>,
    /// Summary metadata for the bounded result.
    pub summary: GraphqlMemoryGraphSummary,
}

/// Entity node in the graph-memory projection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphNode {
    /// Stable graph node id.
    pub node_id: String,
    /// Canonical entity id.
    pub entity_id: String,
    /// Human-readable entity label. Non-public labels are redacted in lists.
    pub label: String,
    /// Entity type string.
    pub entity_type: String,
    /// Whether this node label was redacted at the GraphQL boundary.
    pub redacted: bool,
    /// Count of returned incident claim edges.
    pub claim_count: i64,
}

/// Claim edge in the graph-memory projection.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphEdge {
    /// Stable claim id.
    pub claim_id: String,
    /// Source entity node id.
    pub source_node_id: String,
    /// Target entity node id.
    pub target_node_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Conservative fact preview. Non-public facts are redacted in graph lists.
    pub fact: String,
    /// Whether the fact was redacted at the GraphQL boundary.
    pub fact_redacted: bool,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Summary metadata for a bounded graph-memory result.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryGraphSummary {
    /// Number of claim edges returned.
    pub returned_claim_count: i64,
    /// Number of entity nodes returned.
    pub returned_node_count: i64,
    /// Effective result limit.
    pub limit: i32,
    /// Whether at least one matching claim was omitted by the limit.
    pub truncated: bool,
}

impl From<MemoryGraph> for GraphqlMemoryGraph {
    fn from(graph: MemoryGraph) -> Self {
        Self {
            nodes: graph.nodes.into_iter().map(Into::into).collect(),
            edges: graph.edges.into_iter().map(Into::into).collect(),
            summary: graph.summary.into(),
        }
    }
}

impl From<MemoryGraphNode> for GraphqlMemoryGraphNode {
    fn from(node: MemoryGraphNode) -> Self {
        let redacted = node.max_sensitivity != Sensitivity::Public;
        Self {
            node_id: node.node_id,
            entity_id: node.entity_id,
            label: graphql_list_display_name(node.label, node.max_sensitivity),
            entity_type: node.entity_type,
            redacted,
            claim_count: node.claim_count,
        }
    }
}

impl From<MemoryGraphEdge> for GraphqlMemoryGraphEdge {
    fn from(edge: MemoryGraphEdge) -> Self {
        let fact_redacted = edge.sensitivity != Sensitivity::Public;
        Self {
            claim_id: edge.claim_id,
            source_node_id: edge.source_node_id,
            target_node_id: edge.target_node_id,
            predicate_id: edge.predicate_id,
            predicate_label: edge.predicate_label,
            fact: graphql_list_fact(&edge.fact, edge.sensitivity),
            fact_redacted,
            status: claim_status_label(edge.status).to_string(),
            sensitivity: sensitivity_label(edge.sensitivity).to_string(),
            confidence: edge.confidence,
            evidence_count: edge.evidence_count,
            created_at: edge.created_at,
            updated_at: edge.updated_at,
        }
    }
}

impl From<MemoryGraphSummary> for GraphqlMemoryGraphSummary {
    fn from(summary: MemoryGraphSummary) -> Self {
        Self {
            returned_claim_count: summary.returned_claim_count,
            returned_node_count: summary.returned_node_count,
            limit: i32::try_from(summary.limit).unwrap_or(i32::MAX),
            truncated: summary.truncated,
        }
    }
}

/// Provider auth method exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAuthMethod {
    /// OAuth device-code flow.
    OauthDeviceCode,
    /// Secret input flow.
    SecretInput,
    /// External manual flow.
    ExternalManual,
    /// No auth required.
    None,
}

impl From<ProviderAuthMethod> for GraphqlProviderAuthMethod {
    fn from(method: ProviderAuthMethod) -> Self {
        match method {
            ProviderAuthMethod::OauthDeviceCode => Self::OauthDeviceCode,
            ProviderAuthMethod::SecretInput => Self::SecretInput,
            ProviderAuthMethod::ExternalManual => Self::ExternalManual,
            ProviderAuthMethod::None => Self::None,
        }
    }
}

impl From<GraphqlProviderAuthMethod> for ProviderAuthMethod {
    fn from(method: GraphqlProviderAuthMethod) -> Self {
        match method {
            GraphqlProviderAuthMethod::OauthDeviceCode => Self::OauthDeviceCode,
            GraphqlProviderAuthMethod::SecretInput => Self::SecretInput,
            GraphqlProviderAuthMethod::ExternalManual => Self::ExternalManual,
            GraphqlProviderAuthMethod::None => Self::None,
        }
    }
}

/// Provider account status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAccountStatus {
    /// Status has not been checked.
    Unknown,
    /// Status is being checked.
    Checking,
    /// Account is authenticated.
    Authenticated,
    /// Account is unauthenticated.
    Unauthenticated,
    /// Provider is unavailable.
    Unavailable,
}

impl From<ProviderAccountStatus> for GraphqlProviderAccountStatus {
    fn from(status: ProviderAccountStatus) -> Self {
        match status {
            ProviderAccountStatus::Unknown => Self::Unknown,
            ProviderAccountStatus::Checking => Self::Checking,
            ProviderAccountStatus::Authenticated => Self::Authenticated,
            ProviderAccountStatus::Unauthenticated => Self::Unauthenticated,
            ProviderAccountStatus::Unavailable => Self::Unavailable,
        }
    }
}

/// Onboarding step status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlOnboardingStepStatus {
    /// Step is complete.
    Complete,
    /// Step blocks the user from continuing.
    Blocked,
}

/// One onboarding step.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlOnboardingStep {
    /// Stable step id.
    pub id: String,
    /// Step status.
    pub status: GraphqlOnboardingStepStatus,
    /// Provider family when the step is provider-backed.
    pub provider_kind: Option<String>,
    /// Provider account id when the step is provider-backed.
    pub provider_account_id: Option<String>,
    /// Provider-local account key when the step is provider-backed.
    pub account_key: Option<String>,
    /// Human-readable account name when the step is provider-backed.
    pub display_name: Option<String>,
    /// Last known provider account status.
    pub provider_account_status: Option<GraphqlProviderAccountStatus>,
    /// Auth method when the step can start auth.
    pub auth_method: Option<GraphqlProviderAuthMethod>,
}

/// Onboarding status.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlOnboardingStatus {
    /// Whether chat can start.
    pub is_user_onboarded: bool,
    /// Ordered onboarding steps.
    pub steps: Vec<GraphqlOnboardingStep>,
}

impl From<OnboardingStatus> for GraphqlOnboardingStatus {
    fn from(status: OnboardingStatus) -> Self {
        Self {
            is_user_onboarded: status.is_user_onboarded,
            steps: status
                .steps
                .into_iter()
                .map(|step| GraphqlOnboardingStep {
                    id: step.id,
                    status: match step.status {
                        crate::OnboardingStepStatus::Complete => {
                            GraphqlOnboardingStepStatus::Complete
                        }
                        crate::OnboardingStepStatus::Blocked => {
                            GraphqlOnboardingStepStatus::Blocked
                        }
                    },
                    provider_kind: step.provider_kind,
                    provider_account_id: step.provider_account_id,
                    account_key: step.account_key,
                    display_name: step.display_name,
                    provider_account_status: step.provider_account_status.map(Into::into),
                    auth_method: step.auth_method.map(Into::into),
                })
                .collect(),
        }
    }
}

/// Input for starting a provider auth attempt.
#[derive(Clone, Debug, InputObject)]
pub struct GraphqlStartProviderAuthAttemptInput {
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Requested authentication method.
    pub method: GraphqlProviderAuthMethod,
}

/// Provider auth attempt status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAuthAttemptStatus {
    /// Attempt is starting.
    Starting,
    /// Waiting for the user.
    WaitingForUser,
    /// Attempt completed.
    Completed,
    /// Attempt failed.
    Failed,
    /// Attempt expired.
    Expired,
    /// Attempt was cancelled.
    Cancelled,
}

impl From<ProviderAuthAttemptStatus> for GraphqlProviderAuthAttemptStatus {
    fn from(status: ProviderAuthAttemptStatus) -> Self {
        match status {
            ProviderAuthAttemptStatus::Starting => Self::Starting,
            ProviderAuthAttemptStatus::WaitingForUser => Self::WaitingForUser,
            ProviderAuthAttemptStatus::Completed => Self::Completed,
            ProviderAuthAttemptStatus::Failed => Self::Failed,
            ProviderAuthAttemptStatus::Expired => Self::Expired,
            ProviderAuthAttemptStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Provider auth attempt view.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlProviderAuthAttempt {
    /// Short-lived auth attempt id.
    pub attempt_id: String,
    /// Provider family.
    pub provider_kind: String,
    /// Provider account id.
    pub provider_account_id: String,
    /// Provider account auth method.
    pub method: GraphqlProviderAuthMethod,
    /// Attempt status.
    pub status: GraphqlProviderAuthAttemptStatus,
    /// Verification URL when available.
    pub verification_url: Option<String>,
    /// User code when available.
    pub user_code: Option<String>,
    /// Static UI-safe instruction text.
    pub instructions: Option<String>,
    /// Stable non-secret error code.
    pub error_code: Option<String>,
    /// Failure message when available.
    pub error_message: Option<String>,
}

impl From<ProviderAuthAttemptView> for GraphqlProviderAuthAttempt {
    fn from(view: ProviderAuthAttemptView) -> Self {
        Self {
            attempt_id: view.attempt_id,
            provider_kind: view.provider_kind,
            provider_account_id: view.provider_account_id,
            method: view.method.into(),
            status: view.status.into(),
            verification_url: view.verification_url,
            user_code: view.user_code,
            instructions: view.instructions,
            error_code: view.error_code,
            error_message: view.error_message,
        }
    }
}

/// Agent status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
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
pub struct GraphqlUserText {
    /// Text authored by the user.
    pub text: String,
}

/// Assistant text transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAssistantText {
    /// Text to render as the assistant response.
    pub text: String,
}

/// Activity transcript item.
#[derive(Clone, Debug, SimpleObject)]
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
pub struct GraphqlErrorNotice {
    /// Human-readable error message.
    pub message: String,
    /// Whether the chat turn can continue.
    pub recoverable: bool,
}

/// Transcript item union.
#[derive(Clone, Debug, Union)]
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
pub struct GraphqlTurnAccepted {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Conversation item event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
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
pub struct GraphqlAgentStatusEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Current agent status.
    pub status: GraphqlAgentStatus,
}

/// Turn completion event delivered by subscription.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlTurnCompletedEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
    /// Frontend-generated id for optimistic UI correlation.
    pub client_message_id: Option<String>,
}

/// Subscription readiness event delivered before live turn events.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlSubscriptionReadyEvent {
    /// Durable Noema conversation id.
    pub conversation_id: String,
}

/// Conversation subscription event union.
#[derive(Clone, Debug, Union)]
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

#[cfg(test)]
mod tests {
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
}
