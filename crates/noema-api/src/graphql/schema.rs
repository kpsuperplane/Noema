#[cfg(test)]
use crate::test_support::TestEnvironment;
use async_graphql::{Context, Object, Result, Schema};
#[cfg(test)]
use noema_runtime::RuntimeEventRegistry;
use noema_tasks::RecurrenceCommandKind;

mod mutation;
mod query;
mod subscription;

use mutation::MutationRoot;
use query::QueryRoot;
use subscription::SubscriptionRoot;

pub use super::GraphqlState;

use super::{
    adapters::{
        self, GraphqlAdapterAuthorizationGrant, GraphqlAdapterDefinition,
        GraphqlAdapterOauthApplication, GraphqlAdapterOauthAttemptEvent,
        GraphqlAdapterOauthSetupAttempt, GraphqlAdapterOauthState,
        GraphqlApproveAdapterDefinitionInput, GraphqlAttachAdapterOauthConnectionInput,
        GraphqlCancelAdapterDefinitionInput, GraphqlDeleteAdapterConnectionInput,
        GraphqlDeleteAdapterOauthApplicationInput, GraphqlDeleteAdapterServiceInput,
        GraphqlDisconnectAdapterOauthGrantInput, GraphqlImportAdapterOauthApplicationInput,
        GraphqlReplaceAdapterOauthApplicationInput, GraphqlSaveAdapterOauthGrantLabelInput,
        GraphqlSetAdapterConnectionActiveInput, GraphqlSetupAdapterConnectionInput,
        GraphqlStartAdapterOauthSetupInput,
    },
    agents::{
        self, GraphqlAcpAgent, GraphqlAgent, GraphqlAgentModelPreference,
        GraphqlAuthenticateAcpAgentInput, GraphqlCreateAcpAgentInput, GraphqlDeleteAcpAgentInput,
        GraphqlSaveAgentModelPreferenceInput, GraphqlTestAcpAgentInput, GraphqlUpdateAcpAgentInput,
    },
    apns::{
        GraphqlApnsProviderStatus, GraphqlClientLiveActivityStatus,
        GraphqlClientNotificationPresenceEvent, GraphqlClientNotificationStatus,
        GraphqlConfigureApnsProviderInput, GraphqlRegisterClientLiveActivitiesInput,
        GraphqlRegisterClientLiveActivityUpdateInput, GraphqlRegisterClientNotificationsInput,
    },
    artifacts::{self, GraphqlCreateConversationExternalArtifactInput},
    capability_integrations::{
        self, GraphqlCapabilityConnection, GraphqlCapabilityConnectionRefInput,
        GraphqlCapabilityIntegration, GraphqlCapabilityIntegrationKind,
        GraphqlCapabilityManagedTool, GraphqlResetCapabilityToolPolicyInput,
        GraphqlSaveCapabilityConnectionLabelInput, GraphqlSaveCapabilityConnectionPolicyInput,
        GraphqlSaveCapabilityToolOverrideInput, GraphqlSetCapabilityToolEnabledInput,
    },
    chat::{
        self, GraphqlConversationEvent, GraphqlConversationTranscriptPage,
        GraphqlConversationTranscriptPageInput, GraphqlPrimaryConversation,
        GraphqlSendA2UIActionInput, GraphqlSendConversationTurnInput,
        GraphqlSendMultipleChoiceSelectionInput, GraphqlTurnAccepted,
    },
    clients::{self, GraphqlClient},
    governed_actions::{self, GraphqlGovernedAction, GraphqlResolveGovernedActionInput},
    human_interventions::{
        self, GraphqlAdapterAuthenticationIntervention, GraphqlHumanIntervention,
        GraphqlMcpAuthenticationIntervention, GraphqlResolveMcpSetupInterventionInput,
        GraphqlSkipAdapterAuthenticationInput, GraphqlSkipMcpAuthenticationInput,
        GraphqlStartAdapterAuthenticationInput, GraphqlStartMcpAuthenticationInput,
    },
    local_models::{
        self, GraphqlDefaultModelPreference, GraphqlImportLocalModelInput,
        GraphqlInstallLocalModelInput, GraphqlLocalModelCatalogEntry, GraphqlLocalModelEvent,
        GraphqlLocalModelInstallation, GraphqlLocalModelRuntimeStatus, GraphqlLocalModelSetup,
        GraphqlSaveDefaultModelPreferenceInput,
    },
    local_status::{self, GraphqlLocalStatus},
    mcp::{
        self, GraphqlAddMcpConnectionInput, GraphqlContinueMcpServerSetupInput,
        GraphqlCreateMcpServerInput, GraphqlMcpOAuthSetupAttempt, GraphqlMcpServer,
        GraphqlMcpServerSetupResult, GraphqlStartMcpServerOAuthSetupInput,
        GraphqlStartMcpServerReauthenticationOAuthSetupInput,
    },
    native_memory::{
        self, GraphqlNativeMemoryPage, GraphqlNativeMemorySearchResult,
        GraphqlNativeMemorySettings, GraphqlNativeMemoryTree, GraphqlNativeMemoryUpdateResult,
        GraphqlSaveMemoryModelPreferenceInput,
    },
    onboarding::{
        self, GraphqlCancelProviderAuthAttemptInput, GraphqlConfirmOnboardingModelSelectionsInput,
        GraphqlOnboardingModelSetup, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
        GraphqlStartProviderAuthAttemptInput,
    },
    privacy_settings::{self, GraphqlPrivacySettings, GraphqlSaveActionReviewerPreferenceInput},
    provider_accounts::{
        self, GraphqlCapabilityFeatures, GraphqlClearProviderSecretInput,
        GraphqlCreateProviderAccountInput, GraphqlDeleteProviderAccountInput,
        GraphqlProviderAccount, GraphqlProviderAccountCatalogEntry, GraphqlProviderCapability,
        GraphqlProviderSecretInput,
    },
    runtime_debug::{self, GraphqlRuntimeDebugProfile, GraphqlRuntimeDebugProfileInput},
    tasks::{
        self, GraphqlAnswerTaskInput, GraphqlArchiveProjectInput, GraphqlCancelTaskInput,
        GraphqlCaptureTaskInput, GraphqlCreateProjectInput, GraphqlProjectCommandPayload,
        GraphqlProjectConnection, GraphqlQueueTaskInput, GraphqlReopenProjectInput,
        GraphqlReopenTaskInput, GraphqlRetryTaskInput, GraphqlRunScheduledTaskNowInput,
        GraphqlScheduleTaskInput, GraphqlTaskAttentionConnection, GraphqlTaskCommandPayload,
        GraphqlTaskComplexity, GraphqlTaskConnection, GraphqlTaskDetail, GraphqlTaskEvent,
        GraphqlTaskEventConnection, GraphqlTaskExecutionPolicy, GraphqlTaskExecutionPolicyInput,
        GraphqlTaskListInput, GraphqlTaskModelPoolEntry, GraphqlTaskModelPoolEntryInput,
        GraphqlTaskOverview, GraphqlTaskRecurrence, GraphqlTaskRecurrenceCommandInput,
        GraphqlTaskRecurrenceSummary, GraphqlTaskRunItemConnection, GraphqlTaskSchedulePreview,
        GraphqlTaskSchedulePreviewInput, GraphqlTerminalTaskKind, GraphqlUnscheduleTaskInput,
        GraphqlUpdateInboxTaskInput, GraphqlUpdateProjectInput, GraphqlUpdateTaskRecurrenceInput,
    },
    usage_settings::{self, GraphqlSaveToolProgressAuditPreferenceInput, GraphqlUsageSettings},
    web_fetch_settings::{
        self, GraphqlSaveWebFetchSummarizerPreferenceInput, GraphqlWebFetchSettings,
    },
    web_push::{
        GraphqlRegisterWebPushSubscriptionInput, GraphqlWebPushPresenceEvent, GraphqlWebPushStatus,
    },
    web_tool_settings::{
        self, GraphqlSaveBrowserProviderRouteInput, GraphqlSaveWebToolProviderBindingInput,
        GraphqlWebToolBindingSettings, GraphqlWebToolSettings,
    },
};

const _: fn(GraphqlProviderCapability, GraphqlCapabilityFeatures) = |_, _| {};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Build the Noema GraphQL schema.
#[must_use]
pub fn build_schema(state: GraphqlState) -> GraphqlSchema {
    let builder = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).data(state);
    #[cfg(test)]
    let builder = builder.data(crate::graphql::RequestPrincipal::local());
    builder.finish()
}

#[cfg(test)]
fn build_schema_without_request_principal(state: GraphqlState) -> GraphqlSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state)
        .finish()
}

/// Render the transport-neutral GraphQL schema definition.
#[must_use]
pub fn schema_sdl() -> String {
    build_schema(GraphqlState::for_schema_definition()).sdl()
}

include!("schema_tests.rs");
