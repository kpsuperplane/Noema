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

pub(crate) fn provider_route_resolver(
    selection: noema_providers::ProviderSelectionSnapshot,
    provider: noema_providers::ProviderHandle,
) -> noema_providers::ProviderRouteResolverHandle {
    let provider_kind = selection.provider_kind.clone();
    let routes = crate::daemon::LegacyProviderRoutes::new([(provider_kind.as_str(), provider)])
        .expect("test provider routes");
    routes.bind(noema_providers::provider_selection_loader(move || {
        let selection = selection.clone();
        Box::pin(async move { Ok(selection) })
    }))
}
