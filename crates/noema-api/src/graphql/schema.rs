#[cfg(test)]
use crate::test_support::TestEnvironment;
use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use noema_runtime::{RuntimeEventRegistry, TaskRuntimeEvent};

mod mutation;
mod query;
mod subscription;

use mutation::MutationRoot;
use query::QueryRoot;
use subscription::SubscriptionRoot;

use super::{
    GraphqlRuntimeState,
    agents::{
        self, GraphqlAgent, GraphqlAgentModelPreference, GraphqlSaveAgentModelPreferenceInput,
    },
    artifacts::{self, GraphqlCreateConversationExternalArtifactInput},
    chat::{
        self, GraphqlConversationEvent, GraphqlConversationTranscriptPage,
        GraphqlConversationTranscriptPageInput, GraphqlPrimaryConversation,
        GraphqlSendConversationTurnInput, GraphqlSendMultipleChoiceSelectionInput,
        GraphqlTurnAccepted,
    },
    local_models::{
        self, GraphqlDefaultModelPreference, GraphqlImportLocalModelInput,
        GraphqlInstallLocalModelInput, GraphqlLocalModelCatalogEntry, GraphqlLocalModelEvent,
        GraphqlLocalModelInstallation, GraphqlLocalModelRuntimeStatus, GraphqlLocalModelSetup,
        GraphqlSaveDefaultModelPreferenceInput,
    },
    local_status::{self, GraphqlLocalStatus, GraphqlMemoryStorageStatus},
    mcp::{
        self, GraphqlAutofillToolCalibrationsResult, GraphqlContinueMcpServerSetupInput,
        GraphqlCreateMcpServerInput, GraphqlMcpOAuthSetupAttempt, GraphqlMcpServer,
        GraphqlMcpServerSetupResult, GraphqlMcpTool, GraphqlSaveToolCalibrationInput,
        GraphqlStartMcpServerOAuthSetupInput, GraphqlStartMcpServerReauthenticationOAuthSetupInput,
        GraphqlToolCalibration,
    },
    memory::{
        self, GraphqlMemoryArticle, GraphqlMemoryGraph, GraphqlMemoryGraphInput,
        GraphqlMemoryServiceStatus, GraphqlMemorySettings, GraphqlSaveMemoryServiceSettingsInput,
    },
    onboarding::{
        self, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
        GraphqlStartProviderAuthAttemptInput,
    },
    provider_accounts::{
        self, GraphqlCapabilityFeatures, GraphqlClearProviderSecretInput,
        GraphqlCreateProviderAccountInput, GraphqlDeleteProviderAccountInput,
        GraphqlProviderAccount, GraphqlProviderAccountCatalogEntry, GraphqlProviderCapability,
        GraphqlProviderSecretInput,
    },
    tasks::{
        self, GraphqlTaskComplexity, GraphqlTaskDetail, GraphqlTaskExecutionPolicy,
        GraphqlTaskExecutionPolicyInput, GraphqlTaskModelPoolEntry, GraphqlTaskModelPoolEntryInput,
        GraphqlTaskRunItemsConnection,
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

/// Shared state available to GraphQL resolvers.
#[derive(Clone)]
pub struct GraphqlState {
    runtime_state: GraphqlRuntimeState,
}

impl GraphqlState {
    fn for_schema_definition() -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_schema_definition(),
        }
    }

    /// Build test state with ready memory storage.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn for_tests() -> Self {
        Self::for_schema_definition()
    }

    /// Build test state backed by a real embedded store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: noema_store::NoemaStore) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store(store),
        }
    }

    /// Build test state backed by a real embedded store and isolated environment.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_environment(
        store: noema_store::NoemaStore,
        environment: TestEnvironment,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store_and_environment(
                store,
                environment,
            ),
        }
    }

    /// Build test state with explicit provider account operations.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_provider_account_operations(
        provider_account_operations: noema_providers::ProviderAccountOperationsHandle,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_provider_account_operations(
                provider_account_operations,
            ),
        }
    }

    /// Build test state backed by a store and runtime handle.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_runtime(
        store: noema_store::NoemaStore,
        runtime: noema_runtime::RuntimeHandle,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store_and_runtime(store, runtime),
        }
    }

    /// Attach explicit memory-service access to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_memory_service_access(
        mut self,
        memory_service_access: noema_memory::MemoryServiceAccessHandle,
    ) -> Self {
        self.runtime_state = self
            .runtime_state
            .with_memory_service_access(memory_service_access);
        self
    }

    /// Attach explicit MCP control-plane operations to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_mcp_operations(
        mut self,
        mcp_operations: noema_capabilities_mcp::McpControlPlaneHandle,
    ) -> Self {
        self.runtime_state = self.runtime_state.with_mcp_operations(mcp_operations);
        self
    }

    /// Attach explicit local-model control-plane operations to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_local_model_manager(
        mut self,
        local_model_manager: noema_providers::LocalModelManager,
    ) -> Self {
        self.runtime_state = self
            .runtime_state
            .with_local_model_manager(local_model_manager);
        self
    }

    /// Attach an explicit provider registry to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_provider_registry(
        mut self,
        provider_registry: noema_providers::ProviderRegistryHandle,
    ) -> Self {
        self.runtime_state = self.runtime_state.with_provider_registry(provider_registry);
        self
    }

    /// Build state backed by assembled host service handles.
    #[must_use]
    pub fn from_host_services(services: &noema_host::HostServices) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::from_host_services(services),
        }
    }

    pub(crate) fn runtime(&self) -> Result<&noema_runtime::RuntimeHandle> {
        self.runtime_state.runtime()
    }

    pub(crate) fn store(&self) -> Result<&noema_store::NoemaStore> {
        self.runtime_state.store()
    }

    pub(crate) fn optional_store(&self) -> Option<&noema_store::NoemaStore> {
        self.runtime_state.optional_store()
    }

    pub(crate) fn artifact_operations(&self) -> Result<&noema_artifacts::ArtifactOperationsHandle> {
        self.runtime_state.artifact_operations()
    }

    pub(crate) fn record_artifact_download_failure(&self, operation: &'static str) {
        self.runtime_state
            .record_artifact_download_failure(operation);
    }

    pub(crate) fn provider_account_operations(
        &self,
    ) -> Result<&noema_providers::ProviderAccountOperationsHandle> {
        self.runtime_state.provider_account_operations()
    }

    pub(crate) fn mcp_operations(&self) -> Result<&noema_capabilities_mcp::McpControlPlaneHandle> {
        self.runtime_state.mcp_operations()
    }

    pub(crate) fn local_model_manager(&self) -> Result<&noema_providers::LocalModelManager> {
        self.runtime_state.local_model_manager()
    }

    pub(crate) fn onboarding(&self) -> Result<&noema_host::OnboardingService> {
        self.runtime_state.onboarding()
    }

    pub(crate) fn provider_registry(&self) -> Result<&noema_providers::ProviderRegistryHandle> {
        self.runtime_state.provider_registry()
    }

    pub(crate) fn memory_repository(&self) -> Result<&noema_memory::MemoryRepositoryHandle> {
        self.runtime_state.memory_repository()
    }

    pub(crate) fn memory_service_access(&self) -> Option<&noema_memory::MemoryServiceAccessHandle> {
        self.runtime_state.memory_service_access()
    }

    pub(crate) fn memory_startup_error(&self) -> Option<&str> {
        self.runtime_state.memory_startup_error()
    }

    pub(crate) fn subscriptions(&self) -> &RuntimeEventRegistry {
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

/// Render the transport-neutral GraphQL schema definition.
#[must_use]
pub fn schema_sdl() -> String {
    build_schema(GraphqlState::for_schema_definition()).sdl()
}

include!("schema_tests.rs");
