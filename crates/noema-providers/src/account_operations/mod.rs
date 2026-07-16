//! Provider account orchestration contracts.

mod error;
mod requests;

use std::{future::Future, pin::Pin, sync::Arc};

pub use error::ProviderAccountOperationError;
pub use requests::{
    CreateSecretProviderAccountRequest, SaveProviderAccountSecretRequest, StartProviderAuthRequest,
};

use crate::{ProviderAccountCatalogEntry, ProviderAccountRecord, ProviderAuthAttemptView};

/// Boxed future returned by provider account operations.
pub type ProviderAccountOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderAccountOperationError>> + Send + 'a>>;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderAccountStatus, ProviderAuthMethod, capabilities_for_provider_account};
    use serde_json::json;

    #[derive(Debug)]
    struct FakeAccountOperations;

    impl ProviderAccountOperations for FakeAccountOperations {
        fn account_catalog(&self) -> Vec<ProviderAccountCatalogEntry> {
            Vec::new()
        }

        fn active_accounts(
            &self,
        ) -> ProviderAccountOperationFuture<'_, Vec<ProviderAccountRecord>> {
            Box::pin(async { Ok(vec![account()]) })
        }

        fn create_secret_account(
            &self,
            _request: CreateSecretProviderAccountRequest,
        ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
            Box::pin(async { Ok(account()) })
        }

        fn save_secret(
            &self,
            _request: SaveProviderAccountSecretRequest,
        ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
            Box::pin(async { Ok(account()) })
        }

        fn clear_secret<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
            Box::pin(async { Ok(account()) })
        }

        fn delete_account<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, bool> {
            Box::pin(async { Ok(true) })
        }

        fn start_auth(
            &self,
            request: StartProviderAuthRequest,
        ) -> ProviderAccountOperationFuture<'_, ProviderAuthAttemptView> {
            Box::pin(async move {
                Ok(ProviderAuthAttemptView {
                    attempt_id: "attempt:test".to_string(),
                    provider_kind: request.provider_kind,
                    provider_account_id: request.provider_account_id,
                    method: request.method,
                    status: crate::ProviderAuthAttemptStatus::Starting,
                    verification_url: None,
                    user_code: None,
                    instructions: None,
                    error_code: None,
                    error_message: None,
                })
            })
        }

        fn auth_attempt<'a>(
            &'a self,
            _attempt_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
            Box::pin(async { Ok(None) })
        }

        fn cancel_auth_attempt<'a>(
            &'a self,
            _attempt_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
            Box::pin(async { Ok(None) })
        }

        fn reconcile_account<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
            Box::pin(async { Ok(account()) })
        }

        fn refresh_model_catalog<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
            Box::pin(async { Ok(account()) })
        }
    }

    fn account() -> ProviderAccountRecord {
        ProviderAccountRecord {
            provider_account_id: "provider_account:exa:test".to_string(),
            provider_kind: "exa".to_string(),
            account_key: "test".to_string(),
            display_name: "Exa".to_string(),
            auth_method: ProviderAuthMethod::SecretInput,
            is_active: true,
            is_default: false,
            status: ProviderAccountStatus::Authenticated,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({"secretConfigured": true}),
            capabilities: capabilities_for_provider_account(
                "exa",
                "test",
                ProviderAccountStatus::Authenticated,
            ),
        }
    }

    #[tokio::test]
    async fn account_operations_are_object_safe_and_pathless() {
        let operations: ProviderAccountOperationsHandle = Arc::new(FakeAccountOperations);
        let accounts = operations.active_accounts().await.expect("active accounts");

        assert_eq!(accounts, vec![account()]);
    }
}
