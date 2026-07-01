use crate::{
    NoemaPaths, NoemaRuntimeHost, NoemaStore, daemon::CodexRuntimeHandle,
    mcp::McpOAuthSetupManager, provider::auth::ProviderAuthManager,
};

use super::{ConversationSubscriptionRegistry, local_status::GraphqlMemoryStorageStatus};

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<CodexRuntimeHandle>,
    store: Option<NoemaStore>,
    provider_auth: Option<ProviderAuthManager>,
    mcp_oauth: Option<McpOAuthSetupManager>,
    paths: Option<NoemaPaths>,
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
            provider_auth: Some(host.provider_auth().clone()),
            mcp_oauth: Some(host.mcp_oauth().clone()),
            paths: Some(host.paths().clone()),
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
            provider_auth: None,
            mcp_oauth: Some(McpOAuthSetupManager::new()),
            paths: None,
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
        Self {
            store: Some(store),
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

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.memory_storage
    }
}
