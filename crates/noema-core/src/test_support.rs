//! Test-only composition helpers shared across runtime and API tests.

use std::sync::Arc;

use tempfile::TempDir;

pub(crate) fn test_paths() -> noema_home::NoemaPaths {
    let home = TempDir::new().expect("temp Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("test paths");
    std::mem::forget(home);
    paths
}

pub(crate) fn system_error_logger() -> noema_home::SystemErrorLogger {
    noema_home::SystemErrorLogger::from_paths(&test_paths())
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    let paths = test_paths();
    test_store_for_paths(&paths).await
}

pub(crate) async fn test_store_for_paths(paths: &noema_home::NoemaPaths) -> crate::NoemaStore {
    crate::NoemaStore::open(&crate::StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("open store")
}

pub(crate) async fn seed_task(
    store: &crate::NoemaStore,
    title: &str,
) -> (noema_tasks::TaskRecord, noema_tasks::AgentRunRecord) {
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
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    store
        .create_task_with_executor(noema_tasks::NewTask {
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
        })
        .await
        .expect("task")
}

pub(crate) async fn create_exa_provider_account_for_tests(
    store: &crate::NoemaStore,
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
    store: &crate::NoemaStore,
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
    store: &crate::NoemaStore,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let paths = test_paths();
    artifact_operations_for_paths(store, &paths)
}

pub(crate) fn artifact_operations_for_paths(
    store: &crate::NoemaStore,
    paths: &noema_home::NoemaPaths,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let metadata: noema_artifacts::ArtifactMetadataStoreHandle = Arc::new(store.clone());
    let service = noema_artifacts::LocalArtifactService::new(paths.root(), metadata)
        .map_err(|error| error.to_string())?;
    Ok(Arc::new(service))
}

pub(crate) fn provider_route(
    selection: noema_providers::ProviderSelectionSnapshot,
    provider: noema_providers::ProviderHandle,
) -> Arc<noema_providers::ProviderRouteLease> {
    let provider_kind = selection.provider_kind.clone();
    let routes = crate::daemon::LegacyProviderRoutes::new([(provider_kind.as_str(), provider)])
        .expect("test provider routes");
    Arc::new(
        routes
            .resolve_snapshot(selection)
            .expect("test provider route"),
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
