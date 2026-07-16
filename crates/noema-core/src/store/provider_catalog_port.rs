//! Provider model-catalog persistence port backed by SQLite.

use noema_providers::{
    PersistProviderModelCatalogRequest, ProviderAccountStatus, ProviderModelCatalogPersistence,
    ProviderModelProfile, ProviderPersistenceError, ProviderPersistenceFuture,
};
use rusqlite::{TransactionBehavior, params};
use serde_json::Value;

use super::{
    NoemaStore, StoreError,
    ids::now_string,
    provider_accounts::{
        PROVIDER_ACCOUNT_SELECT, provider_account_from_row, provider_account_row,
        provider_status_str,
    },
    sqlite::json_to_string,
};

impl ProviderModelCatalogPersistence for NoemaStore {
    fn persist_provider_model_catalog(
        &self,
        request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, noema_providers::ProviderAccountRecord> {
        Box::pin(async move { persist_model_catalog(self, request).await })
    }
}

async fn persist_model_catalog(
    store: &NoemaStore,
    request: PersistProviderModelCatalogRequest,
) -> Result<noema_providers::ProviderAccountRecord, ProviderPersistenceError> {
    store
        .with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = transaction
                .query_row(
                    format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                        .as_str(),
                    [&request.provider_account_id],
                    provider_account_row,
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => StoreError::ProviderAccountNotFound {
                        provider_account_id: request.provider_account_id.clone(),
                    },
                    other => StoreError::Sqlite(other),
                })?;
            let current = provider_account_from_row(current)?;
            let mut metadata = current.metadata.clone();
            ProviderModelProfile::write_account_metadata(&mut metadata, &request.profiles)
                .map_err(|_| StoreError::InvariantViolation {
                    message: "typed provider model profiles failed to serialize".to_string(),
                })?;
            let metadata_object =
                metadata
                    .as_object_mut()
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: "provider model profile metadata was not an object".to_string(),
                    })?;
            metadata_object.insert(
                "models_refreshed_at".to_string(),
                Value::String(request.refreshed_at_unix.to_string()),
            );
            metadata_object.insert("models_source".to_string(), Value::String(request.source));
            metadata_object.insert(
                "models_metadata_version".to_string(),
                Value::from(request.metadata_version),
            );
            metadata_object.insert(
                "models_client_version".to_string(),
                Value::String(request.client_version),
            );
            if let Some(refreshed_at) = request.client_version_refreshed_at_unix {
                metadata_object.insert(
                    "models_client_version_refreshed_at".to_string(),
                    Value::String(refreshed_at.to_string()),
                );
            }
            let metadata_json = json_to_string(&metadata)?;
            let status_changed = current.status != request.resulting_status;
            let checked_at = status_changed.then(now_string);
            let last_authenticated_at = if status_changed
                && request.resulting_status == ProviderAccountStatus::Authenticated
            {
                checked_at.clone()
            } else {
                current.last_authenticated_at.clone()
            };
            let last_error_code = if status_changed {
                None
            } else {
                current.last_error_code.as_deref()
            };
            let last_error_message = if status_changed {
                None
            } else {
                current.last_error_message.as_deref()
            };
            let changed = transaction.execute(
                r#"
                UPDATE provider_accounts
                SET metadata_json = ?2,
                    status = ?3,
                    last_checked_at = COALESCE(?4, last_checked_at),
                    last_authenticated_at = ?5,
                    last_error_code = ?6,
                    last_error_message = ?7,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE provider_account_id = ?1
                "#,
                params![
                    request.provider_account_id,
                    metadata_json,
                    provider_status_str(request.resulting_status),
                    checked_at,
                    last_authenticated_at,
                    last_error_code,
                    last_error_message,
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "provider model catalog update changed an unexpected row count"
                        .to_string(),
                });
            }
            let updated = transaction.query_row(
                format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                    .as_str(),
                [&request.provider_account_id],
                provider_account_row,
            )?;
            let updated = provider_account_from_row(updated)?;
            transaction.commit()?;
            Ok(updated)
        })
        .await
        .map_err(|error| match error {
            StoreError::ProviderAccountNotFound {
                provider_account_id,
            } => ProviderPersistenceError::AccountNotFound {
                provider_account_id,
            },
            StoreError::Json(_)
            | StoreError::InvalidEnum { .. }
            | StoreError::InvariantViolation { .. }
            | StoreError::Schema(_) => ProviderPersistenceError::Invariant {
                operation: "persist_provider_model_catalog",
            },
            _ => ProviderPersistenceError::Persistence {
                operation: "persist_provider_model_catalog",
            },
        })
}
