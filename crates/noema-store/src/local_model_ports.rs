//! Local-model persistence ports backed by the embedded SQLite store.

use noema_providers::{
    LocalModelActivationPersistence, LocalModelEventRecord, LocalModelInstallationPersistence,
    LocalModelInstallationRecord, LocalModelInstallationUpdate, LocalModelLifecyclePersistence,
    LocalModelReconstructionSnapshot, LocalModelRetirementClaimResult,
    LocalModelRuntimeRetirementResult, NewLocalModelInstallation, ProviderInstanceKey,
    ProviderPersistenceError, ProviderPersistenceFuture, ProviderReadySelection,
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

    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            NoemaStore::remove_terminal_local_model_installation(self, installation_id)
                .await
                .map_err(|error| {
                    local_model_error(error, "remove_terminal_local_model_installation")
                })
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
        ready_selection: &'a ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        Box::pin(async move {
            NoemaStore::activate_local_model_as_system_default(
                self,
                installation_id,
                ready_selection,
            )
            .await
            .map_err(|error| local_model_error(error, "activate_local_model"))
        })
    }
}

impl LocalModelLifecyclePersistence for NoemaStore {
    fn local_model_reconstruction_snapshot(
        &self,
    ) -> ProviderPersistenceFuture<'_, LocalModelReconstructionSnapshot> {
        Box::pin(async move {
            NoemaStore::local_model_reconstruction_snapshot(self)
                .await
                .map_err(|error| local_model_error(error, "local_model_reconstruction_snapshot"))
        })
    }

    fn retire_unreferenced_instance_runtime<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRuntimeRetirementResult> {
        Box::pin(async move {
            NoemaStore::retire_unreferenced_instance_runtime(self, provider_instance_key)
                .await
                .map_err(|error| local_model_error(error, "retire_unreferenced_instance_runtime"))
        })
    }

    fn claim_unreferenced_instance_for_retirement<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRetirementClaimResult> {
        Box::pin(async move {
            NoemaStore::claim_unreferenced_instance_for_retirement(self, provider_instance_key)
                .await
                .map_err(|error| {
                    local_model_error(error, "claim_unreferenced_instance_for_retirement")
                })
        })
    }

    fn complete_claimed_local_model_removal<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        Box::pin(async move {
            NoemaStore::complete_claimed_local_model_removal(self, provider_instance_key)
                .await
                .map_err(|error| local_model_error(error, "complete_claimed_local_model_removal"))
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
        StoreError::ProviderInstanceClaimed {
            provider_instance_key,
        } => match ProviderInstanceKey::new(provider_instance_key) {
            Ok(provider_instance_key) => ProviderPersistenceError::ProviderInstanceRetiring {
                provider_instance_key,
            },
            Err(_) => ProviderPersistenceError::Invariant { operation },
        },
        StoreError::ProviderInstanceReferenced {
            provider_instance_key,
        } => match ProviderInstanceKey::new(provider_instance_key) {
            Ok(provider_instance_key) => ProviderPersistenceError::ProviderInstanceReferenced {
                provider_instance_key,
            },
            Err(_) => ProviderPersistenceError::Invariant { operation },
        },
        StoreError::ProviderInstanceKeyMismatch { .. }
        | StoreError::ProviderInstanceKeyMissing
        | StoreError::ProviderInstanceUnavailable { .. } => {
            ProviderPersistenceError::Invariant { operation }
        }
        StoreError::LocalModelRetirementConflict { operation } => {
            ProviderPersistenceError::Conflict { operation }
        }
        StoreError::InvalidEnum { .. } | StoreError::Schema(_) => {
            ProviderPersistenceError::Invariant { operation }
        }
        StoreError::InvariantViolation { .. } => ProviderPersistenceError::Invariant { operation },
        _ => ProviderPersistenceError::Persistence { operation },
    }
}
