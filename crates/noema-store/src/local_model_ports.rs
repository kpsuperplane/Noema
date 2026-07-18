//! Local-model persistence ports backed by the embedded SQLite store.

use noema_providers::{
    LocalModelActivationPersistence, LocalModelEventRecord, LocalModelInstallationPersistence,
    LocalModelInstallationRecord, LocalModelInstallationUpdate, LocalModelLifecyclePersistence,
    LocalModelReconstructionSnapshot, LocalModelRetirementClaimResult,
    LocalModelRuntimeRetirementResult, NewLocalModelInstallation, ProviderInstanceKey,
    ProviderPersistenceError, ProviderPersistenceFuture, ProviderReadySelection,
    RemovedLocalModelInstallation,
};

use super::{NoemaStore, StoreError, provider_account_port::provider_error};

impl LocalModelInstallationPersistence for NoemaStore {
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord> {
        local_model_future(
            NoemaStore::upsert_local_model_installation(self, input),
            "upsert_local_model_installation",
        )
    }

    fn local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        local_model_future(
            NoemaStore::get_local_model_installation(self, installation_id),
            "local_model_installation",
        )
    }

    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>> {
        local_model_future(
            NoemaStore::list_local_model_installations(self),
            "local_model_installations",
        )
    }

    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        local_model_future(
            NoemaStore::update_local_model_installation(self, installation_id, update),
            "update_local_model_installation",
        )
    }

    fn cancel_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        local_model_future(
            NoemaStore::cancel_local_model_installation(self, installation_id),
            "cancel_local_model_installation",
        )
    }

    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        local_model_future(
            NoemaStore::remove_terminal_local_model_installation(self, installation_id),
            "remove_terminal_local_model_installation",
        )
    }

    fn local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>> {
        local_model_future(
            NoemaStore::list_local_model_events(self, after_cursor, limit),
            "local_model_events",
        )
    }
}

impl LocalModelActivationPersistence for NoemaStore {
    fn activate_local_model_as_system_default<'a>(
        &'a self,
        installation_id: &'a str,
        ready_selection: &'a ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        local_model_future(
            NoemaStore::activate_local_model_as_system_default(
                self,
                installation_id,
                ready_selection,
            ),
            "activate_local_model",
        )
    }
}

impl LocalModelLifecyclePersistence for NoemaStore {
    fn local_model_reconstruction_snapshot(
        &self,
    ) -> ProviderPersistenceFuture<'_, LocalModelReconstructionSnapshot> {
        local_model_future(
            NoemaStore::local_model_reconstruction_snapshot(self),
            "local_model_reconstruction_snapshot",
        )
    }

    fn retire_unreferenced_instance_runtime<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRuntimeRetirementResult> {
        local_model_future(
            NoemaStore::retire_unreferenced_instance_runtime(self, provider_instance_key),
            "retire_unreferenced_instance_runtime",
        )
    }

    fn claim_unreferenced_instance_for_retirement<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRetirementClaimResult> {
        local_model_future(
            NoemaStore::claim_unreferenced_instance_for_retirement(self, provider_instance_key),
            "claim_unreferenced_instance_for_retirement",
        )
    }

    fn complete_claimed_local_model_removal<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        local_model_future(
            NoemaStore::complete_claimed_local_model_removal(self, provider_instance_key),
            "complete_claimed_local_model_removal",
        )
    }
}

fn local_model_future<'a, T: 'a>(
    future: impl std::future::Future<Output = Result<T, StoreError>> + Send + 'a,
    operation: &'static str,
) -> ProviderPersistenceFuture<'a, T> {
    Box::pin(async move {
        future
            .await
            .map_err(|error| local_model_error(error, operation))
    })
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
        error => provider_error(error, operation),
    }
}
