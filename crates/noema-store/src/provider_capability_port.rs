//! Provider capability-assignment persistence port backed by SQLite.

use noema_providers::{
    ProviderCapabilityAssignment, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderPersistenceError, ProviderPersistenceFuture,
    UpsertProviderCapabilityAssignmentRequest,
};

use super::{NoemaStore, StoreError};

impl ProviderCapabilityAssignmentPersistence for NoemaStore {
    fn provider_capability_assignment<'a>(
        &'a self,
        key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderCapabilityAssignment>> {
        Box::pin(async move {
            NoemaStore::provider_capability_binding(
                self,
                key.tool_name_str(),
                key.capability_id_str(),
            )
            .await
            .map_err(capability_read_error)
        })
    }

    fn upsert_provider_capability_assignment(
        &self,
        request: UpsertProviderCapabilityAssignmentRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderCapabilityAssignment> {
        Box::pin(async move {
            NoemaStore::upsert_provider_capability_binding(
                self,
                request.tool_name_str(),
                request.capability_id_str(),
                request.account_reference(),
            )
            .await
            .map_err(capability_write_error)
        })
    }
}

fn capability_read_error(error: StoreError) -> ProviderPersistenceError {
    match error {
        StoreError::Json(_)
        | StoreError::InvalidEnum { .. }
        | StoreError::InvariantViolation { .. }
        | StoreError::Schema(_) => ProviderPersistenceError::Invariant {
            operation: "provider_capability_assignment",
        },
        _ => ProviderPersistenceError::Persistence {
            operation: "provider_capability_assignment",
        },
    }
}

fn capability_write_error(error: StoreError) -> ProviderPersistenceError {
    match error {
        StoreError::ProviderAccountNotFound {
            provider_account_id,
        } => ProviderPersistenceError::AccountNotFound {
            provider_account_id,
        },
        StoreError::Json(_) | StoreError::InvalidEnum { .. } => {
            ProviderPersistenceError::Invariant {
                operation: "upsert_provider_capability_assignment",
            }
        }
        StoreError::InvariantViolation { .. } | StoreError::Schema(_) => {
            ProviderPersistenceError::Invariant {
                operation: "upsert_provider_capability_assignment",
            }
        }
        _ => ProviderPersistenceError::Persistence {
            operation: "upsert_provider_capability_assignment",
        },
    }
}
