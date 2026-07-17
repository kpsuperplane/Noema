//! Shared Noema runtime host used by daemon and desktop shells.

use crate::{
    DEFAULT_NOEMA_CONFIG_YAML, DaemonError, NoemaStore, StoreConfig,
    daemon::{
        CodexRuntimeHandle, CodexRuntimeSpawnConfig, LegacyProviderRoutes,
        ProviderAccountRuntimeAccess, TaskRuntimeHandle,
    },
    mcp_completion::RuntimeMcpAutofillCompletionBridge,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use noema_capabilities_mcp::{
    FilesystemMcpSecretStore, LocalMcpService, LocalMcpServiceConfig, McpControlPlaneHandle,
    McpHttpAuthorizationHandle, McpRepositoryHandle, McpSessionFactoryHandle,
    McpSessionFactoryRouter, StdioMcpSessionFactory, StreamableHttpMcpSessionFactory,
    SystemErrorMcpDiagnostics,
};
use noema_home::{
    NoemaHomeInitOptions, NoemaPathError, NoemaPaths, SystemErrorEvent, SystemErrorLogger,
    init_noema_home,
};
#[cfg(test)]
use noema_memory::SaveMemoryServiceSettings;
use noema_memory::{
    MemoryModelProxy, MemoryModelProxyConfig, MemoryRepositoryHandle, MemoryServiceAccessHandle,
    MemoryServiceMode, MemoryServicePaths, MemoryServiceSettingsRecord, MnemosyneLifecycle,
    MnemosyneMemoryService, MnemosyneMemoryServiceAccess, memory_provider_selection_loader,
};
use noema_providers::{
    DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
    DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, DEFAULT_TOOL_CLASSIFICATION_MODEL,
    ProviderAccountOperationsHandle, ProviderAccountService, ProviderConfig, erase_model_provider,
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;

/// Shared host state for Noema client surfaces.
pub struct NoemaRuntimeHost {
    runtime: CodexRuntimeHandle,
    task_runtime: TaskRuntimeHandle,
    store: NoemaStore,
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    provider_account_service: ProviderAccountService,
    provider_account_operations: ProviderAccountOperationsHandle,
    mcp_service: LocalMcpService,
    mcp_operations: McpControlPlaneHandle,
    memory_repository: MemoryRepositoryHandle,
    memory_service_access: MemoryServiceAccessHandle,
    mnemosyne: Option<MnemosyneLifecycle>,
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
        Self::start_with_local_model_runtime_root(provider, None).await
    }

    /// Start the shared host with an optional packaged llama.cpp resource root.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHostError`] when path setup, store startup, or runtime
    /// startup fails.
    pub async fn start_with_local_model_runtime_root(
        provider: ProviderConfig,
        local_model_runtime_root: Option<PathBuf>,
    ) -> Result<Self, RuntimeHostError> {
        let configured_provider_kind = provider.kind().as_str().to_string();
        let paths = NoemaPaths::from_process_env()
            .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
        init_noema_home(
            &paths,
            Some(DEFAULT_NOEMA_CONFIG_YAML.as_bytes()),
            NoemaHomeInitOptions { force: false },
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
        store
            .ensure_default_local_models_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        store
            .ensure_default_task_model_pool_settings(&configured_provider_kind)
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        let codex_oauth = match &provider {
            ProviderConfig::Codex(config) => config.oauth.clone(),
            _ => noema_providers::CodexOAuthConfig::default(),
        };
        let provider_account_service = ProviderAccountService::new_with_codex_oauth(
            paths.clone(),
            Arc::new(store.clone()),
            Arc::new(store.clone()),
            system_errors.clone(),
            codex_oauth,
        )
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        let provider_account_operations = provider_account_service.operations();
        let provider_credentials = provider_account_service.credentials();

        let provider = match provider {
            ProviderConfig::LocalModels(mut config) => {
                if config.runtime_root.is_none() {
                    config.runtime_root = local_model_runtime_root.clone();
                }
                if config.model_path.is_none() {
                    let installation = store
                        .get_installed_local_model(&config.default_model)
                        .await
                        .map_err(|source| RuntimeHostError::Store(source.to_string()))?
                        .ok_or_else(|| {
                            RuntimeHostError::Runtime(format!(
                                "local model `{}` is not installed",
                                config.default_model
                            ))
                        })?;
                    config.model_path = Some(
                        paths
                            .local_model_blob_path(installation.sha256.as_deref().ok_or_else(
                                || {
                                    RuntimeHostError::Runtime(format!(
                                        "installed local model `{}` has no verified digest",
                                        installation.model_id
                                    ))
                                },
                            )?)
                            .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?,
                    );
                    config.preferred_backend = Some(installation.backend);
                }
                ProviderConfig::LocalModels(config)
            }
            provider => provider,
        };

        let (default_provider_kind, mut providers, mut local_models_runtime) =
            CodexRuntimeHandle::provider_map_from_config(
                provider,
                system_errors.clone(),
                provider_credentials.clone(),
            )
            .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        if !providers.contains_key(noema_providers::ProviderKind::LocalModels.as_str())
            && let Some(installation) = store
                .list_local_model_installations()
                .await
                .map_err(|source| RuntimeHostError::Store(source.to_string()))?
                .into_iter()
                .find(|installation| {
                    installation.is_active
                        && installation.status
                            == noema_providers::LocalModelInstallationStatus::Installed
                })
        {
            let model_path = paths
                .local_model_blob_path(installation.sha256.as_deref().ok_or_else(|| {
                    RuntimeHostError::Runtime(format!(
                        "installed local model `{}` has no verified digest",
                        installation.model_id
                    ))
                })?)
                .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
            let provider =
                crate::LocalModelsProvider::new(noema_providers::LocalModelsProviderConfig {
                    default_model: installation.model_id,
                    model_path: Some(model_path),
                    preferred_backend: Some(installation.backend),
                    runtime_root: local_model_runtime_root.clone(),
                    context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
                    timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
                    startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
                    system_errors: Some(system_errors.clone()),
                })
                .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
            local_models_runtime = Some(provider.runtime().clone());
            providers.insert(
                noema_providers::ProviderKind::LocalModels
                    .as_str()
                    .to_string(),
                erase_model_provider(provider),
            );
        }
        if let Some(runtime) = &local_models_runtime
            && let Err(error) = runtime.retry().await
        {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "local_model_runtime_unavailable",
                    "The local model runtime could not start",
                )
                .with_error_chain([error.to_string()]),
            );
        }
        let provider_routes = LegacyProviderRoutes::new(providers.clone())
            .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        let memory_repository: MemoryRepositoryHandle = Arc::new(store.clone());
        let memory_settings = memory_repository
            .memory_service_settings()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        let external_memory_connection = (memory_settings.mode == MemoryServiceMode::External)
            .then(|| {
                memory_settings
                    .base_url
                    .clone()
                    .map(|base_url| noema_memory::MnemosyneConnection::new(base_url, None))
            })
            .flatten();
        let mut managed_memory_connection = None;
        let mut memory_startup_error = None;
        let memory_model_proxy = match memory_settings.mode {
            MemoryServiceMode::External => None,
            MemoryServiceMode::Managed => {
                match memory_model_proxy_config_from_settings(
                    &memory_settings,
                    &default_provider_kind,
                    &providers,
                    provider_routes.clone(),
                    memory_repository.clone(),
                    generate_memory_model_proxy_api_key().map_err(RuntimeHostError::Runtime)?,
                    system_errors.clone(),
                ) {
                    Ok(config) => match MemoryModelProxy::start(config).await {
                        Ok(proxy) => Some(proxy),
                        Err(error) => {
                            let error = error.to_string();
                            memory_startup_error = Some(error.clone());
                            system_errors.try_append(
                                SystemErrorEvent::new(
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
                            SystemErrorEvent::new(
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

        let memory_paths = MemoryServicePaths::from_noema_root(paths.root());
        let mnemosyne = if memory_settings.mode == MemoryServiceMode::Managed
            && memory_model_proxy.is_none()
            && memory_startup_error.is_some()
        {
            None
        } else {
            match MnemosyneLifecycle::start(
                &memory_paths.data_dir(),
                &memory_paths.runtime_dir(),
                &memory_settings,
                system_errors.clone(),
                memory_model_proxy,
            )
            .await
            {
                Ok(lifecycle) => {
                    if let Some(connection) = lifecycle.connection().cloned() {
                        managed_memory_connection = Some(connection);
                    }
                    Some(lifecycle)
                }
                Err(error) => {
                    memory_startup_error = Some(error.to_string());
                    system_errors.try_append(
                        SystemErrorEvent::new(
                            "mnemosyne_lifecycle_unavailable",
                            "Mnemosyne lifecycle is unavailable",
                        )
                        .with_error_chain([error.to_string()]),
                    );
                    None
                }
            }
        };
        let memory_service_access = MnemosyneMemoryServiceAccess::new(
            memory_repository.clone(),
            managed_memory_connection.clone(),
        )
        .into_handle();
        let runtime_memory_operations = managed_memory_connection
            .or(external_memory_connection)
            .map(|connection| {
                MnemosyneMemoryService::from_connection(Some(connection)).into_handle()
            });

        let subscriptions = crate::graphql::ConversationSubscriptionRegistry::default();
        let artifact_metadata: noema_artifacts::ArtifactMetadataStoreHandle =
            std::sync::Arc::new(store.clone());
        let artifact_operations: noema_artifacts::ArtifactOperationsHandle = std::sync::Arc::new(
            noema_artifacts::LocalArtifactService::new(paths.root(), artifact_metadata)
                .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?,
        );
        let provider_accounts = ProviderAccountRuntimeAccess::new(
            provider_account_operations.clone(),
            provider_credentials,
        );
        let mcp_repository: McpRepositoryHandle = Arc::new(store.clone());
        let mcp_secrets = Arc::new(FilesystemMcpSecretStore::new(paths.clone()));
        let mcp_diagnostics = SystemErrorMcpDiagnostics::new(system_errors.clone()).handle();
        let mcp_completion = RuntimeMcpAutofillCompletionBridge::new(system_errors.clone());
        let mcp_service = LocalMcpService::new(
            mcp_repository,
            mcp_secrets,
            mcp_diagnostics.clone(),
            Some(mcp_completion.handle()),
            LocalMcpServiceConfig::default(),
            |oauth| {
                let authorization: McpHttpAuthorizationHandle = Arc::new(oauth);
                let stdio: McpSessionFactoryHandle =
                    Arc::new(StdioMcpSessionFactory::new(Some(mcp_diagnostics.clone())));
                let streamable_http: McpSessionFactoryHandle =
                    Arc::new(StreamableHttpMcpSessionFactory::new(
                        Some(mcp_diagnostics),
                        Some(authorization),
                    ));
                Arc::new(McpSessionFactoryRouter::new(stdio, streamable_http))
            },
        )
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        let mcp_operations = mcp_service.operations();
        let runtime = CodexRuntimeHandle::spawn(CodexRuntimeSpawnConfig {
            default_provider_kind,
            provider_routes,
            store: store.clone(),
            artifact_operations: artifact_operations.clone(),
            system_errors: system_errors.clone(),
            memory_operations: runtime_memory_operations,
            task_subscriptions: subscriptions.clone(),
            provider_accounts,
            capability_bindings: mcp_service.binding_source(),
            capability_invokers: Arc::from([mcp_service.invoker_registration()]),
        })
        .await
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        mcp_completion.attach(runtime.clone());
        runtime
            .set_local_models_runtime_root(local_model_runtime_root)
            .await;
        if let Some(local_models_runtime) = local_models_runtime {
            runtime
                .attach_local_models_runtime(local_models_runtime)
                .await;
        }
        let task_runtime = TaskRuntimeHandle::start(
            store.clone(),
            runtime.clone(),
            system_errors.clone(),
            subscriptions.clone(),
        );

        Ok(Self {
            runtime,
            task_runtime,
            store,
            artifact_operations,
            provider_account_service,
            provider_account_operations,
            mcp_service,
            mcp_operations,
            memory_repository,
            memory_service_access,
            mnemosyne,
            memory_startup_error,
            system_errors,
            paths,
            subscriptions,
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

    /// Governed artifact operations shared by runtime and API consumers.
    pub(crate) fn artifact_operations(&self) -> &noema_artifacts::ArtifactOperationsHandle {
        &self.artifact_operations
    }

    /// Provider-owned account and authentication orchestration.
    #[must_use]
    pub(crate) fn provider_account_operations(&self) -> &ProviderAccountOperationsHandle {
        &self.provider_account_operations
    }

    /// MCP settings and setup control plane.
    #[must_use]
    pub(crate) fn mcp_operations(&self) -> &McpControlPlaneHandle {
        &self.mcp_operations
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

    /// Memory settings and article-cache repository.
    #[must_use]
    pub fn memory_repository(&self) -> &MemoryRepositoryHandle {
        &self.memory_repository
    }

    /// Request-scoped access to the configured memory service.
    #[must_use]
    pub fn memory_service_access(&self) -> &MemoryServiceAccessHandle {
        &self.memory_service_access
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
        self.mcp_service.begin_shutdown();
        self.task_runtime.shutdown().await;
        if let Some(mnemosyne) = self.mnemosyne {
            mnemosyne.shutdown().await;
        }
        self.runtime.shutdown().await;
        if !self.mcp_service.shutdown().await {
            self.system_errors.try_append(SystemErrorEvent::new(
                "mcp_shutdown_drain_timeout",
                "MCP work did not terminate before the shutdown deadline",
            ));
        }
        self.provider_account_service.shutdown().await;
    }
}

fn memory_model_proxy_config_from_settings(
    settings: &MemoryServiceSettingsRecord,
    default_provider_kind: &str,
    providers: &crate::daemon::RuntimeProviderMap,
    provider_routes: LegacyProviderRoutes,
    memory_repository: MemoryRepositoryHandle,
    api_key: String,
    system_errors: SystemErrorLogger,
) -> Result<MemoryModelProxyConfig, String> {
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
    let route_resolver = provider_routes.bind(memory_provider_selection_loader(
        memory_repository,
        default_provider_kind,
    ));
    Ok(MemoryModelProxyConfig {
        route_resolver,
        api_key,
        model_profile,
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
    use noema_providers::ProviderOperations;
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

    #[tokio::test]
    async fn memory_proxy_config_prefers_memory_model_provider() {
        let providers = crate::daemon::RuntimeProviderMap::from([
            (
                "codex".to_string(),
                Arc::new(DefaultModelProvider("codex-default")) as noema_providers::ProviderHandle,
            ),
            (
                "foundation_local".to_string(),
                Arc::new(DefaultModelProvider("foundation-default"))
                    as noema_providers::ProviderHandle,
            ),
        ]);
        let settings = MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: Some("provider_account:foundation_local:default".to_string()),
            provider_kind: Some("foundation_local".to_string()),
            model_profile: Some("memory-profile".to_string()),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
        };
        let store = crate::store::tests::test_store().await;
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .save_memory_service_settings(SaveMemoryServiceSettings {
                mode: settings.mode,
                base_url: settings.base_url.clone(),
                port: settings.port,
                provider_account_id: settings.provider_account_id.clone(),
                provider_kind: settings.provider_kind.clone(),
                model_profile: settings.model_profile.clone(),
                reasoning_effort: settings.reasoning_effort,
            })
            .await
            .expect("memory settings");
        let routes = LegacyProviderRoutes::new(providers.clone()).expect("provider routes");

        let config = memory_model_proxy_config_from_settings(
            &settings,
            "codex",
            &providers,
            routes,
            Arc::new(store.clone()),
            "secret".to_string(),
            test_system_error_logger(),
        )
        .expect("proxy config");
        let route = config
            .route_resolver
            .resolve_route()
            .await
            .expect("memory route");

        assert_eq!(config.model_profile, "memory-profile");
        assert_eq!(
            route.selection().reasoning_effort,
            Some(noema_providers::ReasoningEffort::Low)
        );
        assert_eq!(
            route
                .operations()
                .default_tool_classification_model()
                .as_deref(),
            Some("foundation-default")
        );

        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .save_memory_service_settings(SaveMemoryServiceSettings {
                mode: MemoryServiceMode::Managed,
                base_url: None,
                port: None,
                provider_account_id: Some("provider_account:codex:default".to_string()),
                provider_kind: Some("codex".to_string()),
                model_profile: Some("codex-memory".to_string()),
                reasoning_effort: None,
            })
            .await
            .expect("updated memory settings");
        let refreshed = config
            .route_resolver
            .resolve_route()
            .await
            .expect("refreshed memory route");
        assert_eq!(refreshed.selection().provider_kind, "codex");
        assert_eq!(
            refreshed.selection().model_profile.as_deref(),
            Some("codex-memory")
        );
        assert_eq!(
            refreshed
                .operations()
                .default_tool_classification_model()
                .as_deref(),
            Some("codex-default")
        );
    }

    #[tokio::test]
    async fn memory_proxy_config_falls_back_to_default_provider_when_unset() {
        let providers = crate::daemon::RuntimeProviderMap::from([(
            "codex".to_string(),
            Arc::new(DefaultModelProvider("codex-default")) as noema_providers::ProviderHandle,
        )]);
        let settings = MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        };
        let store = crate::store::tests::test_store().await;
        let routes = LegacyProviderRoutes::new(providers.clone()).expect("provider routes");

        let config = memory_model_proxy_config_from_settings(
            &settings,
            "codex",
            &providers,
            routes,
            Arc::new(store),
            "secret".to_string(),
            test_system_error_logger(),
        )
        .expect("proxy config");
        let route = config
            .route_resolver
            .resolve_route()
            .await
            .expect("memory route");

        assert_eq!(config.model_profile, "codex-default");
        assert_eq!(
            route
                .operations()
                .default_tool_classification_model()
                .as_deref(),
            Some("codex-default")
        );
    }

    #[derive(Debug)]
    struct DefaultModelProvider(&'static str);

    impl ProviderOperations for DefaultModelProvider {
        fn default_tool_classification_model(&self) -> Option<String> {
            Some(self.0.to_string())
        }

        fn generate_streaming<'a>(
            &'a self,
            _request: noema_providers::GenerateRequest,
            _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            noema_providers::GenerateResponse,
                            noema_providers::ProviderError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Ok(noema_providers::GenerateResponse::final_text(
                    "ok", "test", "model",
                ))
            })
        }
    }

    fn test_system_error_logger() -> SystemErrorLogger {
        SystemErrorLogger::new(std::env::temp_dir().join("noema-runtime-host-test-errors.jsonl"))
    }
}
