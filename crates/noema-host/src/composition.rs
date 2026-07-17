//! Concrete application assembly and dependency-ordered lifecycle.

mod lifecycle;

use noema_store::{NoemaStore, StoreConfig};

pub(crate) use lifecycle::HostLifecycle;
use lifecycle::StartupResources;

use crate::{
    Config, ConfigOverrides, DEFAULT_NOEMA_CONFIG_YAML, HostConfig, HostServices, NoemaHost,
    OnboardingService, RuntimeHostError, mcp_completion::RuntimeMcpAutofillCompletionBridge,
    runtime_host::SystemErrorArtifactDiagnostics,
};
use noema_runtime::{
    RuntimeEventRegistry, RuntimeHandle, RuntimeSpawnConfig, TaskRuntimeHandle, WebBackendFuture,
    WebBackendRequest, WebBackendResolver, WebBackendResolverError,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use noema_capabilities_mcp::{
    FilesystemMcpSecretStore, LocalMcpService, LocalMcpServiceConfig, McpHttpAuthorizationHandle,
    McpRepositoryHandle, McpSessionFactoryHandle, McpSessionFactoryRouter, StdioMcpSessionFactory,
    StreamableHttpMcpSessionFactory, SystemErrorMcpDiagnostics,
};
use noema_home::{
    NoemaHomeInitOptions, NoemaPaths, SystemErrorEvent, SystemErrorLogger, init_noema_home,
};
use noema_memory::{
    MemoryModelProxy, MemoryModelProxyConfig, MemoryRepositoryHandle, MemoryServiceMode,
    MemoryServicePaths, MnemosyneLifecycle, MnemosyneMemoryService, MnemosyneMemoryServiceAccess,
    memory_provider_selection_loader,
};
#[cfg(test)]
use noema_memory::{MemoryServiceSettingsRecord, SaveMemoryServiceSettings};
use noema_providers::{
    CodexProviderConfig, DEFAULT_FOUNDATION_LOCAL_PROFILE, EXA_FETCH_PROVIDER_ID,
    EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient, FoundationLocalProviderConfig,
    LocalModelActivationPersistenceHandle, LocalModelInstallationPersistenceHandle,
    LocalModelLifecyclePersistenceHandle, LocalModelManager, ProviderAccountOperationsHandle,
    ProviderAccountService, ProviderConfig, ProviderCredential, ProviderCredentialAccessHandle,
    ProviderError, ProviderHandle, ProviderKind, ProviderRegistry, ProviderRegistryHandle,
    ProviderRouteResolverHandle, ProviderSelectionSnapshot, RegistryProviderRouteResolver,
    WebFetchBackendHandle, WebSearchBackendHandle, default_web_fetch_backend,
    default_web_search_backend, hosted_provider_from_config, provider_account_instance_key,
    provider_bootstrap_from_config,
};
use ring::rand::{SecureRandom, SystemRandom};
use std::sync::Arc;

pub(crate) async fn start_from_process_env_with_local_model_runtime_root(
    local_model_runtime_root: Option<std::path::PathBuf>,
) -> Result<NoemaHost, RuntimeHostError> {
    let paths = initialize_process_home()?;
    let mut config = Config::load(None, ConfigOverrides::default())?;
    if let Some(runtime_root) = local_model_runtime_root {
        config = config.with_local_model_runtime_root(runtime_root);
    }
    assemble(config, paths).await
}

pub(crate) async fn start_from_loaded_config(
    config: HostConfig,
) -> Result<NoemaHost, RuntimeHostError> {
    let paths = initialize_process_home()?;
    assemble(config, paths).await
}

fn initialize_process_home() -> Result<NoemaPaths, RuntimeHostError> {
    let paths = NoemaPaths::from_process_env()?;
    initialize_home(&paths)?;
    Ok(paths)
}

fn initialize_home(paths: &NoemaPaths) -> Result<(), RuntimeHostError> {
    init_noema_home(
        paths,
        Some(DEFAULT_NOEMA_CONFIG_YAML.as_bytes()),
        NoemaHomeInitOptions { force: false },
    )
    .map(|_| ())
    .map_err(RuntimeHostError::from)
}

async fn assemble(config: HostConfig, paths: NoemaPaths) -> Result<NoemaHost, RuntimeHostError> {
    let system_errors = SystemErrorLogger::from_paths(&paths);
    let mut resources = StartupResources::new(system_errors.clone());
    match assemble_services(config, paths, system_errors, &mut resources).await {
        Ok((services, web_config)) => Ok(NoemaHost::assembled(
            services,
            web_config,
            HostLifecycle::new(resources),
        )),
        Err(error) => {
            resources.shutdown().await;
            Err(error)
        }
    }
}

async fn assemble_services(
    config: HostConfig,
    paths: NoemaPaths,
    system_errors: SystemErrorLogger,
    resources: &mut StartupResources,
) -> Result<(HostServices, crate::WebConfig), RuntimeHostError> {
    let HostConfig {
        provider,
        local_model_runtime_root,
        ..
    } = config.clone();
    let configured_provider_model = provider.model().map(str::to_string);
    let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path())).await?;
    store.ensure_default_provider_account().await?;
    store
        .ensure_default_foundation_local_provider_account()
        .await?;
    store.ensure_default_openai_provider_account().await?;
    store.ensure_default_local_models_provider_account().await?;
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
    )?;
    resources.retain_provider_accounts(provider_account_service.clone());
    let provider_account_operations = provider_account_service.operations();
    let provider_credentials = provider_account_service.credentials();
    let local_model_manager_config =
        provider.local_model_manager_config(local_model_runtime_root, system_errors.clone());
    let (default_provider_kind, default_model_profile, providers) = provider_map_from_config(
        provider,
        system_errors.clone(),
        provider_credentials.clone(),
    )?;
    let provider_registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
    register_hosted_providers(&provider_registry, &providers)?;
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
        .await?;
    if providers.contains_key(ProviderKind::OpenAi.as_str()) {
        store
            .update_provider_account_status(
                "provider_account:openai:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await?;
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
    )?;
    resources.retain_local_models(local_model_manager.clone());
    local_model_manager
        .reconstruct_persisted_instances()
        .await?;
    let configured_default = if store.get_default_model_preference().await?.is_some() {
        // Once initialized, SQLite is authoritative. In particular, a
        // Settings activation may legitimately differ from stale YAML.
        store.default_provider_selection().await?
    } else {
        let configured_model_profile = default_model_profile
            .or(configured_provider_model)
            .ok_or_else(|| {
                RuntimeHostError::Composition(
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
                .await?
                .into_iter()
                .find(|installation| {
                    installation.is_active
                        && installation.model_id
                            == selection.model_profile.as_deref().unwrap_or_default()
                })
                .ok_or_else(|| {
                    RuntimeHostError::ConfiguredDefault(
                        noema_store::StoreError::ConfiguredDefaultUnresolvable {
                            reason: "configured local default has no active installation"
                                .to_string(),
                        },
                    )
                })?;
            store
                .update_provider_account_status(
                    noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                    noema_providers::ProviderAccountStatus::Authenticated,
                    None,
                    None,
                )
                .await?;
            active.provider_instance_key
        } else {
            provider_account_instance_key(&selection.provider_account_id)?
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
        .await?;
    let memory_repository: MemoryRepositoryHandle = Arc::new(store.clone());
    let memory_settings = store.memory_service_settings().await?;
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
            let proxy_config = match generate_memory_model_proxy_api_key() {
                Ok(api_key) => {
                    memory_model_proxy_config_from_settings(
                        provider_registry.clone(),
                        memory_repository.clone(),
                        api_key,
                        system_errors.clone(),
                    )
                    .await
                }
                Err(error) => Err(error),
            };
            match proxy_config {
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
    if let Some(mnemosyne) = mnemosyne {
        resources.retain_mnemosyne(mnemosyne);
    }
    let memory_service_access = MnemosyneMemoryServiceAccess::new(
        memory_repository.clone(),
        managed_memory_connection.clone(),
    )
    .into_handle();
    let runtime_memory_operations = managed_memory_connection
        .or(external_memory_connection)
        .map(|connection| MnemosyneMemoryService::from_connection(Some(connection)).into_handle());

    let runtime_events = RuntimeEventRegistry::default();
    let artifact_metadata: noema_artifacts::ArtifactMetadataStoreHandle =
        std::sync::Arc::new(store.clone());
    let artifact_operations: noema_artifacts::ArtifactOperationsHandle = std::sync::Arc::new(
        noema_artifacts::LocalArtifactService::new(paths.root(), artifact_metadata)?,
    );
    let web_backends = Arc::new(HostWebBackendResolver::new(
        provider_account_operations.clone(),
        provider_credentials,
    ));
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
            let streamable_http: McpSessionFactoryHandle = Arc::new(
                StreamableHttpMcpSessionFactory::new(Some(mcp_diagnostics), Some(authorization)),
            );
            Arc::new(McpSessionFactoryRouter::new(stdio, streamable_http))
        },
    )?;
    resources.retain_mcp(mcp_service.clone());
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
    let capability_invokers: Arc<[noema_capabilities::CapabilityInvokerRegistration]> =
        Arc::from([mcp_service.invoker_registration()]);
    let runtime = RuntimeHandle::spawn(RuntimeSpawnConfig {
        primary_provider,
        default_provider,
        progress_audit_provider,
        web_summary_provider,
        provider_registry: provider_registry.clone(),
        store: store.clone(),
        artifact_operations: artifact_operations.clone(),
        system_errors: system_errors.clone(),
        memory_operations: runtime_memory_operations,
        runtime_events: runtime_events.clone(),
        web_backends,
        capability_bindings: mcp_service.binding_source(),
        capability_invokers,
    })
    .await?;
    resources.retain_runtime(runtime.clone());
    mcp_completion.attach(runtime.clone());
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        local_model_manager.registry(),
        system_errors.clone(),
        runtime_events.clone(),
    );
    resources.retain_task_runtime(task_runtime);

    let onboarding = OnboardingService::new(
        store.clone(),
        provider_account_operations.clone(),
        local_model_manager.clone(),
    );
    let artifact_diagnostics = Arc::new(SystemErrorArtifactDiagnostics::new(system_errors.clone()));
    let services = HostServices::new(
        runtime.clone(),
        store,
        artifact_operations,
        artifact_diagnostics,
        provider_account_operations,
        mcp_operations,
        local_model_manager.clone(),
        provider_registry,
        memory_repository,
        memory_service_access,
        onboarding,
        memory_startup_error,
        runtime_events,
    );
    let web_config = config.web().clone();

    Ok((services, web_config))
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

type ConfiguredProviderMap = (
    String,
    Option<String>,
    std::collections::HashMap<String, ProviderHandle>,
);

fn provider_map_from_config(
    provider_config: ProviderConfig,
    system_errors: SystemErrorLogger,
    provider_credentials: ProviderCredentialAccessHandle,
) -> Result<ConfiguredProviderMap, ProviderError> {
    let bootstrap = provider_bootstrap_from_config(
        provider_config,
        provider_credentials.clone(),
        system_errors.clone(),
    )?;
    let default_provider_kind = bootstrap.default_provider_kind().to_string();
    let default_model_profile = bootstrap.default_model_profile().map(str::to_string);
    let mut providers = std::collections::HashMap::new();
    if let Some((provider_kind, provider)) = bootstrap.into_hosted_provider() {
        providers.insert(provider_kind, provider);
    }
    if !providers.contains_key("codex") {
        let (provider_kind, provider) = hosted_provider_from_config(
            ProviderConfig::Codex(CodexProviderConfig::default()),
            provider_credentials.clone(),
            system_errors.clone(),
        )?;
        providers.insert(provider_kind, provider);
    }
    if !providers.contains_key("foundation_local") {
        let (provider_kind, provider) = hosted_provider_from_config(
            ProviderConfig::FoundationLocal(default_foundation_local_config()),
            provider_credentials,
            system_errors,
        )?;
        providers.insert(provider_kind, provider);
    }
    Ok((default_provider_kind, default_model_profile, providers))
}

fn default_foundation_local_config() -> FoundationLocalProviderConfig {
    FoundationLocalProviderConfig {
        default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
        bridge_path: None,
        system_errors: None,
    }
}

#[derive(Clone)]
struct HostWebBackendResolver {
    provider_accounts: ProviderAccountOperationsHandle,
    credentials: ProviderCredentialAccessHandle,
    default_search: WebSearchBackendHandle,
    default_fetch: WebFetchBackendHandle,
}

impl HostWebBackendResolver {
    fn new(
        provider_accounts: ProviderAccountOperationsHandle,
        credentials: ProviderCredentialAccessHandle,
    ) -> Self {
        Self {
            provider_accounts,
            credentials,
            default_search: default_web_search_backend(),
            default_fetch: default_web_fetch_backend(),
        }
    }
}

impl std::fmt::Debug for HostWebBackendResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostWebBackendResolver")
            .field("provider_accounts", &"[CONFIGURED]")
            .field("credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl WebBackendResolver for HostWebBackendResolver {
    fn resolve_search(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebSearchBackendHandle> {
        let credentials = self.credentials.clone();
        let default_search = self.default_search.clone();
        Box::pin(async move {
            match request.provider_kind.as_str() {
                noema_providers::DUCKDUCKGO_PUBLIC_PROVIDER_ID => Ok(default_search),
                EXA_SEARCH_PROVIDER_ID => {
                    let api_key = credentials
                        .exa_api_key(&request.provider_account_id)
                        .await
                        .map(ProviderCredential::into_secret)
                        .map_err(|_| WebBackendResolverError::Unauthenticated)?;
                    let provider = ExaSearchClient::new(api_key)
                        .map_err(|_| WebBackendResolverError::Unavailable)?;
                    Ok(WebSearchBackendHandle::new(provider))
                }
                _ => Err(WebBackendResolverError::Unavailable),
            }
        })
    }

    fn resolve_fetch(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebFetchBackendHandle> {
        let credentials = self.credentials.clone();
        let default_fetch = self.default_fetch.clone();
        Box::pin(async move {
            match request.provider_kind.as_str() {
                noema_providers::DIRECT_HTTP_PROVIDER_ID => Ok(default_fetch),
                EXA_FETCH_PROVIDER_ID => {
                    let api_key = credentials
                        .exa_api_key(&request.provider_account_id)
                        .await
                        .map(ProviderCredential::into_secret)
                        .map_err(|_| WebBackendResolverError::Unauthenticated)?;
                    let provider = ExaFetchClient::new(api_key)
                        .map_err(|_| WebBackendResolverError::Unavailable)?;
                    Ok(WebFetchBackendHandle::new(provider))
                }
                _ => Err(WebBackendResolverError::Unavailable),
            }
        })
    }

    fn record_auth_failure(
        &self,
        provider_account_id: String,
        credential_revision: u64,
    ) -> WebBackendFuture<'_, ()> {
        let provider_accounts = self.provider_accounts.clone();
        Box::pin(async move {
            provider_accounts
                .record_auth_failure(&provider_account_id, credential_revision)
                .await
                .map(|_| ())
                .map_err(|_| WebBackendResolverError::Unavailable)
        })
    }
}

fn register_hosted_providers(
    registry: &ProviderRegistryHandle,
    providers: &std::collections::HashMap<String, noema_providers::ProviderHandle>,
) -> Result<(), RuntimeHostError> {
    for (provider_kind, provider) in providers {
        let account_id = model_provider_account_id(provider_kind)?;
        let key = provider_account_instance_key(account_id)?;
        registry.register(key, provider.clone())?;
    }
    Ok(())
}

fn model_provider_account_id(provider_kind: &str) -> Result<&'static str, RuntimeHostError> {
    match provider_kind {
        "codex" => Ok("provider_account:codex:default"),
        "openai" => Ok("provider_account:openai:default"),
        "foundation_local" => Ok("provider_account:foundation_local:default"),
        "local_models" => Ok(noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID),
        other => Err(RuntimeHostError::Composition(format!(
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

#[cfg(test)]
mod tests;
