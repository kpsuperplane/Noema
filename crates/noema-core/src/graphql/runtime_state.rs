use noema_store::NoemaStore;

use crate::{
    NoemaRuntimeHost,
    daemon::{CodexRuntimeHandle, RuntimeEventRegistry},
};
use noema_capabilities_mcp::McpControlPlaneHandle;
#[cfg(test)]
use noema_home::NoemaPaths;
use noema_home::{SystemErrorEvent, SystemErrorLogger};
use noema_memory::{MemoryRepositoryHandle, MemoryServiceAccessHandle};
use noema_providers::{LocalModelManager, ProviderAccountOperationsHandle, ProviderRegistryHandle};

use super::local_status::GraphqlMemoryStorageStatus;

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<CodexRuntimeHandle>,
    store: Option<NoemaStore>,
    artifact_operations: Option<noema_artifacts::ArtifactOperationsHandle>,
    artifact_diagnostics: ArtifactDiagnosticReporter,
    provider_account_operations: Option<ProviderAccountOperationsHandle>,
    mcp_operations: Option<McpControlPlaneHandle>,
    local_model_manager: Option<LocalModelManager>,
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
    pub fn from_host(host: &NoemaRuntimeHost) -> Self {
        Self {
            runtime: Some(host.runtime().clone()),
            store: Some(host.store().clone()),
            artifact_operations: Some(host.artifact_operations().clone()),
            artifact_diagnostics: ArtifactDiagnosticReporter::new(host.system_errors().clone()),
            provider_account_operations: Some(host.provider_account_operations().clone()),
            mcp_operations: Some(host.mcp_operations().clone()),
            local_model_manager: Some(host.local_model_manager().clone()),
            provider_registry: Some(host.local_model_manager().registry()),
            memory_repository: Some(host.memory_repository().clone()),
            memory_service_access: Some(host.memory_service_access().clone()),
            memory_startup_error: host.memory_startup_error().map(str::to_string),
            subscriptions: host.runtime_events().clone(),
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
            mcp_operations: None,
            local_model_manager: None,
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
        let mcp_operations = test_mcp_operations(store.clone(), None);
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        Self {
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
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
        runtime: CodexRuntimeHandle,
    ) -> Self {
        let provider_account_operations = test_provider_account_operations(store.clone());
        let mcp_operations = test_mcp_operations(store.clone(), Some(runtime.clone()));
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        Self {
            runtime: Some(runtime),
            store: Some(store),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
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
        let provider_account_operations = test_provider_account_service(&store, &paths);
        let mcp_operations = test_mcp_operations(store.clone(), None);
        let memory_repository: MemoryRepositoryHandle = std::sync::Arc::new(store.clone());
        Self {
            store: Some(store),
            artifact_operations: Some(artifact_operations),
            artifact_diagnostics: ArtifactDiagnosticReporter::new(SystemErrorLogger::from_paths(
                &paths,
            )),
            provider_account_operations: Some(provider_account_operations),
            mcp_operations: Some(mcp_operations),
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
use test_mcp::test_mcp_operations;

#[cfg(test)]
mod test_mcp {
    use std::sync::Arc;

    use noema_capabilities_mcp::{
        CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
        McpAutofillCalibrationsCommand, McpAutofillCalibrationsResult, McpControlPlaneHandle,
        McpDeleteServerCommand, McpDeleteServerResult, McpListToolsCommand,
        McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptView, McpOperationError, McpOperationFuture,
        McpOperations, McpRepository, McpSaveCalibrationsCommand, McpSaveCalibrationsResult,
        McpServerList, McpServerSetupResult, McpToolList, StartMcpOAuthReauthenticationCommand,
        StartMcpOAuthSetupCommand, build_autofill_prompt, parse_autofill_response,
    };

    use noema_store::NoemaStore;

    use crate::daemon::CodexRuntimeHandle;

    pub(super) fn test_mcp_operations(
        store: NoemaStore,
        runtime: Option<CodexRuntimeHandle>,
    ) -> McpControlPlaneHandle {
        Arc::new(TestStoreMcpOperations { store, runtime })
    }

    struct TestStoreMcpOperations {
        store: NoemaStore,
        runtime: Option<CodexRuntimeHandle>,
    }

    impl McpOperations for TestStoreMcpOperations {
        fn list_servers(&self) -> McpOperationFuture<'_, Result<McpServerList, McpOperationError>> {
            Box::pin(async move {
                self.store
                    .control_plane_catalog()
                    .await
                    .map(|servers| McpServerList {
                        servers: servers.into_iter().map(|server| server.server).collect(),
                    })
                    .map_err(|_| McpOperationError::Unavailable)
            })
        }

        fn list_tools(
            &self,
            command: McpListToolsCommand,
        ) -> McpOperationFuture<'_, Result<McpToolList, McpOperationError>> {
            Box::pin(async move {
                let server = self
                    .store
                    .control_plane_server(command.mcp_server_id)
                    .await
                    .map_err(|_| McpOperationError::Unavailable)?
                    .ok_or(McpOperationError::NotFound)?;
                Ok(McpToolList {
                    server: server.server,
                    tools: server.tools,
                })
            })
        }

        fn create_server(
            &self,
            _command: CreateMcpServerCommand,
        ) -> McpOperationFuture<'_, Result<McpServerSetupResult, McpOperationError>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }

        fn continue_setup(
            &self,
            _command: ContinueMcpServerSetupCommand,
        ) -> McpOperationFuture<'_, Result<McpServerSetupResult, McpOperationError>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }

        fn start_oauth_setup(
            &self,
            _command: StartMcpOAuthSetupCommand,
        ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }

        fn start_oauth_reauthentication(
            &self,
            _command: StartMcpOAuthReauthenticationCommand,
        ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }

        fn oauth_setup_attempt(
            &self,
            _query: McpOAuthSetupAttemptQuery,
        ) -> McpOperationFuture<'_, Result<Option<McpOAuthSetupAttemptView>, McpOperationError>>
        {
            Box::pin(async { Ok(None) })
        }

        fn complete_oauth_setup(
            &self,
            _command: CompleteMcpOAuthSetupCommand,
        ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }

        fn autofill_calibrations(
            &self,
            command: McpAutofillCalibrationsCommand,
        ) -> McpOperationFuture<'_, Result<McpAutofillCalibrationsResult, McpOperationError>>
        {
            Box::pin(async move {
                let runtime = self
                    .runtime
                    .as_ref()
                    .ok_or(McpOperationError::Unavailable)?;
                let server = self
                    .store
                    .control_plane_server(command.mcp_server_id)
                    .await
                    .map_err(|_| McpOperationError::Unavailable)?
                    .ok_or(McpOperationError::NotFound)?;
                let tools = server
                    .tools
                    .iter()
                    .map(|tool| tool.tool.clone())
                    .collect::<Vec<_>>();
                let prompt = build_autofill_prompt(&server.server.display_name, &tools);
                let mut request = noema_providers::GenerateRequest::text(prompt);
                request.instructions =
                    Some("Return strict JSON only for MCP calibration suggestions.".to_string());
                let response = runtime
                    .generate_once_with_tool_classification_model(request)
                    .await
                    .map_err(|_| McpOperationError::Failed)?;
                let suggestions = parse_autofill_response(&response.assistant_text(), &tools)
                    .map_err(|_| McpOperationError::MalformedResponse)?;
                Ok(McpAutofillCalibrationsResult { suggestions })
            })
        }

        fn save_calibrations(
            &self,
            command: McpSaveCalibrationsCommand,
        ) -> McpOperationFuture<'_, Result<McpSaveCalibrationsResult, McpOperationError>> {
            Box::pin(async move {
                self.store
                    .save_calibrations(command.calibrations)
                    .await
                    .map(|calibrations| McpSaveCalibrationsResult { calibrations })
                    .map_err(|_| McpOperationError::InvalidInput)
            })
        }

        fn delete_server(
            &self,
            command: McpDeleteServerCommand,
        ) -> McpOperationFuture<'_, Result<McpDeleteServerResult, McpOperationError>> {
            Box::pin(async move {
                let Some(ticket) = self
                    .store
                    .begin_delete(command.mcp_server_id)
                    .await
                    .map_err(|_| McpOperationError::Unavailable)?
                else {
                    return Ok(McpDeleteServerResult { deleted: false });
                };
                let deleted = self
                    .store
                    .finish_delete(ticket)
                    .await
                    .map_err(|_| McpOperationError::Unavailable)?;
                Ok(McpDeleteServerResult { deleted })
            })
        }
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
