//! Test-only contract fakes and runtime composition helpers.

use std::sync::Arc;

use tempfile::TempDir;

pub(crate) use crate::contract_test_support::{
    create_exa_provider_account_for_tests, initialize_codex_provider_selections,
    ready_provider_selection, ready_test_provider_registry,
    save_provider_capability_assignment_for_tests, seed_task,
};

pub(crate) fn test_paths() -> noema_home::NoemaPaths {
    let home = TempDir::new().expect("temp Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("test paths");
    std::mem::forget(home);
    paths
}

pub(crate) fn system_error_logger() -> noema_home::SystemErrorLogger {
    noema_home::SystemErrorLogger::from_paths(&test_paths())
}

pub(crate) async fn test_store() -> noema_store::NoemaStore {
    noema_store::test_support::open_ephemeral_store()
        .await
        .expect("open ephemeral store")
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

pub(crate) fn artifact_operations(
    store: &noema_store::NoemaStore,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let metadata: noema_artifacts::ArtifactMetadataStoreHandle = Arc::new(store.clone());
    noema_artifacts::LocalArtifactService::new(test_paths().root(), metadata)
        .map(|service| Arc::new(service) as noema_artifacts::ArtifactOperationsHandle)
        .map_err(|error| error.to_string())
}

pub(crate) fn web_backends() -> crate::WebBackendResolverHandle {
    crate::contract_test_support::test_web_backends(None)
}

pub(crate) fn web_backends_for_store(
    store: &noema_store::NoemaStore,
) -> crate::WebBackendResolverHandle {
    crate::contract_test_support::test_web_backends(Some(store))
}
