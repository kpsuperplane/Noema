use crate::{
    NoemaRuntimeHost, NoemaStore, daemon::CodexRuntimeHandle, mcp::McpOAuthSetupManager,
    provider::auth::ProviderAuthManager,
};
use noema_home::{NoemaPaths, SystemErrorEvent, SystemErrorLogger};

use super::{ConversationSubscriptionRegistry, local_status::GraphqlMemoryStorageStatus};

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<CodexRuntimeHandle>,
    store: Option<NoemaStore>,
    artifact_operations: Option<noema_artifacts::ArtifactOperationsHandle>,
    artifact_diagnostics: ArtifactDiagnosticReporter,
    provider_auth: Option<ProviderAuthManager>,
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
            provider_auth: Some(host.provider_auth().clone()),
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
            provider_auth: None,
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
        Self {
            store: Some(store),
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
        Self {
            runtime: Some(runtime),
            store: Some(store),
            ..Self::for_tests()
        }
    }

    /// Build state for resolver tests with a store and path root.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store_and_paths(store: NoemaStore, paths: NoemaPaths) -> Self {
        let artifact_operations =
            crate::test_support::artifact_operations(&store).expect("test artifact service");
        Self {
            store: Some(store),
            artifact_operations: Some(artifact_operations),
            artifact_diagnostics: ArtifactDiagnosticReporter::new(SystemErrorLogger::from_paths(
                &paths,
            )),
            paths: Some(paths),
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

    pub(crate) fn provider_auth(&self) -> async_graphql::Result<&ProviderAuthManager> {
        self.provider_auth
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema provider auth is unavailable"))
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
