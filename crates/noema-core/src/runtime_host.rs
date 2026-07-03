//! Shared Noema runtime host used by daemon and desktop shells.

use crate::{
    DaemonError, NoemaHomeInitOptions, NoemaPathError, NoemaPaths, NoemaStore, ProviderConfig,
    StoreConfig, SystemErrorLogger, daemon::CodexRuntimeHandle, mcp::McpOAuthSetupManager,
    provider::auth::ProviderAuthManager,
};

use thiserror::Error;

/// Shared host state for Noema client surfaces.
pub struct NoemaRuntimeHost {
    runtime: CodexRuntimeHandle,
    store: NoemaStore,
    provider_auth: ProviderAuthManager,
    mcp_oauth: McpOAuthSetupManager,
    system_errors: SystemErrorLogger,
    paths: NoemaPaths,
    #[allow(dead_code)]
    subscriptions: crate::graphql::ConversationSubscriptionRegistry,
}

impl NoemaRuntimeHost {
    /// Start the shared Noema runtime host.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHostError`] when path setup, store startup, or runtime
    /// startup fails.
    pub async fn start(provider: ProviderConfig) -> Result<Self, RuntimeHostError> {
        let paths = NoemaPaths::from_process_env()
            .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
        crate::init_noema_home(
            &paths,
            NoemaHomeInitOptions {
                force: false,
                write_config: !paths.config_exists(),
            },
        )
        .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;

        let system_errors = SystemErrorLogger::from_paths(&paths);
        let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        store
            .ensure_default_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;

        let runtime =
            CodexRuntimeHandle::spawn_from_config(provider, store.clone(), system_errors.clone())
                .await
                .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;

        Ok(Self {
            runtime,
            store,
            provider_auth: ProviderAuthManager::new(),
            mcp_oauth: McpOAuthSetupManager::new(),
            system_errors,
            paths,
            subscriptions: crate::graphql::ConversationSubscriptionRegistry::default(),
        })
    }

    /// Runtime command handle.
    pub(crate) fn runtime(&self) -> &CodexRuntimeHandle {
        &self.runtime
    }

    /// Store handle.
    #[must_use]
    pub fn store(&self) -> &NoemaStore {
        &self.store
    }

    /// Provider auth manager.
    #[must_use]
    pub fn provider_auth(&self) -> &ProviderAuthManager {
        &self.provider_auth
    }

    /// MCP OAuth setup manager.
    #[must_use]
    pub fn mcp_oauth(&self) -> &McpOAuthSetupManager {
        &self.mcp_oauth
    }

    /// Developer diagnostic system error logger.
    #[must_use]
    pub fn system_errors(&self) -> &SystemErrorLogger {
        &self.system_errors
    }

    /// Resolved Noema paths.
    #[must_use]
    pub fn paths(&self) -> &NoemaPaths {
        &self.paths
    }

    /// Conversation subscription registry.
    #[allow(dead_code)]
    pub(crate) fn subscriptions(&self) -> &crate::graphql::ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    /// Shut down runtime-owned work.
    pub async fn shutdown(self) {
        self.runtime.shutdown().await;
    }
}

/// Runtime host startup error.
#[derive(Debug, Error)]
pub enum RuntimeHostError {
    /// Data folder/path setup failed.
    #[error("data folder setup failed: {0}")]
    DataFolder(String),
    /// Store setup failed.
    #[error("store setup failed: {0}")]
    Store(String),
    /// Runtime setup failed.
    #[error("runtime setup failed: {0}")]
    Runtime(String),
}

impl RuntimeHostError {
    /// Plain-language user-facing message.
    #[must_use]
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::DataFolder(_) => "Noema could not open its data folder.",
            Self::Store(_) => "Noema could not start its local memory store.",
            Self::Runtime(_) => "Noema could not start the local assistant service.",
        }
    }

    /// Detailed diagnostic string.
    #[must_use]
    pub fn technical_details(&self) -> String {
        self.to_string()
    }
}

impl From<NoemaPathError> for RuntimeHostError {
    fn from(source: NoemaPathError) -> Self {
        Self::DataFolder(source.to_string())
    }
}

impl From<DaemonError> for RuntimeHostError {
    fn from(source: DaemonError) -> Self {
        Self::Runtime(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_host_error_messages_are_plain_language() {
        assert_eq!(
            RuntimeHostError::DataFolder("permission denied".to_string()).user_message(),
            "Noema could not open its data folder."
        );
        assert_eq!(
            RuntimeHostError::Store("rocksdb failed".to_string()).user_message(),
            "Noema could not start its local memory store."
        );
        assert_eq!(
            RuntimeHostError::Runtime("provider failed".to_string()).user_message(),
            "Noema could not start the local assistant service."
        );
    }
}
