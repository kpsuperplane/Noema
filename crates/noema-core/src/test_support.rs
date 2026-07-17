//! Test-only composition helpers shared across runtime and API tests.

use std::sync::Arc;

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

pub(crate) fn provider_route(
    mut selection: noema_providers::ProviderSelectionSnapshot,
    provider: noema_providers::ProviderHandle,
) -> Arc<noema_providers::ProviderRouteLease> {
    let key = noema_providers::provider_account_instance_key(&selection.provider_account_id)
        .expect("hosted test provider key");
    selection.provider_instance_key = Some(key.clone());
    let registry = noema_providers::ProviderRegistry::new();
    registry
        .register(key.clone(), provider)
        .expect("register test provider");
    let lease = registry.lease(&key).expect("lease test provider");
    Arc::new(
        noema_providers::ProviderRouteLease::try_new(selection, lease)
            .expect("exact test provider route"),
    )
}

pub(crate) fn mnemosyne_operations_for_base_url(
    base_url: String,
) -> noema_memory::MemoryOperationsHandle {
    let connection = noema_memory::MnemosyneConnection::new(base_url, None);
    noema_memory::MnemosyneMemoryService::from_connection(Some(connection)).into_handle()
}

pub(crate) fn memory_service_access(
    repository: noema_memory::MemoryRepositoryHandle,
) -> noema_memory::MemoryServiceAccessHandle {
    noema_memory::MnemosyneMemoryServiceAccess::new(repository, None).into_handle()
}
