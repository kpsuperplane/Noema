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
