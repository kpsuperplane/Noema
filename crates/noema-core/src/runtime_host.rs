//! Shared Noema runtime host used by daemon and desktop shells.

use crate::{
    DaemonError, NoemaHomeInitOptions, NoemaPathError, NoemaPaths, NoemaStore, ProviderConfig,
    StoreConfig, SystemErrorLogger, daemon::CodexRuntimeHandle, mcp::McpOAuthSetupManager,
    provider::DEFAULT_TOOL_CLASSIFICATION_MODEL, provider::auth::ProviderAuthManager,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

/// Shared host state for Noema client surfaces.
pub struct NoemaRuntimeHost {
    runtime: CodexRuntimeHandle,
    store: NoemaStore,
    provider_auth: ProviderAuthManager,
    mcp_oauth: McpOAuthSetupManager,
    mnemosyne: Option<crate::MnemosyneLifecycle>,
    memory_startup_error: Option<String>,
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

        let (default_provider_kind, providers) =
            CodexRuntimeHandle::provider_map_from_config(provider, system_errors.clone())
                .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;

        let memory_settings = store
            .memory_service_settings()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        let mut mnemosyne_connection = match memory_settings.mode {
            crate::MemoryServiceMode::External => memory_settings
                .base_url
                .clone()
                .map(|base_url| crate::MnemosyneConnection::new(base_url, None)),
            crate::MemoryServiceMode::Managed => None,
        };
        let mut memory_startup_error = None;
        let memory_model_proxy = match memory_settings.mode {
            crate::MemoryServiceMode::External => None,
            crate::MemoryServiceMode::Managed => {
                match memory_model_proxy_config_from_settings(
                    &memory_settings,
                    &default_provider_kind,
                    &providers,
                    generate_memory_model_proxy_api_key().map_err(RuntimeHostError::Runtime)?,
                    system_errors.clone(),
                ) {
                    Ok(config) => match crate::MemoryModelProxy::start(config).await {
                        Ok(proxy) => Some(proxy),
                        Err(error) => {
                            let error = error.to_string();
                            memory_startup_error = Some(error.clone());
                            system_errors.try_append(
                                crate::SystemErrorEvent::new(
                                    "memory_model_proxy_unavailable",
                                    "Memory model proxy is unavailable",
                                )
                                .with_error_chain([error]),
                            );
                            None
                        }
                    },
                    Err(error) => {
                        memory_startup_error = Some(error.clone());
                        system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "memory_model_proxy_unavailable",
                                "Memory model proxy is unavailable",
                            )
                            .with_error_chain([error]),
                        );
                        None
                    }
                }
            }
        };

        let mnemosyne = if memory_settings.mode == crate::MemoryServiceMode::Managed
            && memory_model_proxy.is_none()
            && memory_startup_error.is_some()
        {
            None
        } else {
            match crate::MnemosyneLifecycle::start(
                &paths,
                &memory_settings,
                system_errors.clone(),
                memory_model_proxy,
            )
            .await
            {
                Ok(lifecycle) => {
                    if let Some(connection) = lifecycle.connection().cloned() {
                        mnemosyne_connection = Some(connection);
                    }
                    Some(lifecycle)
                }
                Err(error) => {
                    memory_startup_error = Some(error.to_string());
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "mnemosyne_lifecycle_unavailable",
                            "Mnemosyne lifecycle is unavailable",
                        )
                        .with_error_chain([error.to_string()]),
                    );
                    None
                }
            }
        };

        let runtime = CodexRuntimeHandle::spawn_with_provider_map_and_memory(
            default_provider_kind,
            providers,
            store.clone(),
            system_errors.clone(),
            mnemosyne_connection,
        )
        .await
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;

        Ok(Self {
            runtime,
            store,
            provider_auth: ProviderAuthManager::new(),
            mcp_oauth: McpOAuthSetupManager::new(),
            mnemosyne,
            memory_startup_error,
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

    /// Runtime-only managed memory service connection, when available.
    #[must_use]
    pub fn memory_connection(&self) -> Option<&crate::MnemosyneConnection> {
        self.mnemosyne
            .as_ref()
            .and_then(crate::MnemosyneLifecycle::connection)
    }

    /// Runtime-only managed memory service startup error, when startup failed.
    #[must_use]
    pub fn memory_startup_error(&self) -> Option<&str> {
        self.memory_startup_error.as_deref()
    }

    /// Conversation subscription registry.
    #[allow(dead_code)]
    pub(crate) fn subscriptions(&self) -> &crate::graphql::ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    /// Shut down runtime-owned work.
    pub async fn shutdown(self) {
        self.runtime.shutdown().await;
        if let Some(mnemosyne) = self.mnemosyne {
            mnemosyne.shutdown().await;
        }
    }
}

fn memory_model_proxy_config_from_settings(
    settings: &crate::MemoryServiceSettingsRecord,
    default_provider_kind: &str,
    providers: &crate::daemon::RuntimeProviderMap,
    api_key: String,
    system_errors: SystemErrorLogger,
) -> Result<crate::MemoryModelProxyConfig, String> {
    let provider_kind = settings
        .provider_kind
        .as_deref()
        .unwrap_or(default_provider_kind);
    let provider = providers
        .get(provider_kind)
        .cloned()
        .ok_or_else(|| format!("memory model provider is unavailable: {provider_kind}"))?;
    let model_profile = settings
        .model_profile
        .clone()
        .or_else(|| provider.default_tool_classification_model())
        .unwrap_or_else(|| DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string());
    Ok(crate::MemoryModelProxyConfig {
        provider,
        api_key,
        model_profile,
        reasoning_effort: settings.reasoning_effort,
        system_errors: Some(system_errors),
    })
}

fn generate_memory_model_proxy_api_key() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "could not generate memory model proxy API key".to_string())?;
    Ok(format!("noema-memory-{}", URL_SAFE_NO_PAD.encode(bytes)))
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
    use crate::daemon::RuntimeModelProvider;
    use std::{future::Future, pin::Pin, sync::Arc};

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

    #[test]
    fn memory_proxy_config_prefers_memory_model_provider() {
        let providers = crate::daemon::RuntimeProviderMap::from([
            (
                "codex".to_string(),
                Arc::new(DefaultModelProvider("codex-default")) as Arc<dyn RuntimeModelProvider>,
            ),
            (
                "foundation_local".to_string(),
                Arc::new(DefaultModelProvider("foundation-default"))
                    as Arc<dyn RuntimeModelProvider>,
            ),
        ]);
        let settings = crate::MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: crate::MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: Some("foundation_local:default".to_string()),
            provider_kind: Some("foundation_local".to_string()),
            model_profile: Some("memory-profile".to_string()),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
        };

        let config = memory_model_proxy_config_from_settings(
            &settings,
            "codex",
            &providers,
            "secret".to_string(),
            test_system_error_logger(),
        )
        .expect("proxy config");

        assert_eq!(config.model_profile, "memory-profile");
        assert_eq!(
            config.reasoning_effort,
            Some(crate::provider::ReasoningEffort::Low)
        );
        assert_eq!(
            config
                .provider
                .default_tool_classification_model()
                .as_deref(),
            Some("foundation-default")
        );
    }

    #[test]
    fn memory_proxy_config_falls_back_to_default_provider_when_unset() {
        let providers = crate::daemon::RuntimeProviderMap::from([(
            "codex".to_string(),
            Arc::new(DefaultModelProvider("codex-default")) as Arc<dyn RuntimeModelProvider>,
        )]);
        let settings = crate::MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: crate::MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        };

        let config = memory_model_proxy_config_from_settings(
            &settings,
            "codex",
            &providers,
            "secret".to_string(),
            test_system_error_logger(),
        )
        .expect("proxy config");

        assert_eq!(config.model_profile, "codex-default");
        assert_eq!(
            config
                .provider
                .default_tool_classification_model()
                .as_deref(),
            Some("codex-default")
        );
    }

    #[derive(Debug)]
    struct DefaultModelProvider(&'static str);

    impl RuntimeModelProvider for DefaultModelProvider {
        fn default_tool_classification_model(&self) -> Option<String> {
            Some(self.0.to_string())
        }

        fn generate_streaming<'a>(
            &'a self,
            _request: crate::provider::GenerateRequest,
            _on_event: &'a mut (dyn FnMut(crate::provider::GenerateStreamEvent) + Send),
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            crate::provider::GenerateResponse,
                            crate::provider::ProviderError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Ok(crate::provider::GenerateResponse::final_text(
                    "ok", "test", "model",
                ))
            })
        }
    }

    fn test_system_error_logger() -> SystemErrorLogger {
        SystemErrorLogger::new(std::env::temp_dir().join("noema-runtime-host-test-errors.jsonl"))
    }
}
