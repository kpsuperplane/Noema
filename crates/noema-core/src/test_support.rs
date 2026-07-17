//! Test-only composition helpers shared across runtime and API tests.

use std::{collections::HashMap, sync::Arc};

use tempfile::TempDir;

#[derive(Debug)]
struct ReadyTestProvider;

impl noema_providers::ProviderOperations for ReadyTestProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: noema_providers::GenerateRequest,
        _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
    ) -> noema_providers::ProviderOperationFuture<'a, noema_providers::GenerateResponse> {
        Box::pin(async {
            Ok(noema_providers::GenerateResponse::final_text(
                "ready",
                "core-test",
                "core-test",
            ))
        })
    }
}

pub(crate) fn test_paths() -> noema_home::NoemaPaths {
    let home = TempDir::new().expect("temp Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("test paths");
    std::mem::forget(home);
    paths
}

pub(crate) fn system_error_logger() -> noema_home::SystemErrorLogger {
    noema_home::SystemErrorLogger::from_paths(&test_paths())
}

pub(crate) fn local_model_manager(
    store: &noema_store::NoemaStore,
    paths: noema_home::NoemaPaths,
) -> noema_providers::LocalModelManager {
    let installations: noema_providers::LocalModelInstallationPersistenceHandle =
        Arc::new(store.clone());
    let activation: noema_providers::LocalModelActivationPersistenceHandle =
        Arc::new(store.clone());
    let lifecycle: noema_providers::LocalModelLifecyclePersistenceHandle = Arc::new(store.clone());
    noema_providers::LocalModelManager::new(
        installations,
        activation,
        lifecycle,
        Arc::new(noema_providers::ProviderRegistry::new()),
        paths.clone(),
        noema_providers::LocalModelManagerConfig {
            runtime_root: None,
            context_window_tokens: noema_providers::DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: Some(noema_home::SystemErrorLogger::from_paths(&paths)),
        },
    )
    .expect("test local-model manager")
}

pub(crate) async fn test_store() -> noema_store::NoemaStore {
    noema_store::test_support::open_ephemeral_store()
        .await
        .expect("open ephemeral store")
}

pub(crate) async fn test_store_for_paths(
    paths: &noema_home::NoemaPaths,
) -> noema_store::NoemaStore {
    noema_store::NoemaStore::open(&noema_store::StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("open store")
}

pub(crate) fn ready_provider_selection(
    selection: noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderReadySelection {
    let registry = noema_providers::ProviderRegistry::new();
    ready_provider_selection_in_registry(selection, &registry)
}

pub(crate) fn ready_provider_selection_in_registry(
    mut selection: noema_providers::ProviderSelectionSnapshot,
    registry: &noema_providers::ProviderRegistry,
) -> noema_providers::ProviderReadySelection {
    if selection.provider_instance_key.is_none()
        && selection.provider_kind != noema_providers::ProviderKind::LocalModels.as_str()
    {
        selection.provider_instance_key = Some(
            noema_providers::provider_account_instance_key(&selection.provider_account_id)
                .expect("hosted provider key"),
        );
    }
    let key = selection
        .provider_instance_key
        .clone()
        .expect("ready selection requires an exact key");
    registry
        .register(key, ready_test_provider())
        .expect("register test provider");
    registry
        .prove_ready_selection(selection)
        .expect("prove ready selection")
}

pub(crate) fn ready_test_provider() -> noema_providers::ProviderHandle {
    Arc::new(ReadyTestProvider)
}

pub(crate) fn ready_test_provider_registry() -> noema_providers::ProviderRegistryHandle {
    let registry = Arc::new(noema_providers::ProviderRegistry::new());
    for account_id in [
        "provider_account:codex:default",
        "provider_account:openai:default",
        "provider_account:foundation_local:default",
    ] {
        let key = noema_providers::provider_account_instance_key(account_id)
            .expect("hosted test provider key");
        registry
            .register(key, ready_test_provider())
            .expect("register hosted test provider");
    }
    registry
}

pub(crate) async fn initialize_codex_provider_selections(store: &noema_store::NoemaStore) {
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("test_configured_default".to_string()),
    );
    let ready_selection = ready_provider_selection(configured_default);
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialized provider selections");
}

pub(crate) async fn seed_task(
    store: &noema_store::NoemaStore,
    title: &str,
) -> (noema_tasks::TaskRecord, noema_tasks::AgentRunRecord) {
    initialize_codex_provider_selections(store).await;
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let provider_registry = ready_test_provider_registry();
    store
        .create_task_with_executor_with_readiness(
            noema_tasks::NewTask {
                task_id: None,
                title: title.to_string(),
                request_markdown: "Complete the task".to_string(),
                complexity: noema_tasks::TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: noema_tasks::TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![noema_tasks::NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Task is complete".to_string(),
                    expected_evidence: None,
                }],
            },
            provider_registry.as_ref(),
        )
        .await
        .expect("task")
}

pub(crate) async fn create_exa_provider_account_for_tests(
    store: &noema_store::NoemaStore,
    display_name: &str,
    status: noema_providers::ProviderAccountStatus,
    metadata: serde_json::Value,
) -> noema_providers::PersistedProviderAccountRecord {
    noema_providers::ProviderAccountPersistence::create_provider_account(
        store,
        noema_providers::NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some(display_name.to_string()),
            auth_method: noema_providers::ProviderAuthMethod::SecretInput,
            status,
            metadata,
        },
    )
    .await
    .expect("create Exa provider account")
}

pub(crate) async fn save_provider_capability_assignment_for_tests(
    store: &noema_store::NoemaStore,
    tool_name: &str,
    capability_id: &str,
    account_reference: noema_providers::ProviderCapabilityAccountReference,
) -> noema_providers::ProviderCapabilityAssignment {
    let request = noema_providers::UpsertProviderCapabilityAssignmentRequest::from_storage_values(
        tool_name,
        capability_id,
        account_reference,
    )
    .expect("valid provider capability assignment");
    noema_providers::ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
        store, request,
    )
    .await
    .expect("save provider capability assignment")
}

pub(crate) fn artifact_operations(
    store: &noema_store::NoemaStore,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let paths = test_paths();
    artifact_operations_for_paths(store, &paths)
}

pub(crate) fn artifact_operations_for_paths(
    store: &noema_store::NoemaStore,
    paths: &noema_home::NoemaPaths,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let metadata: noema_artifacts::ArtifactMetadataStoreHandle = Arc::new(store.clone());
    let service = noema_artifacts::LocalArtifactService::new(paths.root(), metadata)
        .map_err(|error| error.to_string())?;
    Ok(Arc::new(service))
}

pub(crate) fn memory_service_access(
    repository: noema_memory::MemoryRepositoryHandle,
) -> noema_memory::MemoryServiceAccessHandle {
    noema_memory::MnemosyneMemoryServiceAccess::new(repository, None).into_handle()
}

#[derive(Debug)]
struct EmptyCapabilityBindingSource;

impl noema_capabilities::CapabilityBindingSource for EmptyCapabilityBindingSource {
    fn catalog(
        &self,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<
            noema_capabilities::CapabilityCatalogResult,
            noema_capabilities::CapabilityBindingSourceError,
        >,
    > {
        Box::pin(async { Ok(noema_capabilities::CapabilityCatalogResult::default()) })
    }
}

#[derive(Debug)]
struct EmptyCapabilityInvoker;

impl noema_capabilities::CapabilityInvoker for EmptyCapabilityInvoker {
    fn invoke(
        &self,
        _invocation: noema_capabilities::CapabilityInvocation,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<noema_capabilities::CapabilityOutput, noema_capabilities::CapabilityError>,
    > {
        Box::pin(async { Err(noema_capabilities::CapabilityError::UnknownOperation) })
    }
}

#[derive(Debug)]
struct CoreTestWebBackendResolver {
    search: noema_providers::WebSearchBackendHandle,
    fetch: noema_providers::WebFetchBackendHandle,
}

impl noema_runtime::WebBackendResolver for CoreTestWebBackendResolver {
    fn resolve_search(
        &self,
        request: noema_runtime::WebBackendRequest,
    ) -> noema_runtime::WebBackendFuture<'_, noema_providers::WebSearchBackendHandle> {
        let search = self.search.clone();
        Box::pin(async move {
            if request.provider_kind == noema_providers::DUCKDUCKGO_PUBLIC_PROVIDER_ID {
                Ok(search)
            } else {
                Err(noema_runtime::WebBackendResolverError::Unavailable)
            }
        })
    }

    fn resolve_fetch(
        &self,
        request: noema_runtime::WebBackendRequest,
    ) -> noema_runtime::WebBackendFuture<'_, noema_providers::WebFetchBackendHandle> {
        let fetch = self.fetch.clone();
        Box::pin(async move {
            if request.provider_kind == noema_providers::DIRECT_HTTP_PROVIDER_ID {
                Ok(fetch)
            } else {
                Err(noema_runtime::WebBackendResolverError::Unavailable)
            }
        })
    }

    fn record_auth_failure(
        &self,
        _provider_account_id: String,
        _credential_revision: u64,
    ) -> noema_runtime::WebBackendFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn test_capability_handles() -> (
    noema_capabilities::CapabilityBindingSourceHandle,
    Arc<[noema_capabilities::CapabilityInvokerRegistration]>,
) {
    (
        Arc::new(EmptyCapabilityBindingSource),
        Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("core-test"),
            Arc::new(EmptyCapabilityInvoker),
        )]),
    )
}

fn test_web_backends() -> noema_runtime::WebBackendResolverHandle {
    Arc::new(CoreTestWebBackendResolver {
        search: noema_providers::default_web_search_backend(),
        fetch: noema_providers::default_web_fetch_backend(),
    })
}

pub(crate) async fn spawn_runtime_with_provider(
    provider: noema_providers::ProviderHandle,
    store: noema_store::NoemaStore,
) -> Result<noema_runtime::RuntimeHandle, noema_runtime::RuntimeError> {
    spawn_runtime_with_provider_map("codex", [("codex".to_string(), provider)], store).await
}

pub(crate) async fn spawn_runtime_with_provider_map<I>(
    default_provider_kind: impl Into<String>,
    providers: I,
    store: noema_store::NoemaStore,
) -> Result<noema_runtime::RuntimeHandle, noema_runtime::RuntimeError>
where
    I: IntoIterator<Item = (String, noema_providers::ProviderHandle)>,
{
    let default_provider_kind = default_provider_kind.into();
    let providers = providers.into_iter().collect::<HashMap<_, _>>();
    if !providers.contains_key(&default_provider_kind) {
        return Err(noema_runtime::RuntimeError::Protocol(format!(
            "default provider '{default_provider_kind}' is unavailable"
        )));
    }

    let default_account = match default_provider_kind.as_str() {
        "foundation_local" => {
            store
                .ensure_default_foundation_local_provider_account()
                .await?
        }
        _ => store.ensure_default_provider_account().await?,
    };
    store
        .update_provider_account_status(
            &default_account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    store.ensure_default_actors().await?;

    let default_kind = if default_provider_kind == "foundation_local" {
        "foundation_local"
    } else {
        "codex"
    };
    let default_model = if default_kind == "foundation_local" {
        noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE
    } else {
        "gpt-5.6-luna"
    };
    let registry = Arc::new(noema_providers::ProviderRegistry::new());
    for (provider_kind, provider) in providers {
        let account_id = format!("provider_account:{provider_kind}:default");
        let key = noema_providers::provider_account_instance_key(&account_id)
            .map_err(|error| noema_runtime::RuntimeError::Protocol(error.to_string()))?;
        registry
            .register(key, provider)
            .map_err(|error| noema_runtime::RuntimeError::Protocol(error.to_string()))?;
    }
    let mut configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
        default_kind,
        &default_account.provider_account_id,
        default_model,
        None,
        Some("core_test_runtime_default".to_string()),
    );
    configured_default.provider_instance_key = Some(
        noema_providers::provider_account_instance_key(&default_account.provider_account_id)
            .map_err(|error| noema_runtime::RuntimeError::Protocol(error.to_string()))?,
    );
    let ready_selection = registry
        .prove_ready_selection(configured_default.clone())
        .map_err(|error| noema_runtime::RuntimeError::Protocol(error.to_string()))?;
    store
        .initialize_missing_provider_selections(&configured_default, Some(&ready_selection))
        .await?;

    spawn_runtime_with_provider_registry_and_memory(
        registry,
        store.clone(),
        artifact_operations(&store).map_err(noema_runtime::RuntimeError::Protocol)?,
        system_error_logger(),
        None,
        noema_runtime::RuntimeEventRegistry::default(),
    )
    .await
}

pub(crate) async fn spawn_runtime_with_provider_registry_and_memory(
    provider_registry: noema_providers::ProviderRegistryHandle,
    store: noema_store::NoemaStore,
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    system_errors: noema_home::SystemErrorLogger,
    memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    runtime_events: noema_runtime::RuntimeEventRegistry,
) -> Result<noema_runtime::RuntimeHandle, noema_runtime::RuntimeError> {
    let bind = |loader| -> noema_providers::ProviderRouteResolverHandle {
        Arc::new(noema_providers::RegistryProviderRouteResolver::new(
            loader,
            Arc::clone(&provider_registry),
        ))
    };
    let (capability_bindings, capability_invokers) = test_capability_handles();
    noema_runtime::RuntimeHandle::spawn(noema_runtime::RuntimeSpawnConfig {
        primary_provider: bind(store.agent_provider_selection_loader("agent:primary")),
        default_provider: bind(store.default_provider_selection_loader()),
        progress_audit_provider: bind(
            store.auxiliary_provider_selection_loader(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID),
        ),
        web_summary_provider: bind(
            store.auxiliary_provider_selection_loader(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID),
        ),
        provider_registry,
        store,
        artifact_operations,
        system_errors,
        memory_operations,
        runtime_events,
        web_backends: test_web_backends(),
        capability_bindings,
        capability_invokers,
    })
    .await
}
