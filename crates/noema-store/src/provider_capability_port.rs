//! Provider capability-assignment persistence port backed by SQLite.

use noema_providers::{
    ProviderCapabilityAssignment, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderPersistenceError, ProviderPersistenceFuture,
    ReplaceProviderCapabilityRouteRequest, UpsertProviderCapabilityAssignmentRequest,
};

use super::{
    NoemaStore, StoreError,
    provider_account_port::{provider_error, provider_future},
};

impl ProviderCapabilityAssignmentPersistence for NoemaStore {
    fn provider_capability_assignment<'a>(
        &'a self,
        key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderCapabilityAssignment>> {
        provider_future(
            NoemaStore::provider_capability_binding(
                self,
                key.tool_name_str(),
                key.capability_id_str(),
            ),
            "provider_capability_assignment",
        )
    }

    fn provider_capability_route<'a>(
        &'a self,
        key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Vec<ProviderCapabilityAssignment>> {
        provider_future(
            NoemaStore::provider_capability_route(
                self,
                key.tool_name_str(),
                key.capability_id_str(),
            ),
            "provider_capability_route",
        )
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

    fn replace_provider_capability_route(
        &self,
        request: ReplaceProviderCapabilityRouteRequest,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderCapabilityAssignment>> {
        Box::pin(async move {
            NoemaStore::replace_provider_capability_route(self, &request)
                .await
                .map_err(capability_write_error)
        })
    }

    fn clear_provider_capability_assignments<'a>(
        &'a self,
        keys: &'a [ProviderCapabilityAssignmentKey],
    ) -> ProviderPersistenceFuture<'a, ()> {
        Box::pin(async move {
            NoemaStore::clear_provider_capability_bindings(self, keys)
                .await
                .map_err(|error| provider_error(error, "clear_provider_capability_assignments"))
        })
    }
}

fn capability_write_error(error: StoreError) -> ProviderPersistenceError {
    match error {
        StoreError::ProviderAccountNotFound {
            provider_account_id,
        } => ProviderPersistenceError::AccountNotFound {
            provider_account_id,
        },
        error => provider_error(error, "upsert_provider_capability_assignment"),
    }
}
