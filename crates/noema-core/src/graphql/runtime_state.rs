use crate::{NoemaRuntimeHost, NoemaStore, daemon::CodexRuntimeHandle, mcp::McpOAuthSetupManager};
use noema_home::{NoemaPaths, SystemErrorEvent, SystemErrorLogger};
use noema_providers::ProviderAccountOperationsHandle;

use super::{ConversationSubscriptionRegistry, local_status::GraphqlMemoryStorageStatus};

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<CodexRuntimeHandle>,
    store: Option<NoemaStore>,
    artifact_operations: Option<noema_artifacts::ArtifactOperationsHandle>,
    artifact_diagnostics: ArtifactDiagnosticReporter,
    provider_account_operations: Option<ProviderAccountOperationsHandle>,
    mcp_oauth: Option<McpOAuthSetupManager>,
    paths: Option<NoemaPaths>,
    memory_connection: Option<crate::MnemosyneConnection>,
    memory_startup_error: Option<String>,
    subscriptions: ConversationSubscriptionRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlRuntimeState {
    /// Build state from a real runtime host.
    #[must_use]
    pub fn from_host(host: &NoemaRuntimeHost) -> Self {
        Self {
            runtime: Some(host.runtime().clone()),
            store: Some(host.store().clone()),
            artifact_operations: Some(host.artifact_operations().clone()),
            artifact_diagnostics: ArtifactDiagnosticReporter::new(host.system_errors().clone()),
            provider_account_operations: Some(host.provider_account_operations().clone()),
            mcp_oauth: Some(host.mcp_oauth().clone()),
            paths: Some(host.paths().clone()),
            memory_connection: host.memory_connection().cloned(),
            memory_startup_error: host.memory_startup_error().map(str::to_string),
            subscriptions: host.subscriptions().clone(),
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
            artifact_diagnostics: ArtifactDiagnosticReporter::default(),
            provider_account_operations: None,
            mcp_oauth: Some(McpOAuthSetupManager::new()),
            paths: None,
            memory_connection: None,
            memory_startup_error: None,
            subscriptions: ConversationSubscriptionRegistry::default(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state for resolver tests with a store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: NoemaStore) -> Self {
        let provider_account_operations = test_provider_account_operations(store.clone());
        Self {
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            ..Self::for_tests()
        }
    }

    /// Build state for resolver tests with a store and runtime handle.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_runtime(
        store: NoemaStore,
        runtime: CodexRuntimeHandle,
    ) -> Self {
        let provider_account_operations = test_provider_account_operations(store.clone());
        Self {
            runtime: Some(runtime),
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            ..Self::for_tests()
        }
    }

    /// Build state for resolver tests with a store and path root.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store_and_paths(store: NoemaStore, paths: NoemaPaths) -> Self {
        let artifact_operations =
            crate::test_support::artifact_operations(&store).expect("test artifact service");
        let provider_account_operations = test_provider_account_service(&store, &paths);
        Self {
            store: Some(store),
            artifact_operations: Some(artifact_operations),
            artifact_diagnostics: ArtifactDiagnosticReporter::new(SystemErrorLogger::from_paths(
                &paths,
            )),
            provider_account_operations: Some(provider_account_operations),
            paths: Some(paths),
            ..Self::for_tests()
        }
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

    pub(crate) fn runtime(&self) -> async_graphql::Result<&CodexRuntimeHandle> {
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
        self.artifact_diagnostics.download_failure(operation);
    }

    pub(crate) fn provider_account_operations(
        &self,
    ) -> async_graphql::Result<&ProviderAccountOperationsHandle> {
        self.provider_account_operations.as_ref().ok_or_else(|| {
            async_graphql::Error::new("Noema provider account service is unavailable")
        })
    }

    pub(crate) fn mcp_oauth(&self) -> async_graphql::Result<&McpOAuthSetupManager> {
        self.mcp_oauth
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema MCP OAuth setup is unavailable"))
    }

    pub(crate) fn paths(&self) -> async_graphql::Result<&NoemaPaths> {
        self.paths
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema paths are unavailable"))
    }

    pub(crate) fn memory_connection(&self) -> Option<&crate::MnemosyneConnection> {
        self.memory_connection.as_ref()
    }

    pub(crate) fn memory_startup_error(&self) -> Option<&str> {
        self.memory_startup_error.as_deref()
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.memory_storage
    }
}

#[cfg(test)]
fn test_provider_account_service(
    store: &NoemaStore,
    paths: &NoemaPaths,
) -> ProviderAccountOperationsHandle {
    use noema_providers::{
        ProviderAccountPersistenceHandle, ProviderAccountService,
        ProviderModelCatalogPersistenceHandle,
    };
    use std::sync::Arc;

    let accounts: ProviderAccountPersistenceHandle = Arc::new(store.clone());
    let catalogs: ProviderModelCatalogPersistenceHandle = Arc::new(store.clone());
    ProviderAccountService::new(
        paths.clone(),
        accounts,
        catalogs,
        SystemErrorLogger::from_paths(paths),
    )
    .expect("test provider account service")
    .operations()
}

#[cfg(test)]
fn test_provider_account_operations(store: NoemaStore) -> ProviderAccountOperationsHandle {
    std::sync::Arc::new(TestStoreBackedProviderAccountOperations { store })
}

#[cfg(test)]
struct TestStoreBackedProviderAccountOperations {
    store: NoemaStore,
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
        })
    }

    fn create_secret_account(
        &self,
        _request: noema_providers::CreateSecretProviderAccountRequest,
    ) -> noema_providers::ProviderAccountOperationFuture<'_, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async { Err(noema_providers::ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn save_secret(
        &self,
        _request: noema_providers::SaveProviderAccountSecretRequest,
    ) -> noema_providers::ProviderAccountOperationFuture<'_, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async { Err(noema_providers::ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn clear_secret<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> noema_providers::ProviderAccountOperationFuture<'a, noema_providers::ProviderAccountRecord>
    {
        Box::pin(async { Err(noema_providers::ProviderAccountOperationError::UnsupportedProvider) })
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
            self.store
                .delete_provider_account(provider_account_id)
                .await
                .map_err(|_| noema_providers::ProviderAccountOperationError::Persistence)
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
            Ok(account)
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

#[derive(Clone, Default)]
struct ArtifactDiagnosticReporter {
    system_errors: Option<SystemErrorLogger>,
}

impl ArtifactDiagnosticReporter {
    fn new(system_errors: SystemErrorLogger) -> Self {
        Self {
            system_errors: Some(system_errors),
        }
    }

    fn download_failure(&self, operation: &'static str) {
        let Some(system_errors) = &self.system_errors else {
            return;
        };
        let event = SystemErrorEvent::new(
            "artifact_download_failure",
            "artifact download operation failed",
        )
        .with_context(serde_json::json!({ "operation": operation }));
        if system_errors.append(event).is_err() {
            eprintln!("Noema artifact download failure: diagnostic_write");
        }
    }
}
