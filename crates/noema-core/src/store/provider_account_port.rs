//! Provider account persistence port backed by the embedded SQLite store.

use noema_providers::{
    NewProviderAccount, ProviderAccountPersistence, ProviderAccountRecord,
    ProviderAccountStatusUpdate, ProviderPersistenceError, ProviderPersistenceFuture,
    UpdateProviderAccountRequest,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    NoemaStore, StoreError,
    ids::now_string,
    provider_accounts::{
        PROVIDER_ACCOUNT_SELECT, provider_account_from_row, provider_account_row,
        provider_status_str,
    },
    sqlite::json_to_string,
};

impl ProviderAccountPersistence for NoemaStore {
    fn provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
        Box::pin(async move {
            NoemaStore::get_provider_account(self, provider_account_id)
                .await
                .map_err(|error| account_read_error(error, "provider_account"))
        })
    }

    fn active_provider_account<'a>(
        &'a self,
        provider_kind: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
        Box::pin(async move {
            NoemaStore::active_provider_account(self, provider_kind)
                .await
                .map_err(|error| account_read_error(error, "active_provider_account"))
        })
    }

    fn active_default_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
        Box::pin(async move {
            NoemaStore::active_default_provider_accounts(self)
                .await
                .map_err(|error| account_read_error(error, "active_default_provider_accounts"))
        })
    }

    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
        Box::pin(async move {
            NoemaStore::active_provider_accounts(self)
                .await
                .map_err(|error| account_read_error(error, "active_provider_accounts"))
        })
    }

    fn provider_accounts(&self) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
        Box::pin(async move {
            NoemaStore::list_provider_accounts(self)
                .await
                .map_err(|error| account_read_error(error, "provider_accounts"))
        })
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
}

async fn update_provider_account(
    store: &NoemaStore,
    request: UpdateProviderAccountRequest,
) -> Result<ProviderAccountRecord, ProviderPersistenceError> {
    if request.status.is_none() && request.metadata.is_none() {
        return Err(ProviderPersistenceError::InvalidRequest {
            kind: "empty_provider_account_update",
        });
    }
    let metadata_json = request
        .metadata
        .as_ref()
        .map(json_to_string)
        .transpose()
        .map_err(|_| ProviderPersistenceError::InvalidRequest {
            kind: "provider_account_metadata",
        })?;
    let provider_account_id = request.provider_account_id;
    store
        .with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = transaction
                .query_row(
                    format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                        .as_str(),
                    [&provider_account_id],
                    provider_account_row,
                )
                .optional()?
                .ok_or_else(|| StoreError::ProviderAccountNotFound {
                    provider_account_id: provider_account_id.clone(),
                })?;
            let current = provider_account_from_row(current)?;

            match (request.status, metadata_json) {
                (Some(status), Some(metadata_json)) => update_status_and_metadata(
                    &transaction,
                    &provider_account_id,
                    &current,
                    status,
                    &metadata_json,
                )?,
                (Some(status), None) => {
                    update_status(&transaction, &provider_account_id, &current, status)?;
                }
                (None, Some(metadata_json)) => {
                    let changed = transaction.execute(
                        r#"
                        UPDATE provider_accounts
                        SET metadata_json = ?2,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                        WHERE provider_account_id = ?1
                        "#,
                        params![provider_account_id, metadata_json],
                    )?;
                    require_single_account_change(changed)?;
                }
                (None, None) => unreachable!("empty account updates are rejected before locking"),
            }

            let updated = transaction.query_row(
                format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                    .as_str(),
                [&provider_account_id],
                provider_account_row,
            )?;
            let updated = provider_account_from_row(updated)?;
            transaction.commit()?;
            Ok(updated)
        })
        .await
        .map_err(update_account_error)
}

fn update_status(
    transaction: &rusqlite::Transaction<'_>,
    provider_account_id: &str,
    current: &ProviderAccountRecord,
    status: ProviderAccountStatusUpdate,
) -> Result<(), StoreError> {
    let checked_at = now_string();
    let last_authenticated_at =
        if status.status == noema_providers::ProviderAccountStatus::Authenticated {
            Some(checked_at.clone())
        } else {
            current.last_authenticated_at.clone()
        };
    let changed = transaction.execute(
        r#"
        UPDATE provider_accounts
        SET status = ?2,
            last_checked_at = ?3,
            last_authenticated_at = ?4,
            last_error_code = ?5,
            last_error_message = ?6,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE provider_account_id = ?1
        "#,
        params![
            provider_account_id,
            provider_status_str(status.status),
            checked_at,
            last_authenticated_at,
            status.error_code,
            status.error_message,
        ],
    )?;
    require_single_account_change(changed)
}

fn update_status_and_metadata(
    transaction: &rusqlite::Transaction<'_>,
    provider_account_id: &str,
    current: &ProviderAccountRecord,
    status: ProviderAccountStatusUpdate,
    metadata_json: &str,
) -> Result<(), StoreError> {
    let checked_at = now_string();
    let last_authenticated_at =
        if status.status == noema_providers::ProviderAccountStatus::Authenticated {
            Some(checked_at.clone())
        } else {
            current.last_authenticated_at.clone()
        };
    let changed = transaction.execute(
        r#"
        UPDATE provider_accounts
        SET status = ?2,
            last_checked_at = ?3,
            last_authenticated_at = ?4,
            last_error_code = ?5,
            last_error_message = ?6,
            metadata_json = ?7,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE provider_account_id = ?1
        "#,
        params![
            provider_account_id,
            provider_status_str(status.status),
            checked_at,
            last_authenticated_at,
            status.error_code,
            status.error_message,
            metadata_json,
        ],
    )?;
    require_single_account_change(changed)
}

fn require_single_account_change(changed: usize) -> Result<(), StoreError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: "guarded provider account update changed an unexpected row count".to_string(),
        })
    }
}

async fn delete_provider_account(
    store: &NoemaStore,
    provider_account_id: &str,
) -> Result<bool, ProviderPersistenceError> {
    store
        .with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let is_default = transaction
                .query_row(
                    "SELECT is_default FROM provider_accounts WHERE provider_account_id = ?1",
                    [provider_account_id],
                    |row| row.get::<_, bool>(0),
                )
                .optional()?;
            let Some(is_default) = is_default else {
                transaction.commit()?;
                return Ok(false);
            };
            if is_default {
                return Err(StoreError::ProtectedProviderAccount {
                    provider_account_id: provider_account_id.to_string(),
                });
            }
            transaction.execute(
                "DELETE FROM provider_capability_bindings WHERE provider_account_id = ?1",
                [provider_account_id],
            )?;
            let changed = transaction.execute(
                "DELETE FROM provider_accounts WHERE provider_account_id = ?1 AND is_default = 0",
                [provider_account_id],
            )?;
            require_single_account_change(changed)?;
            transaction.commit()?;
            Ok(true)
        })
        .await
        .map_err(|error| match error {
            StoreError::ProtectedProviderAccount {
                provider_account_id,
            } => ProviderPersistenceError::ProtectedAccount {
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

fn account_read_error(error: StoreError, operation: &'static str) -> ProviderPersistenceError {
    match error {
        StoreError::Json(_)
        | StoreError::InvalidEnum { .. }
        | StoreError::InvariantViolation { .. }
        | StoreError::Schema(_) => ProviderPersistenceError::Invariant { operation },
        _ => ProviderPersistenceError::Persistence { operation },
    }
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
        StoreError::Json(_)
        | StoreError::InvalidEnum { .. }
        | StoreError::InvariantViolation { .. }
        | StoreError::Schema(_) => ProviderPersistenceError::Invariant {
            operation: "update_provider_account",
        },
        _ => ProviderPersistenceError::Persistence {
            operation: "update_provider_account",
        },
    }
}
