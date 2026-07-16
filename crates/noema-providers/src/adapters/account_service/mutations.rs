use std::{fs, future::Future};

use serde_json::json;

use super::{
    ProviderAccountService,
    helpers::{
        authenticated_status, map_persistence_error, metadata_with_credential_state,
        unauthenticated_status,
    },
};
use crate::adapters::{SecretInputStore, account_service::filesystem::FileSnapshot};
use crate::{
    CreateSecretProviderAccountRequest, NewProviderAccount, ProviderAccountOperationError,
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod,
    SaveProviderAccountSecretRequest, UpdateProviderAccountRequest, provider_account_catalog,
};

async fn compensate_failed_create_transaction<
    Rollback,
    Delete,
    DeleteFuture,
    RollbackError,
    DeleteError,
>(
    rollback_credentials: Rollback,
    delete_durable_account: Delete,
) -> bool
where
    Rollback: FnOnce() -> Result<(), RollbackError>,
    Delete: FnOnce() -> DeleteFuture,
    DeleteFuture: Future<Output = Result<bool, DeleteError>>,
{
    if rollback_credentials().is_err() {
        return false;
    }

    matches!(delete_durable_account().await, Ok(true))
}

impl ProviderAccountService {
    pub(super) async fn create_secret_account_impl(
        &self,
        request: CreateSecretProviderAccountRequest,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let catalog_entry = provider_account_catalog()
            .into_iter()
            .find(|entry| entry.provider_kind == request.provider_kind)
            .ok_or(ProviderAccountOperationError::UnsupportedProvider)?;
        if catalog_entry.auth_method != ProviderAuthMethod::SecretInput {
            return Err(ProviderAccountOperationError::AuthMethodMismatch);
        }

        let created = self
            .inner
            .accounts
            .create_provider_account(NewProviderAccount {
                provider_kind: request.provider_kind,
                display_name: request.display_name,
                auth_method: ProviderAuthMethod::SecretInput,
                status: ProviderAccountStatus::Unauthenticated,
                metadata: json!({
                    "secretConfigured": false,
                    "credentialRevision": 0,
                }),
            })
            .await
            .map_err(map_persistence_error)?;
        let gate = self.inner.gates.gate(&created.provider_account_id);
        let _guard = gate.lock().await;
        let secret_store = self.secret_store(&created);
        let snapshot = match secret_store.snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return self
                    .compensate_failed_create(
                        &created.provider_account_id,
                        &secret_store,
                        None,
                        ProviderAccountOperationError::ProviderUnavailable,
                    )
                    .await;
            }
        };
        if secret_store.save_api_key(&request.secret.0).is_err() {
            return self
                .compensate_failed_create(
                    &created.provider_account_id,
                    &secret_store,
                    Some(&snapshot),
                    ProviderAccountOperationError::ProviderUnavailable,
                )
                .await;
        }

        let updated = self
            .inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: created.provider_account_id.clone(),
                status: Some(authenticated_status()),
                metadata: Some(metadata_with_credential_state(
                    &created,
                    "secretConfigured",
                    true,
                )),
            })
            .await;
        match updated {
            Ok(account) => Ok(account),
            Err(error) => {
                self.compensate_failed_create(
                    &created.provider_account_id,
                    &secret_store,
                    Some(&snapshot),
                    map_persistence_error(error),
                )
                .await
            }
        }
    }

    async fn compensate_failed_create(
        &self,
        provider_account_id: &str,
        secret_store: &SecretInputStore,
        snapshot: Option<&FileSnapshot>,
        primary: ProviderAccountOperationError,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let compensated = compensate_failed_create_transaction(
            || {
                snapshot.map_or_else(
                    || secret_store.clear_api_key().map_err(|_| ()),
                    |snapshot| secret_store.restore(snapshot).map_err(|_| ()),
                )
            },
            || {
                self.inner
                    .accounts
                    .delete_provider_account(provider_account_id)
            },
        )
        .await;
        if !compensated {
            self.log_compensation_failure("create_secret_account", provider_account_id);
            return Err(ProviderAccountOperationError::CompensationFailed);
        }
        Err(primary)
    }

    pub(super) async fn save_secret_impl(
        &self,
        request: SaveProviderAccountSecretRequest,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let gate = self.inner.gates.gate(&request.provider_account_id);
        let _guard = gate.lock().await;
        let account = self
            .secret_input_account(&request.provider_account_id)
            .await?;
        let secret_store = self.secret_store(&account);
        let snapshot = secret_store
            .snapshot()
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        if secret_store.save_api_key(&request.secret.0).is_err() {
            return if secret_store.restore(&snapshot).is_err() {
                self.log_compensation_failure("save_provider_secret", &account.provider_account_id);
                Err(ProviderAccountOperationError::CompensationFailed)
            } else {
                Err(ProviderAccountOperationError::ProviderUnavailable)
            };
        }
        match self
            .inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: account.provider_account_id.clone(),
                status: Some(authenticated_status()),
                metadata: Some(metadata_with_credential_state(
                    &account,
                    "secretConfigured",
                    true,
                )),
            })
            .await
        {
            Ok(account) => Ok(account),
            Err(error) => {
                if secret_store.restore(&snapshot).is_err() {
                    self.log_compensation_failure(
                        "save_provider_secret",
                        &account.provider_account_id,
                    );
                    Err(ProviderAccountOperationError::CompensationFailed)
                } else {
                    Err(map_persistence_error(error))
                }
            }
        }
    }

    pub(super) async fn clear_secret_impl(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let account = self.secret_input_account(provider_account_id).await?;
        let secret_store = self.secret_store(&account);
        let snapshot = secret_store
            .snapshot()
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        if secret_store.clear_api_key().is_err() {
            return if secret_store.restore(&snapshot).is_err() {
                self.log_compensation_failure(
                    "clear_provider_secret",
                    &account.provider_account_id,
                );
                Err(ProviderAccountOperationError::CompensationFailed)
            } else {
                Err(ProviderAccountOperationError::ProviderUnavailable)
            };
        }
        match self
            .inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: account.provider_account_id.clone(),
                status: Some(unauthenticated_status()),
                metadata: Some(metadata_with_credential_state(
                    &account,
                    "secretConfigured",
                    false,
                )),
            })
            .await
        {
            Ok(account) => Ok(account),
            Err(error) => {
                if secret_store.restore(&snapshot).is_err() {
                    self.log_compensation_failure(
                        "clear_provider_secret",
                        &account.provider_account_id,
                    );
                    Err(ProviderAccountOperationError::CompensationFailed)
                } else {
                    Err(map_persistence_error(error))
                }
            }
        }
    }

    pub(super) async fn delete_account_impl(
        &self,
        provider_account_id: &str,
    ) -> Result<bool, ProviderAccountOperationError> {
        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let Some(account) = self
            .inner
            .accounts
            .provider_account(provider_account_id)
            .await
            .map_err(map_persistence_error)?
        else {
            return Ok(false);
        };
        if account.is_default {
            return Err(ProviderAccountOperationError::ProtectedAccount);
        }

        let quarantine = self.quarantine_account_home(&account)?;
        match self
            .inner
            .accounts
            .delete_provider_account(provider_account_id)
            .await
        {
            Ok(true) => {
                if let Some(quarantine) = quarantine
                    && fs::remove_dir_all(&quarantine.quarantine_path).is_err()
                {
                    self.log_cleanup_failure(provider_account_id);
                }
                Ok(true)
            }
            Ok(false) => {
                self.restore_quarantine(quarantine.as_ref(), provider_account_id)?;
                Err(ProviderAccountOperationError::Conflict)
            }
            Err(error) => {
                self.restore_quarantine(quarantine.as_ref(), provider_account_id)?;
                Err(map_persistence_error(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::compensate_failed_create_transaction;

    #[tokio::test]
    async fn failed_credential_rollback_preserves_durable_account_evidence() {
        let durable_delete_called = AtomicBool::new(false);

        let compensated = compensate_failed_create_transaction(
            || Err::<(), ()>(()),
            || {
                durable_delete_called.store(true, Ordering::SeqCst);
                async { Ok::<bool, ()>(true) }
            },
        )
        .await;

        assert!(!compensated);
        assert!(!durable_delete_called.load(Ordering::SeqCst));
    }
}
