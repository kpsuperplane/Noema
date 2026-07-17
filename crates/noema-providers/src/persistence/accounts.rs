//! Provider account persistence boundary.

use std::sync::Arc;

use serde_json::Value;

use crate::{
    NewProviderAccount, PersistedProviderAccountRecord, ProviderAccountStatus,
    ProviderPersistenceFuture,
};

/// Safe status fields updated together for one provider account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccountStatusUpdate {
    /// Resulting account readiness status.
    pub status: ProviderAccountStatus,
    /// Stable non-secret provider error code.
    pub error_code: Option<String>,
    /// Fixed non-secret provider error message.
    pub error_message: Option<String>,
}

/// Atomic provider-account update.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateProviderAccountRequest {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Optional status replacement.
    pub status: Option<ProviderAccountStatusUpdate>,
    /// Optional full replacement for safe non-secret account metadata.
    pub metadata: Option<Value>,
}

/// Provider account reads and guarded mutations required by provider services.
pub trait ProviderAccountPersistence: Send + Sync {
    /// Return one account by id.
    fn provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>>;

    /// Return the active default account for one provider.
    fn active_provider_account<'a>(
        &'a self,
        provider_kind: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>>;

    /// Return all active default provider accounts.
    fn active_default_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>>;

    /// Return all active provider accounts.
    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>>;

    /// Return all durable provider accounts.
    fn provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>>;

    /// Create one user-managed provider account.
    fn create_provider_account(
        &self,
        request: NewProviderAccount,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord>;

    /// Atomically replace the requested account fields.
    fn update_provider_account(
        &self,
        request: UpdateProviderAccountRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord>;

    /// Delete one unprotected user-managed provider account.
    fn delete_provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, bool>;
}

/// Clonable provider account persistence handle.
pub type ProviderAccountPersistenceHandle = Arc<dyn ProviderAccountPersistence>;
