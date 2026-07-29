#[cfg(test)]
use crate::test_support::TestEnvironment;
use async_graphql::{Context, Object, Result, Schema};
#[cfg(test)]
use noema_runtime::RuntimeEventRegistry;

mod mutation;
mod query;
mod subscription;

use mutation::MutationRoot;
use query::QueryRoot;
use subscription::SubscriptionRoot;

pub use super::GraphqlState;

use super::{
    adapters::{
        self, GraphqlAdapterDefinition, GraphqlAdapterOauthSetupAttempt,
        GraphqlApproveAdapterDefinitionInput, GraphqlDeleteAdapterConnectionInput,
        GraphqlDeleteAdapterServiceInput, GraphqlImportAdapterOauthClientJsonInput,
        GraphqlStartAdapterOauthSetupInput,
    },
    agents::{
        self, GraphqlAgent, GraphqlAgentModelPreference, GraphqlSaveAgentModelPreferenceInput,
    },
    artifacts::{self, GraphqlCreateConversationExternalArtifactInput},
    capability_integrations::{
        self, GraphqlCapabilityConnection, GraphqlCapabilityConnectionRefInput,
        GraphqlCapabilityIntegration, GraphqlCapabilityIntegrationKind,
        GraphqlCapabilityManagedTool, GraphqlResetCapabilityToolPolicyInput,
        GraphqlSaveCapabilityConnectionPolicyInput, GraphqlSaveCapabilityToolOverrideInput,
        GraphqlSetCapabilityToolEnabledInput,
    },
    chat::{
        self, GraphqlConversationEvent, GraphqlConversationTranscriptPage,
        GraphqlConversationTranscriptPageInput, GraphqlPrimaryConversation,
        GraphqlSendConversationTurnInput, GraphqlSendMultipleChoiceSelectionInput,
        GraphqlTurnAccepted,
    },
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
        self, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
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
        GraphqlReopenTaskInput, GraphqlRetryTaskInput, GraphqlTaskAttentionConnection,
        GraphqlTaskCommandPayload, GraphqlTaskComplexity, GraphqlTaskConnection, GraphqlTaskDetail,
        GraphqlTaskExecutionPolicy, GraphqlTaskExecutionPolicyInput, GraphqlTaskModelPoolEntry,
        GraphqlTaskModelPoolEntryInput, GraphqlTaskRunItemConnection, GraphqlTerminalTaskKind,
        GraphqlUpdateInboxTaskInput, GraphqlUpdateProjectInput, GraphqlWorkEvent,
        GraphqlWorkEventConnection, GraphqlWorkOverview, GraphqlWorkTasksInput,
    },
    usage_settings::{self, GraphqlSaveToolProgressAuditPreferenceInput, GraphqlUsageSettings},
    web_fetch_settings::{
        self, GraphqlSaveWebFetchSummarizerPreferenceInput, GraphqlWebFetchSettings,
    },
    web_tool_settings::{
        self, GraphqlSaveWebToolProviderBindingInput, GraphqlWebToolBindingSettings,
        GraphqlWebToolSettings,
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
