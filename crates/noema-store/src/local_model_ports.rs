//! Local-model persistence ports backed by the embedded SQLite store.

use noema_providers::{
    DefaultModelPreferenceRecord, LocalModelActivationPersistence, LocalModelEventRecord,
    LocalModelInstallationPersistence, LocalModelInstallationRecord, LocalModelInstallationUpdate,
    NewLocalModelInstallation, ProviderPersistenceError, ProviderPersistenceFuture,
    RemovedLocalModelInstallation,
};

use super::{NoemaStore, StoreError};

impl LocalModelInstallationPersistence for NoemaStore {
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord> {
        Box::pin(async move {
            NoemaStore::upsert_local_model_installation(self, input)
                .await
                .map_err(|error| local_model_error(error, "upsert_local_model_installation"))
        })
    }

    fn local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        Box::pin(async move {
            NoemaStore::get_local_model_installation(self, installation_id)
                .await
                .map_err(|error| local_model_error(error, "local_model_installation"))
        })
    }

    fn installed_local_model<'a>(
        &'a self,
        model_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        Box::pin(async move {
            NoemaStore::get_installed_local_model(self, model_id)
                .await
                .map_err(|error| local_model_error(error, "installed_local_model"))
        })
    }

    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>> {
        Box::pin(async move {
            NoemaStore::list_local_model_installations(self)
                .await
                .map_err(|error| local_model_error(error, "local_model_installations"))
        })
    }

    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            NoemaStore::update_local_model_installation(self, installation_id, update)
                .await
                .map_err(|error| local_model_error(error, "update_local_model_installation"))
        })
    }

    fn cancel_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            NoemaStore::cancel_local_model_installation(self, installation_id)
                .await
                .map_err(|error| local_model_error(error, "cancel_local_model_installation"))
        })
    }

    fn remove_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            NoemaStore::remove_local_model_installation(self, installation_id)
                .await
                .map_err(|error| local_model_error(error, "remove_local_model_installation"))
        })
    }

    fn local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>> {
        Box::pin(async move {
            NoemaStore::list_local_model_events(self, after_cursor, limit)
                .await
                .map_err(|error| local_model_error(error, "local_model_events"))
        })
    }
}

impl LocalModelActivationPersistence for NoemaStore {
    fn activate_local_model_as_system_default<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, DefaultModelPreferenceRecord> {
        Box::pin(async move {
            NoemaStore::activate_local_model_as_system_default(self, installation_id)
                .await
                .map_err(|error| local_model_error(error, "activate_local_model"))
        })
    }
}

fn local_model_error(error: StoreError, operation: &'static str) -> ProviderPersistenceError {
    match error {
        StoreError::LocalModelInstallationNotFound { installation_id } => {
            ProviderPersistenceError::InstallationNotFound { installation_id }
        }
        StoreError::InvalidLocalModelTransition { from, to } => {
            ProviderPersistenceError::InvalidInstallationTransition { from, to }
        }
        StoreError::ActiveLocalModelInstallation { installation_id } => {
            ProviderPersistenceError::ActiveInstallationConflict { installation_id }
        }
        StoreError::InvalidLocalModelRequest { kind } => {
            ProviderPersistenceError::InvalidRequest { kind }
        }
        StoreError::LocalModelActivationNotReady { .. } => {
            ProviderPersistenceError::InvalidRequest {
                kind: "local_model_activation_state",
            }
        }
        StoreError::InvalidEnum { .. } | StoreError::Schema(_) => {
            ProviderPersistenceError::Invariant { operation }
        }
        StoreError::InvariantViolation { .. } => ProviderPersistenceError::Invariant { operation },
        _ => ProviderPersistenceError::Persistence { operation },
    }
}
