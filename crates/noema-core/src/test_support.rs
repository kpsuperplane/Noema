//! Test-only composition helpers shared across runtime and API tests.

use std::sync::Arc;

pub(crate) fn artifact_operations(
    store: &crate::NoemaStore,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let paths = store.noema_paths().map_err(|error| error.to_string())?;
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
