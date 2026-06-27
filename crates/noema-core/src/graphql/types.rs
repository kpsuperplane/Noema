use async_graphql::{Enum, InputObject, Json, SimpleObject, Union};
use serde_json::Value;

use crate::{
    AgentStatus, OnboardingStatus, ProviderAccountStatus, ProviderAuthMethod, TurnActivityStatus,
    TurnTranscriptItem,
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
    /// Transcript item to render.
    pub item: GraphqlTranscriptItem,
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
