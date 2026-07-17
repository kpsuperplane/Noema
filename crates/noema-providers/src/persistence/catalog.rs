//! Provider model-catalog persistence boundary.

use std::sync::Arc;

use crate::{
    PersistedProviderAccountRecord, ProviderAccountStatus, ProviderModelProfile,
    ProviderPersistenceFuture,
};

/// Typed provider-owned fields committed by one model-catalog refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistProviderModelCatalogRequest {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Refreshed typed model profiles.
    pub profiles: Vec<ProviderModelProfile>,
    /// Unix-seconds refresh timestamp.
    pub refreshed_at_unix: u64,
    /// Stable provider-specific source label.
    pub source: String,
    /// Catalog metadata schema version.
    pub metadata_version: u64,
    /// Provider client version used for the catalog request.
    pub client_version: String,
    /// Optional Unix-seconds client-version refresh timestamp.
    pub client_version_refreshed_at_unix: Option<u64>,
    /// Resulting provider account status.
    pub resulting_status: ProviderAccountStatus,
}

/// Atomic provider model-catalog persistence operation.
pub trait ProviderModelCatalogPersistence: Send + Sync {
    /// Merge catalog-owned fields into the latest account metadata and commit
    /// them with the resulting account status.
    fn persist_provider_model_catalog(
        &self,
        request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord>;
}

/// Clonable provider model-catalog persistence handle.
pub type ProviderModelCatalogPersistenceHandle = Arc<dyn ProviderModelCatalogPersistence>;
