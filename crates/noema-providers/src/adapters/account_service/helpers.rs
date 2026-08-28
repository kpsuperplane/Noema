use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use noema_home::SystemErrorEvent;
use serde_json::{Map, Value, json};

use super::ProviderAccountService;
use crate::adapters::{SecretInputStore, foundation::FoundationBridgeError};
use crate::{
    ProviderAccountOperationError, ProviderAccountRecord, ProviderAccountStatus,
    ProviderAccountStatusUpdate, ProviderAuthMethod, provider_account_catalog_entry,
};

static NEXT_QUARANTINE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct QuarantinedAccountHome {
    pub(super) original_path: PathBuf,
    pub(super) quarantine_path: PathBuf,
}

impl ProviderAccountService {
    pub(super) async fn secret_input_account(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let account = self.require_active_account(provider_account_id).await?;
        let Some(catalog) = provider_account_catalog_entry(&account.provider_kind) else {
            return Err(ProviderAccountOperationError::UnsupportedProvider);
        };
        if !catalog
            .supported_auth_methods
            .contains(&ProviderAuthMethod::SecretInput)
        {
            return Err(ProviderAccountOperationError::UnsupportedProvider);
        }
        if !catalog
            .supported_auth_methods
            .contains(&account.auth_method)
        {
            return Err(ProviderAccountOperationError::AuthMethodMismatch);
        }
        Ok(account)
    }

    pub(super) async fn require_active_account(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let account = self
            .inner
            .accounts
            .provider_account(provider_account_id)
            .await
            .map_err(map_persistence_error)?
            .ok_or(ProviderAccountOperationError::AccountNotFound)?;
        let account = account;
        if !account.is_active {
            return Err(ProviderAccountOperationError::AccountInactive);
        }
        Ok(account)
    }

    pub(super) fn secret_store(&self, account: &ProviderAccountRecord) -> SecretInputStore {
        SecretInputStore::new(self.account_home(account))
    }

    pub(super) fn account_home(&self, account: &ProviderAccountRecord) -> PathBuf {
        self.inner
            .paths
            .provider_account_home(&account.provider_kind, &account.account_key)
    }

    pub(super) fn quarantine_account_home(
        &self,
        account: &ProviderAccountRecord,
    ) -> Result<Option<QuarantinedAccountHome>, ProviderAccountOperationError> {
        let account_home = self.account_home(account);
        if !account_home.exists() {
            return Ok(None);
        }
        let parent = account_home
            .parent()
            .ok_or(ProviderAccountOperationError::ProviderUnavailable)?;
        let quarantine_path = parent.join(format!(
            ".noema-delete-{}-{}",
            std::process::id(),
            NEXT_QUARANTINE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&account_home, &quarantine_path)
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        Ok(Some(QuarantinedAccountHome {
            original_path: account_home,
            quarantine_path,
        }))
    }

    pub(super) fn restore_quarantine(
        &self,
        quarantine: Option<&QuarantinedAccountHome>,
        provider_account_id: &str,
    ) -> Result<(), ProviderAccountOperationError> {
        let Some(quarantine) = quarantine else {
            return Ok(());
        };
        if fs::rename(&quarantine.quarantine_path, &quarantine.original_path).is_err() {
            self.log_compensation_failure("delete_provider_account", provider_account_id);
            return Err(ProviderAccountOperationError::CompensationFailed);
        }
        Ok(())
    }

    pub(super) fn log_compensation_failure(
        &self,
        operation: &'static str,
        provider_account_id: &str,
    ) {
        self.inner.system_errors.try_append(
            SystemErrorEvent::new(
                "provider_account_compensation_failed",
                "provider account recovery failed",
            )
            .with_context(json!({
                "operation": operation,
                "provider_account_id": provider_account_id,
            })),
        );
    }

    pub(super) fn log_cleanup_failure(&self, provider_account_id: &str) {
        self.inner.system_errors.try_append(
            SystemErrorEvent::new(
                "provider_account_quarantine_cleanup_failed",
                "provider account cleanup failed",
            )
            .with_context(json!({
                "provider_account_id": provider_account_id,
            })),
        );
    }
}

pub(super) fn validate_account_identity(
    account: &ProviderAccountRecord,
    provider_kind: &str,
    auth_method: ProviderAuthMethod,
) -> Result<(), ProviderAccountOperationError> {
    if account.provider_kind != provider_kind {
        return Err(ProviderAccountOperationError::ProviderMismatch);
    }
    if account.auth_method != auth_method {
        return Err(ProviderAccountOperationError::AuthMethodMismatch);
    }
    Ok(())
}

pub(super) fn authenticated_status() -> ProviderAccountStatusUpdate {
    ProviderAccountStatusUpdate {
        status: ProviderAccountStatus::Authenticated,
        error_code: None,
        error_message: None,
    }
}

pub(super) fn unauthenticated_status() -> ProviderAccountStatusUpdate {
    ProviderAccountStatusUpdate {
        status: ProviderAccountStatus::Unauthenticated,
        error_code: None,
        error_message: None,
    }
}

pub(super) fn metadata_with_credential_state(
    account: &ProviderAccountRecord,
    configured_field: &str,
    configured: bool,
) -> Value {
    let mut metadata = metadata_object(account);
    metadata.insert(configured_field.to_string(), Value::Bool(configured));
    increment_credential_revision(&mut metadata);
    Value::Object(metadata)
}

pub(super) fn metadata_with_credential_revision(account: &ProviderAccountRecord) -> Value {
    let mut metadata = metadata_object(account);
    increment_credential_revision(&mut metadata);
    Value::Object(metadata)
}

pub(super) fn credential_revision(account: &ProviderAccountRecord) -> u64 {
    account
        .metadata
        .get("credentialRevision")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn metadata_object(account: &ProviderAccountRecord) -> Map<String, Value> {
    account.metadata.as_object().cloned().unwrap_or_default()
}

fn increment_credential_revision(metadata: &mut Map<String, Value>) {
    let revision = metadata
        .get("credentialRevision")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    metadata.insert("credentialRevision".to_string(), Value::from(revision));
}

pub(super) fn foundation_availability_error_code(error: &FoundationBridgeError) -> &'static str {
    match error.code() {
        "foundation_unavailable" => "foundation_models_unavailable",
        code => code,
    }
}

pub(super) fn map_persistence_error(
    error: crate::ProviderPersistenceError,
) -> ProviderAccountOperationError {
    match error {
        crate::ProviderPersistenceError::AccountNotFound { .. } => {
            ProviderAccountOperationError::AccountNotFound
        }
        crate::ProviderPersistenceError::ProtectedAccount { .. } => {
            ProviderAccountOperationError::ProtectedAccount
        }
        crate::ProviderPersistenceError::AccountInUse { .. }
        | crate::ProviderPersistenceError::Conflict { .. }
        | crate::ProviderPersistenceError::ProviderInstanceReferenced { .. }
        | crate::ProviderPersistenceError::ProviderInstanceRetiring { .. } => {
            ProviderAccountOperationError::Conflict
        }
        crate::ProviderPersistenceError::InvalidRequest { .. }
        | crate::ProviderPersistenceError::InstallationNotFound { .. }
        | crate::ProviderPersistenceError::InvalidInstallationTransition { .. }
        | crate::ProviderPersistenceError::ActiveInstallationConflict { .. }
        | crate::ProviderPersistenceError::Invariant { .. }
        | crate::ProviderPersistenceError::Persistence { .. } => {
            ProviderAccountOperationError::Persistence
        }
    }
}
