//! Concrete application assembly and dependency-ordered lifecycle.

mod lifecycle;

use noema_store::{NoemaStore, StoreConfig};

pub(crate) use lifecycle::StartupResources;

use crate::{
    Config, DEFAULT_NOEMA_CONFIG_YAML, HostConfig, HostServices, NoemaHost, OnboardingService,
    RuntimeHostError, mcp_completion::RuntimeMcpToolClassificationBridge,
    runtime_host::SystemErrorArtifactDiagnostics,
};
use noema_runtime::{
    RuntimeEventRegistry, RuntimeHandle, RuntimeSpawnConfig, TaskRuntimeHandle, WebBackendFuture,
    WebBackendRequest, WebBackendResolver, WebBackendResolverError,
};

use noema_capabilities::CompositeCapabilityBindingSource;
use noema_capabilities_mcp::{
    FilesystemMcpSecretStore, LocalMcpService, LocalMcpServiceConfig, McpHttpAuthorizationHandle,
    McpRepositoryHandle, McpSessionFactoryHandle, McpSessionFactoryRouter, StdioMcpSessionFactory,
    StreamableHttpMcpSessionFactory, SystemErrorMcpDiagnostics,
};
use noema_capability_adapters::AdapterCapabilityService;
use noema_home::{NoemaHomeInitOptions, NoemaPaths, SystemErrorLogger, init_noema_home};
use noema_memory::NativeMemory;
use noema_providers::{
    CodexProviderConfig, DEFAULT_FOUNDATION_LOCAL_PROFILE, EXA_FETCH_PROVIDER_ID,
    EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient, FoundationLocalProvider,
    FoundationLocalProviderConfig, LocalModelActivationPersistenceHandle,
    LocalModelInstallationPersistenceHandle, LocalModelLifecyclePersistenceHandle,
    LocalModelManager, OpenRouterProviderConfig, ProviderAccountOperationsHandle,
    ProviderAccountPersistenceHandle, ProviderAccountService, ProviderConfig, ProviderCredential,
    ProviderCredentialAccessHandle, ProviderError, ProviderHandle, ProviderKind, ProviderRegistry,
    ProviderRegistryHandle, ProviderSelectionSnapshot, RegistryProviderRouteResolver,
    WebBrowseBackendHandle, WebFetchBackendHandle, WebSearchBackendHandle,
    default_web_browse_backend, default_web_fetch_backend, default_web_search_backend,
    hosted_provider_from_config, provider_account_instance_key, provider_bootstrap_from_config,
};
use std::sync::Arc;

pub(crate) async fn start_from_process_env_with_local_model_runtime_root(
    local_model_runtime_root: Option<std::path::PathBuf>,
) -> Result<NoemaHost, RuntimeHostError> {
    let paths = initialize_process_home()?;
    let mut config = Config::load(None)?;
    if let Some(runtime_root) = local_model_runtime_root {
        config.local_model_runtime_root = Some(runtime_root);
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
        Ok((services, web_config)) => Ok(NoemaHost {
            services,
            web_config,
            lifecycle: resources,
        }),
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
    let web_config = config.web().clone();
    let HostConfig {
        provider,
        local_model_runtime_root,
        ..
    } = config.clone();
    let configured_provider_model = provider.model().map(str::to_string);
    let configured_reasoning_effort = provider.reasoning_effort();
    let adapter_service = AdapterCapabilityService::new(paths.clone());
    adapter_service.prepare_filesystem()?;
    let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path())).await?;
    reconcile_legacy_provider_placeholders(&store, &paths, &provider).await?;
    let adapter_snapshot = adapter_service.management_snapshot()?;
    store
        .reconcile_adapter_definitions(&adapter_snapshot.definitions.projections())
        .await?;
    store
        .reconcile_adapter_connections(&adapter_snapshot.connections.projections())
        .await?;
    let provider_registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
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
    resources.provider_accounts = Some(provider_account_service.clone());
    let provider_account_operations = provider_account_service.operations();
    let provider_credentials = provider_account_service.credentials();
    let local_model_manager_config =
        provider.local_model_manager_config(local_model_runtime_root, system_errors.clone());
    let provider_account_persistence: ProviderAccountPersistenceHandle = Arc::new(store.clone());
    let (default_provider_kind, default_model_profile, providers) = provider_map_from_config(
        provider,
        system_errors.clone(),
        provider_credentials.clone(),
        provider_account_persistence,
        provider_account_operations.clone(),
    )?;
    register_hosted_providers(&provider_registry, &providers)?;
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
    resources.local_models = Some(local_model_manager.clone());
    local_model_manager
        .reconstruct_persisted_instances()
        .await?;
    if store.get_default_model_preference().await?.is_none() {
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
            configured_reasoning_effort,
            Some("configured_default".to_string()),
        );
        let configured_key = if default_provider_kind == ProviderKind::LocalModels.as_str() {
            let active =
                local_model_manager
                    .installations()
                    .await?
                    .into_iter()
                    .find(|installation| {
                        installation.is_active
                            && installation.model_id
                                == selection.model_profile.as_deref().unwrap_or_default()
                    });
            if let Some(active) = active {
                store.ensure_default_local_models_provider_account().await?;
                Some(active.provider_instance_key)
            } else {
                None
            }
        } else {
            let account = match default_provider_kind.as_str() {
                "openai" => Some(store.ensure_default_openai_provider_account().await?),
                "foundation_local" => {
                    let available = FoundationLocalProvider::new(FoundationLocalProviderConfig {
                        default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
                        bridge_path: None,
                        system_errors: Some(system_errors.clone()),
                    })?
                    .probe_availability()
                    .await
                    .is_ok();
                    if available {
                        Some(
                            store
                                .ensure_default_foundation_local_provider_account()
                                .await?,
                        )
                    } else {
                        None
                    }
                }
                "codex" | "openrouter" => {
                    store
                        .get_provider_account(model_provider_account_id(&default_provider_kind)?)
                        .await?
                }
                _ => None,
            };
            if let Some(account) = account {
                store
                    .update_provider_account_status(
                        &account.provider_account_id,
                        noema_providers::ProviderAccountStatus::Authenticated,
                        None,
                        None,
                    )
                    .await?;
                Some(provider_account_instance_key(
                    &selection.provider_account_id,
                )?)
            } else {
                None
            }
        };
        if let Some(configured_key) = configured_key {
            selection.provider_instance_key = Some(configured_key);
            let ready_configured_default = provider_registry
                .prove_ready_selection(selection.clone())
                .ok();
            store
                .initialize_missing_provider_selections(
                    &selection,
                    ready_configured_default.as_ref(),
                )
                .await?;
        }
    }
    let old_memory_root = paths.root().join("mnemosyne");
    if old_memory_root.exists() {
        std::fs::remove_dir_all(&old_memory_root).map_err(|error| {
            RuntimeHostError::Composition(format!(
                "could not remove old mnemosyne directory: {error}"
            ))
        })?;
    }
    let native_memory = NativeMemory::new(
        paths.root().join("memory/human"),
        paths.root().join("system/indexes/memory.sqlite3"),
    );
    native_memory.initialize().map_err(|error| {
        RuntimeHostError::Composition(format!("native memory startup failed: {error}"))
    })?;
    let runtime_events = RuntimeEventRegistry::default();
    let artifact_metadata: noema_artifacts::ArtifactMetadataStoreHandle =
        std::sync::Arc::new(store.clone());
    let artifact_operations: noema_artifacts::ArtifactOperationsHandle = std::sync::Arc::new(
        noema_artifacts::LocalArtifactService::new(paths.root(), artifact_metadata)?,
    );
    let web_backends = Arc::new(HostWebBackendResolver {
        provider_accounts: provider_account_operations.clone(),
        credentials: provider_credentials,
        default_search: default_web_search_backend(),
        default_fetch: default_web_fetch_backend(),
        default_browse: default_web_browse_backend(),
    });
    let mcp_repository: McpRepositoryHandle = Arc::new(store.clone());
    let mcp_secrets = Arc::new(FilesystemMcpSecretStore::new(paths.clone()));
    let mcp_diagnostics = SystemErrorMcpDiagnostics::new(system_errors.clone()).handle();
    let mcp_completion = RuntimeMcpToolClassificationBridge::new(system_errors.clone());
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
    resources.mcp = Some(mcp_service.clone());
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
        store.auxiliary_provider_selection_loader(
            noema_store::AuxiliaryModelTask::ToolProgressAudit,
        ),
        provider_registry.clone(),
    );
    let action_reviewer_provider = registry_route_resolver(
        store.auxiliary_provider_selection_loader(noema_store::AuxiliaryModelTask::ActionReviewer),
        provider_registry.clone(),
    );
    let web_summary_provider = registry_route_resolver(
        store.auxiliary_provider_selection_loader(
            noema_store::AuxiliaryModelTask::WebFetchSummarizer,
        ),
        provider_registry.clone(),
    );
    let capability_bindings = Arc::new(CompositeCapabilityBindingSource::new([
        mcp_service.binding_source(),
        adapter_service.binding_source(),
    ]));
    let capability_invokers: Arc<[noema_capabilities::CapabilityInvokerRegistration]> =
        Arc::from([
            mcp_service.invoker_registration(),
            adapter_service.invoker_registration(),
        ]);
    let runtime = RuntimeHandle::spawn(RuntimeSpawnConfig {
        noema_paths: paths.clone(),
        primary_provider,
        default_provider,
        progress_audit_provider,
        action_reviewer_provider,
        web_summary_provider,
        provider_registry: provider_registry.clone(),
        store: store.clone(),
        artifact_operations: artifact_operations.clone(),
        system_errors: system_errors.clone(),
        native_memory: Some(native_memory.clone()),
        runtime_events: runtime_events.clone(),
        web_backends,
        capability_bindings,
        capability_invokers,
    })
    .await?;
    resources.runtime = Some(runtime.clone());
    mcp_completion.attach(runtime.clone());
    mcp_service.resume_pending_tool_classification().await;
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        local_model_manager.registry(),
        system_errors.clone(),
        runtime_events.clone(),
    );
    resources.task_runtime = Some(task_runtime);

    let onboarding = OnboardingService::new(
        store.clone(),
        provider_account_operations.clone(),
        local_model_manager.clone(),
    );
    let artifact_diagnostics = Arc::new(SystemErrorArtifactDiagnostics::new(system_errors.clone()));
    let services = HostServices {
        noema_paths: paths,
        runtime: runtime.clone(),
        store,
        artifact_operations,
        artifact_diagnostics,
        provider_account_operations,
        mcp_operations,
        adapter_operations: adapter_service,
        local_model_manager: local_model_manager.clone(),
        provider_registry,
        onboarding,
        native_memory,
        runtime_events,
    };
    Ok((services, web_config))
}

async fn reconcile_legacy_provider_placeholders(
    store: &NoemaStore,
    paths: &NoemaPaths,
    configured_provider: &ProviderConfig,
) -> Result<(), RuntimeHostError> {
    for (provider_account_id, provider_kind) in [
        ("provider_account:codex:default", "codex"),
        ("provider_account:openai:default", "openai"),
        (
            "provider_account:foundation_local:default",
            "foundation_local",
        ),
        ("provider_account:local_models:default", "local_models"),
    ] {
        let account_home = paths.provider_account_home(provider_kind, "default");
        if provider_kind == "codex" && account_home.join("codex_tokens.json").exists() {
            continue;
        }
        if provider_kind == "openai" && configured_provider.kind() == ProviderKind::OpenAi {
            continue;
        }
        if !store
            .delete_unused_legacy_provider_account(provider_account_id, provider_kind)
            .await?
        {
            continue;
        }
        if account_home.is_dir()
            && account_home
                .read_dir()
                .map_err(|error| RuntimeHostError::Composition(error.to_string()))?
                .next()
                .is_none()
        {
            std::fs::remove_dir(&account_home)
                .map_err(|error| RuntimeHostError::Composition(error.to_string()))?;
        }
    }
    Ok(())
}

fn registry_route_resolver(
    loader: noema_providers::ProviderSelectionLoaderHandle,
    registry: ProviderRegistryHandle,
) -> RegistryProviderRouteResolver {
    RegistryProviderRouteResolver::new(loader, registry)
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
    provider_accounts: ProviderAccountPersistenceHandle,
    provider_account_operations: ProviderAccountOperationsHandle,
) -> Result<ConfiguredProviderMap, ProviderError> {
    let bootstrap = provider_bootstrap_from_config(
        provider_config,
        provider_credentials.clone(),
        Some(provider_accounts.clone()),
        Some(provider_account_operations.clone()),
        system_errors.clone(),
    )?;
    let default_provider_kind = bootstrap.default_provider_kind;
    let default_model_profile = bootstrap.default_model_profile;
    let mut providers = std::collections::HashMap::new();
    if let Some((provider_kind, provider)) = bootstrap.hosted_provider {
        providers.insert(provider_kind, provider);
    }
    if !providers.contains_key("codex") {
        let (provider_kind, provider) = hosted_provider_from_config(
            ProviderConfig::Codex(CodexProviderConfig::default()),
            provider_credentials.clone(),
            Some(provider_accounts.clone()),
            Some(provider_account_operations.clone()),
            system_errors.clone(),
        )?;
        providers.insert(provider_kind, provider);
    }
    if !providers.contains_key("foundation_local") {
        let (provider_kind, provider) = hosted_provider_from_config(
            ProviderConfig::FoundationLocal(FoundationLocalProviderConfig {
                default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
                bridge_path: None,
                system_errors: None,
            }),
            provider_credentials.clone(),
            Some(provider_accounts.clone()),
            Some(provider_account_operations.clone()),
            system_errors.clone(),
        )?;
        providers.insert(provider_kind, provider);
    }
    if !providers.contains_key("openrouter") {
        let (provider_kind, provider) = hosted_provider_from_config(
            ProviderConfig::OpenRouter(OpenRouterProviderConfig::default()),
            provider_credentials,
            Some(provider_accounts),
            Some(provider_account_operations),
            system_errors,
        )?;
        providers.insert(provider_kind, provider);
    }
    Ok((default_provider_kind, default_model_profile, providers))
}

#[derive(Clone)]
struct HostWebBackendResolver {
    provider_accounts: ProviderAccountOperationsHandle,
    credentials: ProviderCredentialAccessHandle,
    default_search: WebSearchBackendHandle,
    default_fetch: WebFetchBackendHandle,
    default_browse: WebBrowseBackendHandle,
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
                        .api_key("exa", &request.provider_account_id)
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
                        .api_key("exa", &request.provider_account_id)
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

    fn resolve_browse(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebBrowseBackendHandle> {
        let default_browse = self.default_browse.clone();
        Box::pin(async move {
            match request.provider_kind.as_str() {
                noema_providers::OBSCURA_BROWSER_PROVIDER_ID => Ok(default_browse),
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
        "openrouter" => Ok("provider_account:openrouter:default"),
        "foundation_local" => Ok("provider_account:foundation_local:default"),
        "local_models" => Ok(noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID),
        other => Err(RuntimeHostError::Composition(format!(
            "unsupported configured model provider: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests;
