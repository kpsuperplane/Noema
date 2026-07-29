//! Provider account orchestration contracts.

mod error;
mod requests;

use futures_util::Stream;
use std::{future::Future, pin::Pin, sync::Arc};

pub use error::ProviderAccountOperationError;
pub use requests::{
    CompleteProviderAuthCallbackRequest, CreateSecretProviderAccountRequest,
    SaveProviderAccountSecretRequest, StartProviderAuthRequest,
};

use crate::{ProviderAccountCatalogEntry, ProviderAccountRecord, ProviderAuthAttemptView};

/// Boxed future returned by provider account operations.
pub type ProviderAccountOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderAccountOperationError>> + Send + 'a>>;

/// Pushed provider-auth attempt state changes.
pub type ProviderAuthAttemptEventStream =
    Pin<Box<dyn Stream<Item = ProviderAuthAttemptView> + Send>>;

/// Object-safe provider account and authentication orchestration.
pub trait ProviderAccountOperations: Send + Sync {
    /// Return provider account kinds that can be configured.
    fn account_catalog(&self) -> Vec<ProviderAccountCatalogEntry>;

    /// Return active provider accounts after provider-owned status reconciliation.
    fn active_accounts(&self) -> ProviderAccountOperationFuture<'_, Vec<ProviderAccountRecord>>;

    /// Create a secret-backed provider account.
    fn create_secret_account(
        &self,
        request: CreateSecretProviderAccountRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord>;

    /// Replace the write-only secret for one provider account.
    fn save_secret(
        &self,
        request: SaveProviderAccountSecretRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord>;

    /// Clear the write-only secret for one provider account.
    fn clear_secret<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord>;

    /// Delete one non-default provider account.
    fn delete_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, bool>;

    /// Start one provider-owned authentication attempt.
    fn start_auth(
        &self,
        request: StartProviderAuthRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAuthAttemptView>;

    /// Claim and complete a provider OAuth callback exactly once.
    fn complete_auth_callback(
        &self,
        request: CompleteProviderAuthCallbackRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAuthAttemptView>;

    /// Subscribe to pushed state changes for provider authentication attempts.
    fn subscribe_auth_attempts(&self) -> ProviderAuthAttemptEventStream;

    /// Return a safe view of one authentication attempt.
    fn auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>>;

    /// Cancel one authentication attempt, if it is still active.
    fn cancel_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>>;

    /// Record a provider-reported authentication failure for the exact
    /// credential revision used by a runtime request.
    fn record_auth_failure<'a>(
        &'a self,
        provider_account_id: &'a str,
        expected_credential_revision: u64,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord>;

    /// Reconcile provider-owned credentials and durable account status.
    fn reconcile_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord>;

    /// Refresh one account's provider model catalog from fresh durable state.
    fn refresh_model_catalog<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord>;
}

/// Shared provider account operations handle.
pub type ProviderAccountOperationsHandle = Arc<dyn ProviderAccountOperations>;
