//! Local-model persistence boundaries.

use std::sync::Arc;

use crate::{
    DefaultModelPreferenceRecord, LocalModelEventRecord, LocalModelInstallationRecord,
    LocalModelInstallationUpdate, NewLocalModelInstallation, ProviderPersistenceFuture,
    RemovedLocalModelInstallation,
};

/// Local-model installation lifecycle persistence.
pub trait LocalModelInstallationPersistence: Send + Sync {
    /// Create or refresh queued installation provenance.
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord>;

    /// Return one installation by id.
    fn local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>>;

    /// Return the legacy active-first/newest installed artifact for one model id.
    fn installed_local_model<'a>(
        &'a self,
        model_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>>;

    /// Return every installation in stable presentation order.
    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>>;

    /// Persist one valid lifecycle transition and its event atomically.
    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord>;

    /// Cancel one unfinished installation.
    fn cancel_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord>;

    /// Remove one inactive installation and report an unreferenced blob.
    fn remove_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation>;

    /// Return lifecycle events strictly after an optional cursor.
    fn local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>>;
}

/// Clonable local-model installation persistence handle.
pub type LocalModelInstallationPersistenceHandle = Arc<dyn LocalModelInstallationPersistence>;

/// Coarse system-wide local-model activation persistence.
pub trait LocalModelActivationPersistence: Send + Sync {
    /// Atomically activate an installed model across every current workload.
    fn activate_local_model_as_system_default<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, DefaultModelPreferenceRecord>;
}

/// Clonable local-model activation persistence handle.
pub type LocalModelActivationPersistenceHandle = Arc<dyn LocalModelActivationPersistence>;
