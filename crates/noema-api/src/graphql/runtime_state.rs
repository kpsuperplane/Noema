use noema_store::NoemaStore;

use noema_capabilities_mcp::McpControlPlaneHandle;
use noema_memory::NativeMemory;
use noema_providers::{LocalModelManager, ProviderAccountOperationsHandle, ProviderRegistryHandle};
use noema_runtime::{RuntimeEventRegistry, RuntimeHandle};

use super::local_status::GraphqlMemoryStorageStatus;

macro_rules! required_service_accessors {
    ($($method:ident => $field:ident: $service:ty = $message:literal;)+) => {
        $(
            pub(crate) fn $method(&self) -> async_graphql::Result<&$service> {
                req(self.$field.as_ref(), $message)
            }
        )+
    };
}

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone, Default)]
pub struct GraphqlState {
    runtime: Option<RuntimeHandle>,
    store: Option<NoemaStore>,
    artifact_operations: Option<noema_artifacts::ArtifactOperationsHandle>,
    artifact_diagnostics: Option<noema_host::ArtifactDiagnosticHandle>,
    provider_account_operations: Option<ProviderAccountOperationsHandle>,
    mcp_operations: Option<McpControlPlaneHandle>,
    local_model_manager: Option<LocalModelManager>,
    onboarding: Option<noema_host::OnboardingService>,
    provider_registry: Option<ProviderRegistryHandle>,
    native_memory: Option<NativeMemory>,
    subscriptions: RuntimeEventRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlState {
    /// Build state from a real runtime host.
    #[must_use]
    pub fn from_host_services(services: &noema_host::HostServices) -> Self {
        Self {
            runtime: Some(services.runtime.clone()),
            store: Some(services.store.clone()),
            artifact_operations: Some(services.artifact_operations.clone()),
            artifact_diagnostics: Some(services.artifact_diagnostics.clone()),
            provider_account_operations: Some(services.provider_account_operations.clone()),
            mcp_operations: Some(services.mcp_operations.clone()),
            local_model_manager: Some(services.local_model_manager.clone()),
            onboarding: Some(services.onboarding.clone()),
            provider_registry: Some(services.provider_registry.clone()),
            native_memory: Some(services.native_memory.clone()),
            subscriptions: services.runtime_events.clone(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build inert state for schema inspection without application services.
    #[must_use]
    pub(super) fn for_schema_definition() -> Self {
        Self::default()
    }

    /// Build test state with ready memory storage.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn for_tests() -> Self {
        Self::for_schema_definition()
    }

    /// Build state for resolver tests with a store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: NoemaStore) -> Self {
        Self::for_tests_with_store_context(store, None)
    }

    /// Build state for resolver tests with an isolated filesystem environment.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_environment(
        store: NoemaStore,
        environment: crate::test_support::TestEnvironment,
    ) -> Self {
        Self::for_tests_with_store_context(store, Some(environment))
    }

    #[cfg(test)]
    fn for_tests_with_store_context(
        store: NoemaStore,
        environment: Option<crate::test_support::TestEnvironment>,
    ) -> Self {
        let provider_account_operations = environment.as_ref().map_or_else(
            || test_provider_account_operations(store.clone()),
            |environment| {
                test_provider_account_operations_for_environment(store.clone(), environment.clone())
            },
        );
        let mut state = Self {
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            provider_registry: Some(crate::test_support::ready_test_provider_registry()),
            ..Self::for_schema_definition()
        };
        if let Some(environment) = environment {
            let store = state.store.as_ref().expect("test store");
            state.artifact_operations = Some(
                crate::test_support::artifact_operations_for_environment(store, &environment)
                    .expect("test artifact service"),
            );
            state.artifact_diagnostics = Some(
                crate::test_support::artifact_diagnostics_for_environment(&environment),
            );
        }
        state
    }

    /// Attach explicit local-model operations to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_local_model_manager(
        mut self,
        local_model_manager: LocalModelManager,
    ) -> Self {
        self.provider_registry = Some(local_model_manager.registry());
        self.onboarding = self
            .store
            .clone()
            .zip(self.provider_account_operations.clone())
            .map(|(store, provider_accounts)| {
                noema_host::OnboardingService::new(
                    store,
                    provider_accounts,
                    local_model_manager.clone(),
                )
            });
        self.local_model_manager = Some(local_model_manager);
        self
    }

    /// Attach explicit MCP control-plane operations to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_mcp_operations(mut self, mcp_operations: McpControlPlaneHandle) -> Self {
        self.mcp_operations = Some(mcp_operations);
        self
    }

    /// Attach an explicit runtime to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_runtime(mut self, runtime: RuntimeHandle) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Attach an explicit provider registry to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_provider_registry(
        mut self,
        provider_registry: ProviderRegistryHandle,
    ) -> Self {
        self.provider_registry = Some(provider_registry);
        self
    }

    /// Attach native memory to an isolated resolver test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_native_memory(mut self, native_memory: NativeMemory) -> Self {
        self.native_memory = Some(native_memory);
        self
    }

    required_service_accessors! {
        runtime => runtime: RuntimeHandle = "Noema runtime is unavailable";
        store => store: NoemaStore = "Noema store is unavailable";
        artifact_operations => artifact_operations: noema_artifacts::ArtifactOperationsHandle = "Noema artifact service is unavailable";
        provider_account_operations => provider_account_operations: ProviderAccountOperationsHandle = "Noema provider account service is unavailable";
        mcp_operations => mcp_operations: McpControlPlaneHandle = "Noema MCP service is unavailable";
        local_model_manager => local_model_manager: LocalModelManager = "Noema local-model service is unavailable";
        onboarding => onboarding: noema_host::OnboardingService = "Noema onboarding service is unavailable";
        provider_registry => provider_registry: ProviderRegistryHandle = "Noema provider registry is unavailable";
    }

    pub(crate) fn optional_store(&self) -> Option<&NoemaStore> {
        self.store.as_ref()
    }

    pub(crate) fn optional_runtime(&self) -> Option<&RuntimeHandle> {
        self.runtime.as_ref()
    }

    pub(crate) fn record_artifact_download_failure(&self, operation: &'static str) {
        if let Some(diagnostics) = &self.artifact_diagnostics {
            diagnostics.record_download_failure(operation);
        }
    }

    pub(crate) fn subscriptions(&self) -> &RuntimeEventRegistry {
        &self.subscriptions
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.memory_storage
    }

    pub(crate) fn native_memory(&self) -> Option<&NativeMemory> {
        self.native_memory.as_ref()
    }
}

fn req<'a, T>(value: Option<&'a T>, msg: &'static str) -> async_graphql::Result<&'a T> {
    value.ok_or_else(|| async_graphql::Error::new(msg))
}

#[cfg(test)]
fn test_provider_account_operations(store: NoemaStore) -> ProviderAccountOperationsHandle {
    test_provider_account_operations_for_environment(store, crate::test_support::test_environment())
}

#[cfg(test)]
fn test_provider_account_operations_for_environment(
    store: NoemaStore,
    environment: crate::test_support::TestEnvironment,
) -> ProviderAccountOperationsHandle {
    let paths =
        noema_home::NoemaPaths::from_noema_home(environment.root()).expect("test provider paths");
    let service = noema_providers::ProviderAccountService::new(
        paths.clone(),
        std::sync::Arc::new(store.clone()),
        std::sync::Arc::new(store),
        noema_home::SystemErrorLogger::from_paths(&paths),
    )
    .expect("test provider account service");
    service.operations()
}
