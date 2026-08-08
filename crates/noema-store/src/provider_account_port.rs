//! Provider account persistence port backed by the embedded SQLite store.

use noema_providers::{
    NewProviderAccount, ProviderAccountPersistence, ProviderAccountRecord,
    ProviderPersistenceError, ProviderPersistenceFuture, ProviderReadySelection,
    ProviderSelectionSnapshot, UpdateProviderAccountRequest,
};

use super::{NoemaStore, StoreError};

impl ProviderAccountPersistence for NoemaStore {
    fn provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
        provider_future(
            NoemaStore::get_provider_account(self, provider_account_id),
            "provider_account",
        )
    }

    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
        provider_future(
            NoemaStore::active_provider_accounts(self),
            "active_provider_accounts",
        )
    }

    fn create_provider_account(
        &self,
        request: NewProviderAccount,
    ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
        Box::pin(async move {
            NoemaStore::create_provider_account(self, request)
                .await
                .map_err(create_account_error)
        })
    }

    fn update_provider_account(
        &self,
        request: UpdateProviderAccountRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
        Box::pin(async move { update_provider_account(self, request).await })
    }

    fn delete_provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, bool> {
        Box::pin(async move { delete_provider_account(self, provider_account_id).await })
    }

    fn initialize_missing_provider_selections<'a>(
        &'a self,
        selection: &'a ProviderSelectionSnapshot,
        ready: &'a ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, ()> {
        provider_future(
            NoemaStore::initialize_missing_provider_selections(self, selection, Some(ready)),
            "initialize_missing_provider_selections",
        )
    }
}

async fn update_provider_account(
    store: &NoemaStore,
    request: UpdateProviderAccountRequest,
) -> Result<ProviderAccountRecord, ProviderPersistenceError> {
    if request.auth_method.is_none() && request.status.is_none() && request.metadata.is_none() {
        return Err(ProviderPersistenceError::InvalidRequest {
            kind: "empty_provider_account_update",
        });
    }
    store
        .update_provider_account_fields(
            &request.provider_account_id,
            request.auth_method,
            request.status.as_ref(),
            request.metadata.as_ref(),
        )
        .await
        .map_err(update_account_error)
}

async fn delete_provider_account(
    store: &NoemaStore,
    provider_account_id: &str,
) -> Result<bool, ProviderPersistenceError> {
    NoemaStore::delete_provider_account(store, provider_account_id)
        .await
        .map_err(|error| match error {
            StoreError::ProtectedProviderAccount {
                provider_account_id,
            } => ProviderPersistenceError::ProtectedAccount {
                provider_account_id,
            },
            StoreError::ProviderAccountInUse {
                provider_account_id,
            } => ProviderPersistenceError::AccountInUse {
                provider_account_id,
            },
            StoreError::InvariantViolation { .. } | StoreError::Schema(_) => {
                ProviderPersistenceError::Invariant {
                    operation: "delete_provider_account",
                }
            }
            _ => ProviderPersistenceError::Persistence {
                operation: "delete_provider_account",
            },
        })
}

pub(super) fn provider_error(
    error: StoreError,
    operation: &'static str,
) -> ProviderPersistenceError {
    match error {
        StoreError::Json(_)
        | StoreError::InvalidEnum { .. }
        | StoreError::InvariantViolation { .. }
        | StoreError::Schema(_) => ProviderPersistenceError::Invariant { operation },
        _ => ProviderPersistenceError::Persistence { operation },
    }
}

pub(super) fn provider_future<'a, T: 'a>(
    future: impl Future<Output = Result<T, StoreError>> + Send + 'a,
    operation: &'static str,
) -> ProviderPersistenceFuture<'a, T> {
    Box::pin(async move {
        future
            .await
            .map_err(|error| provider_error(error, operation))
    })
}

fn create_account_error(error: StoreError) -> ProviderPersistenceError {
    match error {
        StoreError::InvalidEnum { .. } => ProviderPersistenceError::InvalidRequest {
            kind: "provider_account",
        },
        StoreError::ProviderAccountNotFound {
            provider_account_id,
        } => ProviderPersistenceError::AccountNotFound {
            provider_account_id,
        },
        StoreError::InvariantViolation { .. } | StoreError::Schema(_) => {
            ProviderPersistenceError::Invariant {
                operation: "create_provider_account",
            }
        }
        _ => ProviderPersistenceError::Persistence {
            operation: "create_provider_account",
        },
    }
}

fn update_account_error(error: StoreError) -> ProviderPersistenceError {
    match error {
        StoreError::ProviderAccountNotFound {
            provider_account_id,
        } => ProviderPersistenceError::AccountNotFound {
            provider_account_id,
        },
        error => provider_error(error, "update_provider_account"),
    }
}
use std::future::Future;
