//! Shared Noema runtime host used by daemon and desktop shells.

use noema_store::{NoemaStore, StoreConfig};

use crate::{
    DEFAULT_NOEMA_CONFIG_YAML, DaemonError,
    daemon::{
        CodexRuntimeHandle, CodexRuntimeSpawnConfig, ProviderAccountRuntimeAccess,
        TaskRuntimeHandle,
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
use noema_memory::{
    MemoryModelProxy, MemoryModelProxyConfig, MemoryRepositoryHandle, MemoryServiceAccessHandle,
    MemoryServiceMode, MemoryServicePaths, MnemosyneLifecycle, MnemosyneMemoryService,
    MnemosyneMemoryServiceAccess, memory_provider_selection_loader,
};
#[cfg(test)]
use noema_memory::{MemoryServiceSettingsRecord, SaveMemoryServiceSettings};
use noema_providers::{
    LocalModelActivationPersistenceHandle, LocalModelInstallationPersistenceHandle,
    LocalModelLifecyclePersistenceHandle, LocalModelManager, ProviderAccountOperationsHandle,
    ProviderAccountService, ProviderConfig, ProviderKind, ProviderRegistry, ProviderRegistryHandle,
    ProviderRouteResolverHandle, ProviderSelectionSnapshot, RegistryProviderRouteResolver,
    provider_account_instance_key,
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
    local_model_manager: LocalModelManager,
    mnemosyne: Option<MnemosyneLifecycle>,
    memory_startup_error: Option<String>,
    system_errors: SystemErrorLogger,
    paths: NoemaPaths,
    #[allow(dead_code)]
    runtime_events: crate::daemon::RuntimeEventRegistry,
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
        let configured_provider_model = provider.model().map(str::to_string);
        let paths = NoemaPaths::from_process_env()
            .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
        init_noema_home(
            &paths,
            Some(DEFAULT_NOEMA_CONFIG_YAML.as_bytes()),
            NoemaHomeInitOptions { force: false },
        )
        .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;

        let system_errors = SystemErrorLogger::from_paths(&paths);
        let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
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
            .ensure_default_openai_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        store
            .ensure_default_local_models_provider_account()
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
        let local_model_manager_config =
            provider.local_model_manager_config(local_model_runtime_root, system_errors.clone());
        let (default_provider_kind, default_model_profile, providers) =
            CodexRuntimeHandle::provider_map_from_config(
                provider,
                system_errors.clone(),
                provider_credentials.clone(),
            )
            .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        let provider_registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
        register_hosted_providers(&provider_registry, &providers)
            .map_err(RuntimeHostError::Runtime)?;
        // Foundation Models is process-local and credential-free. OpenAI is
        // constructed only from a complete configured secret. Codex retains
        // its account-service authentication state.
        store
            .update_provider_account_status(
                "provider_account:foundation_local:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        if providers.contains_key(ProviderKind::OpenAi.as_str()) {
            store
                .update_provider_account_status(
                    "provider_account:openai:default",
                    noema_providers::ProviderAccountStatus::Authenticated,
                    None,
                    None,
                )
                .await
                .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        }
        let local_model_installations: LocalModelInstallationPersistenceHandle =
            Arc::new(store.clone());
        let local_model_activation: LocalModelActivationPersistenceHandle = Arc::new(store.clone());
        let local_model_lifecycle: LocalModelLifecyclePersistenceHandle = Arc::new(store.clone());
        let local_model_manager = LocalModelManager::new(
            local_model_installations,
            local_model_activation,
            local_model_lifecycle,
            provider_registry.clone(),
            paths.clone(),
            local_model_manager_config,
        )
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        local_model_manager
            .reconstruct_persisted_instances()
            .await
            .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        let configured_default = if store
            .get_default_model_preference()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?
            .is_some()
        {
            // Once initialized, SQLite is authoritative. In particular, a
            // Settings activation may legitimately differ from stale YAML.
            store
                .default_provider_selection()
                .await
                .map_err(|source| RuntimeHostError::Store(source.to_string()))?
        } else {
            let configured_model_profile = default_model_profile
                .or(configured_provider_model)
                .ok_or_else(|| {
                    RuntimeHostError::Runtime(
                        "configured default provider has no concrete model profile".to_string(),
                    )
                })?;
            let mut selection = ProviderSelectionSnapshot::explicit(
                &default_provider_kind,
                model_provider_account_id(&default_provider_kind)?,
                configured_model_profile,
                None,
                Some("configured_default".to_string()),
            );
            let configured_key = if default_provider_kind == ProviderKind::LocalModels.as_str() {
                let active = local_model_manager
                    .installations()
                    .await
                    .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?
                    .into_iter()
                    .find(|installation| {
                        installation.is_active
                            && installation.model_id
                                == selection.model_profile.as_deref().unwrap_or_default()
                    })
                    .ok_or_else(|| {
                        RuntimeHostError::Runtime(
                            "configured local default has no active installation".to_string(),
                        )
                    })?;
                store
                    .update_provider_account_status(
                        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                        noema_providers::ProviderAccountStatus::Authenticated,
                        None,
                        None,
                    )
                    .await
                    .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
                active.provider_instance_key
            } else {
                provider_account_instance_key(&selection.provider_account_id)
                    .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?
            };
            selection.provider_instance_key = Some(configured_key);
            selection
        };
        let ready_configured_default = provider_registry
            .prove_ready_selection(configured_default.clone())
            .ok();
        store
            .initialize_missing_provider_selections(
                &configured_default,
                ready_configured_default.as_ref(),
            )
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
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
                    provider_registry.clone(),
                    memory_repository.clone(),
                    generate_memory_model_proxy_api_key().map_err(RuntimeHostError::Runtime)?,
                    system_errors.clone(),
                )
                .await
                {
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

        let runtime_events = crate::daemon::RuntimeEventRegistry::default();
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
        let primary_provider = registry_route_resolver(
            store.agent_provider_selection_loader("agent:primary"),
            provider_registry.clone(),
        );
        let default_provider = registry_route_resolver(
            store.default_provider_selection_loader(),
            provider_registry.clone(),
        );
        let progress_audit_provider = registry_route_resolver(
            store.auxiliary_provider_selection_loader(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID),
            provider_registry.clone(),
        );
        let web_summary_provider = registry_route_resolver(
            store.auxiliary_provider_selection_loader(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID),
            provider_registry.clone(),
        );
        let runtime = CodexRuntimeHandle::spawn(CodexRuntimeSpawnConfig {
            primary_provider,
            default_provider,
            progress_audit_provider,
            web_summary_provider,
            provider_registry,
            store: store.clone(),
            artifact_operations: artifact_operations.clone(),
            system_errors: system_errors.clone(),
            memory_operations: runtime_memory_operations,
            runtime_events: runtime_events.clone(),
            provider_accounts,
            capability_bindings: mcp_service.binding_source(),
            capability_invokers: Arc::from([mcp_service.invoker_registration()]),
        })
        .await
        .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
        mcp_completion.attach(runtime.clone());
        let task_runtime = TaskRuntimeHandle::start(
            store.clone(),
            runtime.clone(),
            local_model_manager.registry(),
            system_errors.clone(),
            runtime_events.clone(),
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
            local_model_manager,
            mnemosyne,
            memory_startup_error,
            system_errors,
            paths,
            runtime_events,
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

    /// Provider-owned local-model lifecycle and management control plane.
    #[must_use]
    pub fn local_model_manager(&self) -> &LocalModelManager {
        &self.local_model_manager
    }

    /// Runtime-only managed memory service startup error, when startup failed.
    #[must_use]
    pub fn memory_startup_error(&self) -> Option<&str> {
        self.memory_startup_error.as_deref()
    }

    /// Transport-neutral runtime event registry.
    pub(crate) fn runtime_events(&self) -> &crate::daemon::RuntimeEventRegistry {
        &self.runtime_events
    }

    /// Shut down runtime-owned work.
    pub async fn shutdown(self) {
        self.mcp_service.begin_shutdown();
        self.local_model_manager.begin_shutdown().await;
        self.task_runtime.shutdown().await;
        if let Some(mnemosyne) = self.mnemosyne {
            mnemosyne.shutdown().await;
        }
        self.runtime.shutdown().await;
        if let Err(error) = self.local_model_manager.shutdown().await {
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    "local_model_shutdown_failed",
                    "A local model process did not stop cleanly",
                )
                .with_error_chain([error.to_string()]),
            );
        }
        if !self.mcp_service.shutdown().await {
            self.system_errors.try_append(SystemErrorEvent::new(
                "mcp_shutdown_drain_timeout",
                "MCP work did not terminate before the shutdown deadline",
            ));
        }
        self.provider_account_service.shutdown().await;
    }
}

async fn memory_model_proxy_config_from_settings(
    provider_registry: ProviderRegistryHandle,
    memory_repository: MemoryRepositoryHandle,
    api_key: String,
    system_errors: SystemErrorLogger,
) -> Result<MemoryModelProxyConfig, String> {
    let settings = memory_repository
        .memory_service_settings()
        .await
        .map_err(|error| format!("memory model settings are unavailable: {error}"))?;
    let selection = noema_memory::memory_provider_selection(&settings)
        .map_err(|error| format!("memory model selection is unavailable: {error}"))?;
    let route_resolver = registry_route_resolver(
        memory_provider_selection_loader(memory_repository),
        provider_registry,
    );
    let model_profile = selection
        .model_profile
        .ok_or_else(|| "memory model selection has no model profile".to_string())?;
    Ok(MemoryModelProxyConfig {
        route_resolver,
        api_key,
        model_profile,
        system_errors: Some(system_errors),
    })
}

fn registry_route_resolver(
    loader: noema_providers::ProviderSelectionLoaderHandle,
    registry: ProviderRegistryHandle,
) -> ProviderRouteResolverHandle {
    Arc::new(RegistryProviderRouteResolver::new(loader, registry))
}

fn register_hosted_providers(
    registry: &ProviderRegistryHandle,
    providers: &std::collections::HashMap<String, noema_providers::ProviderHandle>,
) -> Result<(), String> {
    for (provider_kind, provider) in providers {
        let account_id =
            model_provider_account_id(provider_kind).map_err(|error| error.to_string())?;
        let key = provider_account_instance_key(account_id).map_err(|error| error.to_string())?;
        registry
            .register(key, provider.clone())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn model_provider_account_id(provider_kind: &str) -> Result<&'static str, RuntimeHostError> {
    match provider_kind {
        "codex" => Ok("provider_account:codex:default"),
        "openai" => Ok("provider_account:openai:default"),
        "foundation_local" => Ok("provider_account:foundation_local:default"),
        "local_models" => Ok(noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID),
        other => Err(RuntimeHostError::Runtime(format!(
            "unsupported configured model provider: {other}"
        ))),
    }
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
            provider_instance_key: None,
            model_profile: Some(noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string()),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
        };
        let store = crate::test_support::test_store().await;
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                "provider_account:foundation_local:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate foundation account");
        let ready_selection =
            crate::test_support::ready_provider_selection(ProviderSelectionSnapshot::explicit(
                "foundation_local",
                "provider_account:foundation_local:default",
                noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE,
                Some(noema_providers::ReasoningEffort::Low),
                Some("memory_proxy_test".to_string()),
            ));
        store
            .save_memory_service_settings_with_ready_selection(
                SaveMemoryServiceSettings {
                    mode: settings.mode,
                    base_url: settings.base_url.clone(),
                    port: settings.port,
                    provider_account_id: settings.provider_account_id.clone(),
                    provider_kind: settings.provider_kind.clone(),
                    model_profile: settings.model_profile.clone(),
                    reasoning_effort: settings.reasoning_effort,
                },
                &ready_selection,
            )
            .await
            .expect("memory settings");
        let registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
        register_hosted_providers(&registry, &providers).expect("register providers");

        let config = memory_model_proxy_config_from_settings(
            registry,
            Arc::new(store.clone()),
            "secret".to_string(),
            test_system_error_logger(),
        )
        .await
        .expect("proxy config");
        let route = config
            .route_resolver
            .resolve_route()
            .await
            .expect("memory route");

        assert_eq!(
            config.model_profile,
            noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE
        );
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
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate codex account");
        let ready_selection =
            crate::test_support::ready_provider_selection(ProviderSelectionSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "codex-memory",
                None,
                Some("memory_proxy_test".to_string()),
            ));
        store
            .save_memory_service_settings_with_ready_selection(
                SaveMemoryServiceSettings {
                    mode: MemoryServiceMode::Managed,
                    base_url: None,
                    port: None,
                    provider_account_id: Some("provider_account:codex:default".to_string()),
                    provider_kind: Some("codex".to_string()),
                    model_profile: Some("codex-memory".to_string()),
                    reasoning_effort: None,
                },
                &ready_selection,
            )
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
    async fn memory_proxy_config_uses_default_seeded_during_initialization() {
        let providers = crate::daemon::RuntimeProviderMap::from([(
            "codex".to_string(),
            Arc::new(DefaultModelProvider("codex-default")) as noema_providers::ProviderHandle,
        )]);
        let store = crate::test_support::test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let mut configured_default = ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "codex-default",
            None,
            Some("test_configured_default".to_string()),
        );
        configured_default.provider_instance_key = Some(
            provider_account_instance_key("provider_account:codex:default").expect("provider key"),
        );
        let registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
        register_hosted_providers(&registry, &providers).expect("register providers");
        let ready_selection = registry
            .prove_ready_selection(configured_default.clone())
            .expect("ready configured selection");
        store
            .initialize_missing_provider_selections(&configured_default, Some(&ready_selection))
            .await
            .expect("initialize selections");

        let config = memory_model_proxy_config_from_settings(
            registry,
            Arc::new(store),
            "secret".to_string(),
            test_system_error_logger(),
        )
        .await
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

    #[tokio::test]
    async fn memory_proxy_config_does_not_require_provider_readiness_at_startup() {
        let store = crate::test_support::test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let configured_default = ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "codex-default",
            None,
            Some("test_configured_default".to_string()),
        );
        let ready_selection =
            crate::test_support::ready_provider_selection(configured_default.clone());
        store
            .initialize_missing_provider_selections(
                ready_selection.selection(),
                Some(&ready_selection),
            )
            .await
            .expect("initialize selections");

        let config = memory_model_proxy_config_from_settings(
            Arc::new(ProviderRegistry::new()),
            Arc::new(store),
            "secret".to_string(),
            test_system_error_logger(),
        )
        .await
        .expect("proxy config");

        assert_eq!(config.model_profile, "codex-default");
        assert!(matches!(
            config.route_resolver.resolve_route().await,
            Err(noema_providers::ProviderRouteError::InstanceMissing { .. })
        ));
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
