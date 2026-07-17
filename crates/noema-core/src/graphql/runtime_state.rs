use noema_store::NoemaStore;

use noema_capabilities_mcp::McpControlPlaneHandle;
#[cfg(test)]
use noema_home::NoemaPaths;
use noema_memory::{MemoryRepositoryHandle, MemoryServiceAccessHandle};
use noema_providers::{LocalModelManager, ProviderAccountOperationsHandle, ProviderRegistryHandle};
use noema_runtime::{RuntimeEventRegistry, RuntimeHandle};

use super::local_status::GraphqlMemoryStorageStatus;

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<RuntimeHandle>,
    store: Option<NoemaStore>,
    artifact_operations: Option<noema_artifacts::ArtifactOperationsHandle>,
    artifact_diagnostics: Option<noema_host::ArtifactDiagnosticHandle>,
    provider_account_operations: Option<ProviderAccountOperationsHandle>,
    mcp_operations: Option<McpControlPlaneHandle>,
    local_model_manager: Option<LocalModelManager>,
    onboarding: Option<noema_host::OnboardingService>,
    provider_registry: Option<ProviderRegistryHandle>,
    memory_repository: Option<MemoryRepositoryHandle>,
    memory_service_access: Option<MemoryServiceAccessHandle>,
    memory_startup_error: Option<String>,
    subscriptions: RuntimeEventRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlRuntimeState {
    /// Build state from a real runtime host.
    #[must_use]
    pub fn from_host_services(services: &noema_host::HostServices) -> Self {
        Self {
            runtime: Some(services.runtime().clone()),
            store: Some(services.store().clone()),
            artifact_operations: Some(services.artifact_operations().clone()),
            artifact_diagnostics: Some(services.artifact_diagnostics().clone()),
            provider_account_operations: Some(services.provider_account_operations().clone()),
            mcp_operations: Some(services.mcp_operations().clone()),
            local_model_manager: Some(services.local_model_manager().clone()),
            onboarding: Some(services.onboarding().clone()),
            provider_registry: Some(services.provider_registry().clone()),
            memory_repository: Some(services.memory_repository().clone()),
            memory_service_access: Some(services.memory_service_access().clone()),
            memory_startup_error: services.memory_startup_error().map(str::to_string),
            subscriptions: services.runtime_events().clone(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state for resolver tests that do not need runtime access.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            runtime: None,
            store: None,
            artifact_operations: None,
            artifact_diagnostics: None,
            provider_account_operations: None,
            mcp_operations: None,
            local_model_manager: None,
            onboarding: None,
            provider_registry: None,
            memory_repository: None,
            memory_service_access: None,
            memory_startup_error: None,
            subscriptions: RuntimeEventRegistry::default(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state for resolver tests with a store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: NoemaStore) -> Self {
        let provider_account_operations = test_provider_account_operations(store.clone());
        let mcp_operations = crate::test_support::test_mcp_operations(store.clone(), None);
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        let local_model_manager =
            crate::test_support::local_model_manager(&store, crate::test_support::test_paths());
        let onboarding = noema_host::OnboardingService::new(
            store.clone(),
            provider_account_operations.clone(),
            local_model_manager.clone(),
        );
        Self {
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
            local_model_manager: Some(local_model_manager),
            onboarding: Some(onboarding),
            provider_registry: Some(crate::test_support::ready_test_provider_registry()),
            memory_repository: Some(memory_repository),
            ..Self::for_tests()
        }
    }

    /// Build state for resolver tests with a store and runtime handle.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_runtime(
        store: NoemaStore,
        runtime: RuntimeHandle,
    ) -> Self {
        let provider_account_operations = test_provider_account_operations(store.clone());
        let mcp_operations =
            crate::test_support::test_mcp_operations(store.clone(), Some(runtime.clone()));
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        let local_model_manager =
            crate::test_support::local_model_manager(&store, crate::test_support::test_paths());
        let onboarding = noema_host::OnboardingService::new(
            store.clone(),
            provider_account_operations.clone(),
            local_model_manager.clone(),
        );
        Self {
            runtime: Some(runtime),
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
            local_model_manager: Some(local_model_manager),
            onboarding: Some(onboarding),
            provider_registry: Some(crate::test_support::ready_test_provider_registry()),
            memory_repository: Some(memory_repository),
            ..Self::for_tests()
        }
    }

    /// Build state for resolver tests with a store and path root.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store_and_paths(store: NoemaStore, paths: NoemaPaths) -> Self {
        let artifact_operations =
            crate::test_support::artifact_operations_for_paths(&store, &paths)
                .expect("test artifact service");
        let artifact_diagnostics = crate::test_support::artifact_diagnostics_for_paths(&paths);
        let provider_account_operations =
            test_provider_account_operations_for_paths(store.clone(), paths.clone());
        let mcp_operations = crate::test_support::test_mcp_operations(store.clone(), None);
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        let local_model_manager = crate::test_support::local_model_manager(&store, paths.clone());
        let onboarding = noema_host::OnboardingService::new(
            store.clone(),
            provider_account_operations.clone(),
            local_model_manager.clone(),
        );
        Self {
            store: Some(store),
            artifact_operations: Some(artifact_operations),
            artifact_diagnostics: Some(artifact_diagnostics),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
            local_model_manager: Some(local_model_manager),
            onboarding: Some(onboarding),
            provider_registry: Some(crate::test_support::ready_test_provider_registry()),
            memory_repository: Some(memory_repository),
            ..Self::for_tests()
        }
    }

    /// Attach explicit MCP control-plane operations to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_mcp_operations(mut self, mcp_operations: McpControlPlaneHandle) -> Self {
        self.mcp_operations = Some(mcp_operations);
        self
    }

    /// Attach explicit local-model control-plane operations to existing test state.
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

    /// Attach explicit memory-service access to existing test state.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_memory_service_access(
        mut self,
        memory_service_access: MemoryServiceAccessHandle,
    ) -> Self {
        self.memory_service_access = Some(memory_service_access);
        self
    }

    /// Build state with explicit provider account operations.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_provider_account_operations(
        provider_account_operations: ProviderAccountOperationsHandle,
    ) -> Self {
        Self {
            provider_account_operations: Some(provider_account_operations),
            ..Self::for_tests()
        }
    }

    pub(crate) fn runtime(&self) -> async_graphql::Result<&RuntimeHandle> {
        self.runtime
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema runtime is unavailable"))
    }

    pub(crate) fn store(&self) -> async_graphql::Result<&NoemaStore> {
        self.store
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema store is unavailable"))
    }

    pub(crate) fn optional_store(&self) -> Option<&NoemaStore> {
        self.store.as_ref()
    }

    pub(crate) fn artifact_operations(
        &self,
    ) -> async_graphql::Result<&noema_artifacts::ArtifactOperationsHandle> {
        self.artifact_operations
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema artifact service is unavailable"))
    }

    pub(crate) fn record_artifact_download_failure(&self, operation: &'static str) {
        if let Some(diagnostics) = &self.artifact_diagnostics {
            diagnostics.record_download_failure(operation);
        }
    }

    pub(crate) fn provider_account_operations(
        &self,
    ) -> async_graphql::Result<&ProviderAccountOperationsHandle> {
        self.provider_account_operations.as_ref().ok_or_else(|| {
            async_graphql::Error::new("Noema provider account service is unavailable")
        })
    }

    pub(crate) fn mcp_operations(&self) -> async_graphql::Result<&McpControlPlaneHandle> {
        self.mcp_operations
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema MCP service is unavailable"))
    }

    pub(crate) fn local_model_manager(&self) -> async_graphql::Result<&LocalModelManager> {
        self.local_model_manager
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema local-model service is unavailable"))
    }

    pub(crate) fn onboarding(&self) -> async_graphql::Result<&noema_host::OnboardingService> {
        self.onboarding
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema onboarding service is unavailable"))
    }

    pub(crate) fn provider_registry(&self) -> async_graphql::Result<&ProviderRegistryHandle> {
        self.provider_registry
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema provider registry is unavailable"))
    }

    pub(crate) fn memory_repository(&self) -> async_graphql::Result<&MemoryRepositoryHandle> {
        self.memory_repository
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema memory repository is unavailable"))
    }

    pub(crate) fn memory_service_access(&self) -> Option<&MemoryServiceAccessHandle> {
        self.memory_service_access.as_ref()
    }

    pub(crate) fn memory_startup_error(&self) -> Option<&str> {
        self.memory_startup_error.as_deref()
    }

    pub(crate) fn subscriptions(&self) -> &RuntimeEventRegistry {
        &self.subscriptions
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.memory_storage
    }
}

#[cfg(test)]
fn test_provider_account_operations(store: NoemaStore) -> ProviderAccountOperationsHandle {
    std::sync::Arc::new(TestStoreBackedProviderAccountOperations { store, paths: None })
}

#[cfg(test)]
fn test_provider_account_operations_for_paths(
    store: NoemaStore,
    paths: NoemaPaths,
) -> ProviderAccountOperationsHandle {
    std::sync::Arc::new(TestStoreBackedProviderAccountOperations {
        store,
        paths: Some(paths),
    })
}

#[cfg(test)]
struct TestStoreBackedProviderAccountOperations {
    store: NoemaStore,
    paths: Option<NoemaPaths>,
}

#[cfg(test)]
impl TestStoreBackedProviderAccountOperations {
    fn account_home(
        &self,
        account: &noema_providers::ProviderAccountRecord,
    ) -> Option<std::path::PathBuf> {
        self.paths
            .as_ref()
            .map(|paths| paths.provider_account_home(&account.provider_kind, &account.account_key))
    }

    fn write_secret_marker(
        &self,
        account: &noema_providers::ProviderAccountRecord,
    ) -> Result<(), noema_providers::ProviderAccountOperationError> {
        let Some(home) = self.account_home(account) else {
            return Ok(());
        };
        std::fs::create_dir_all(&home)
            .map_err(|_| noema_providers::ProviderAccountOperationError::ProviderUnavailable)?;
        std::fs::write(home.join("api_key.json"), b"{}")
            .map_err(|_| noema_providers::ProviderAccountOperationError::ProviderUnavailable)
    }
}

#[cfg(test)]
impl noema_providers::ProviderAccountOperations for TestStoreBackedProviderAccountOperations {
    fn account_catalog(&self) -> Vec<noema_providers::ProviderAccountCatalogEntry> {
        noema_providers::provider_account_catalog()
    }

    fn active_accounts(
        &self,
    ) -> noema_providers::ProviderAccountOperationFuture<
        '_,
        Vec<noema_providers::ProviderAccountRecord>,
    > {
        Box::pin(async move {
            self.store
                .active_provider_accounts()
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)
                .map(|accounts| {
                    accounts
                        .into_iter()
                        .map(noema_providers::provider_account_from_persisted)
                        .collect()
                })
        })
    }

    fn create_secret_account(
        &self,
        request: noema_providers::CreateSecretProviderAccountRequest,
    ) -> noema_providers::ProviderAccountOperationFuture<'_, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async move {
            let catalog = noema_providers::provider_account_catalog();
            let entry = catalog
                .iter()
                .find(|entry| entry.provider_kind == request.provider_kind)
                .ok_or(noema_providers::ProviderAccountOperationError::UnsupportedProvider)?;
            if entry.auth_method != noema_providers::ProviderAuthMethod::SecretInput {
                return Err(noema_providers::ProviderAccountOperationError::AuthMethodMismatch);
            }
            let account = noema_providers::ProviderAccountPersistence::create_provider_account(
                &self.store,
                noema_providers::NewProviderAccount {
                    provider_kind: request.provider_kind,
                    display_name: request.display_name,
                    auth_method: noema_providers::ProviderAuthMethod::SecretInput,
                    status: noema_providers::ProviderAccountStatus::Authenticated,
                    metadata: serde_json::json!({
                        "secretConfigured": true,
                        "credentialRevision": 1,
                    }),
                },
            )
            .await
            .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?;
            let account = noema_providers::provider_account_from_persisted(account);
            self.write_secret_marker(&account)?;
            Ok(account)
        })
    }

    fn save_secret(
        &self,
        request: noema_providers::SaveProviderAccountSecretRequest,
    ) -> noema_providers::ProviderAccountOperationFuture<'_, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async move {
            let account = self
                .store
                .get_provider_account(&request.provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?
                .ok_or(noema_providers::ProviderAccountOperationError::AccountNotFound)?;
            if account.auth_method != noema_providers::ProviderAuthMethod::SecretInput {
                return Err(noema_providers::ProviderAccountOperationError::AuthMethodMismatch);
            }
            let account = noema_providers::provider_account_from_persisted(account);
            self.write_secret_marker(&account)?;
            let account = noema_providers::ProviderAccountPersistence::update_provider_account(
                &self.store,
                noema_providers::UpdateProviderAccountRequest {
                    provider_account_id: account.provider_account_id,
                    status: Some(noema_providers::ProviderAccountStatusUpdate {
                        status: noema_providers::ProviderAccountStatus::Authenticated,
                        error_code: None,
                        error_message: None,
                    }),
                    metadata: Some(serde_json::json!({
                        "secretConfigured": true,
                        "credentialRevision": 1,
                    })),
                },
            )
            .await
            .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?;
            Ok(noema_providers::provider_account_from_persisted(account))
        })
    }

    fn clear_secret<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async move {
            let account = self
                .store
                .get_provider_account(provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?
                .ok_or(noema_providers::ProviderAccountOperationError::AccountNotFound)?;
            if account.auth_method != noema_providers::ProviderAuthMethod::SecretInput {
                return Err(noema_providers::ProviderAccountOperationError::AuthMethodMismatch);
            }
            let account = noema_providers::provider_account_from_persisted(account);
            if let Some(home) = self.account_home(&account) {
                let secret_path = home.join("api_key.json");
                if secret_path.exists() {
                    std::fs::remove_file(secret_path).map_err(|_| {
                        noema_providers::ProviderAccountOperationError::ProviderUnavailable
                    })?;
                }
            }
            let account = noema_providers::ProviderAccountPersistence::update_provider_account(
                &self.store,
                noema_providers::UpdateProviderAccountRequest {
                    provider_account_id: account.provider_account_id,
                    status: Some(noema_providers::ProviderAccountStatusUpdate {
                        status: noema_providers::ProviderAccountStatus::Unauthenticated,
                        error_code: None,
                        error_message: None,
                    }),
                    metadata: Some(serde_json::json!({
                        "secretConfigured": false,
                        "credentialRevision": 1,
                    })),
                },
            )
            .await
            .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?;
            Ok(noema_providers::provider_account_from_persisted(account))
        })
    }

    fn delete_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, bool> {
        Box::pin(async move {
            let Some(account) = self
                .store
                .get_provider_account(provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?
            else {
                return Ok(false);
            };
            if account.is_default {
                return Err(noema_providers::ProviderAccountOperationError::ProtectedAccount);
            }
            let deleted = self
                .store
                .delete_provider_account(provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?;
            if deleted
                && let Some(home) =
                    self.account_home(&noema_providers::provider_account_from_persisted(account))
                && home.exists()
            {
                std::fs::remove_dir_all(home).map_err(|_| {
                    noema_providers::ProviderAccountOperationError::ProviderUnavailable
                })?;
            }
            Ok(deleted)
        })
    }

    fn start_auth(
        &self,
        _request: noema_providers::StartProviderAuthRequest,
    ) -> noema_providers::ProviderAccountOperationFuture<'_, noema_providers::ProviderAuthAttemptView>
    {
        Box::pin(async { Err(noema_providers::ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn auth_attempt<'a>(
        &'a self,
        _attempt_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<
        'a,
        Option<noema_providers::ProviderAuthAttemptView>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn cancel_auth_attempt<'a>(
        &'a self,
        _attempt_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<
        'a,
        Option<noema_providers::ProviderAuthAttemptView>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn record_auth_failure<'a>(
        &'a self,
        _provider_account_id: &'a str,
        _expected_credential_revision: u64,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async { Err(noema_providers::ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn reconcile_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async move {
            let account = self
                .store
                .get_provider_account(provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)?
                .ok_or(noema_providers::ProviderAccountOperationError::AccountNotFound)?;
            if !account.is_active {
                return Err(noema_providers::ProviderAccountOperationError::AccountInactive);
            }
            Ok(noema_providers::provider_account_from_persisted(account))
        })
    }

    fn refresh_model_catalog<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, noema_providers::ProviderAccountRecord>
    {
        self.reconcile_account(provider_account_id)
    }
}
