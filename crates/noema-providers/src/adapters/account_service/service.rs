use std::sync::Arc;

use noema_home::{NoemaPaths, SystemErrorLogger};

use super::{
    credentials::{ProviderCredentialAccessHandle, ProviderCredentialAccessService},
    gates::AccountGateRegistry,
};
use crate::adapters::{auth::ProviderAuthManager, codex::oauth::CodexOAuthClient};
use crate::{
    CodexOAuthConfig, CreateSecretProviderAccountRequest, ProviderAccountOperationError,
    ProviderAccountOperationFuture, ProviderAccountOperations, ProviderAccountOperationsHandle,
    ProviderAccountPersistenceHandle, ProviderAccountRecord, ProviderAuthAttemptView,
    ProviderModelCatalogPersistenceHandle, SaveProviderAccountSecretRequest,
    StartProviderAuthRequest, provider_account_catalog,
};

#[path = "auth_flow.rs"]
mod auth_flow;
#[path = "helpers.rs"]
mod helpers;
#[path = "mutations.rs"]
mod mutations;

/// Root-bound provider account orchestration with compensation-safe credentials.
#[derive(Clone)]
pub struct ProviderAccountService {
    inner: Arc<ProviderAccountServiceInner>,
}

struct ProviderAccountServiceInner {
    paths: NoemaPaths,
    accounts: ProviderAccountPersistenceHandle,
    catalogs: ProviderModelCatalogPersistenceHandle,
    system_errors: SystemErrorLogger,
    auth: ProviderAuthManager,
    gates: AccountGateRegistry,
    credentials: ProviderCredentialAccessHandle,
}

impl std::fmt::Debug for ProviderAccountService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderAccountService")
            .finish_non_exhaustive()
    }
}

impl ProviderAccountService {
    /// Build the provider-owned account and credential services.
    ///
    /// # Errors
    ///
    /// Returns a provider configuration error when the default Codex OAuth
    /// client cannot be constructed.
    pub fn new(
        paths: NoemaPaths,
        accounts: ProviderAccountPersistenceHandle,
        catalogs: ProviderModelCatalogPersistenceHandle,
        system_errors: SystemErrorLogger,
    ) -> Result<Self, crate::ProviderError> {
        let gates = AccountGateRegistry::new();
        let codex_oauth = CodexOAuthClient::new(CodexOAuthConfig::default())?;
        let credentials: ProviderCredentialAccessHandle =
            Arc::new(ProviderCredentialAccessService::new(
                paths.clone(),
                accounts.clone(),
                gates.clone(),
                codex_oauth,
            ));
        Ok(Self::from_parts(
            paths,
            accounts,
            catalogs,
            system_errors,
            gates,
            credentials,
        ))
    }

    pub(crate) fn from_parts(
        paths: NoemaPaths,
        accounts: ProviderAccountPersistenceHandle,
        catalogs: ProviderModelCatalogPersistenceHandle,
        system_errors: SystemErrorLogger,
        gates: AccountGateRegistry,
        credentials: ProviderCredentialAccessHandle,
    ) -> Self {
        Self {
            inner: Arc::new(ProviderAccountServiceInner {
                paths,
                accounts,
                catalogs,
                system_errors,
                auth: ProviderAuthManager::new(),
                gates,
                credentials,
            }),
        }
    }

    /// Return the object-safe account operations handle used by API layers.
    #[must_use]
    pub fn operations(&self) -> ProviderAccountOperationsHandle {
        Arc::new(self.clone())
    }

    /// Return pathless credential access sharing this service's account gates.
    #[must_use]
    pub fn credentials(&self) -> ProviderCredentialAccessHandle {
        self.inner.credentials.clone()
    }
}

impl ProviderAccountOperations for ProviderAccountService {
    fn account_catalog(&self) -> Vec<crate::ProviderAccountCatalogEntry> {
        provider_account_catalog()
    }

    fn active_accounts(&self) -> ProviderAccountOperationFuture<'_, Vec<ProviderAccountRecord>> {
        Box::pin(self.active_accounts_impl())
    }

    fn create_secret_account(
        &self,
        request: CreateSecretProviderAccountRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
        Box::pin(self.create_secret_account_impl(request))
    }

    fn save_secret(
        &self,
        request: SaveProviderAccountSecretRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
        Box::pin(self.save_secret_impl(request))
    }

    fn clear_secret<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(self.clear_secret_impl(provider_account_id))
    }

    fn delete_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, bool> {
        Box::pin(self.delete_account_impl(provider_account_id))
    }

    fn start_auth(
        &self,
        request: StartProviderAuthRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAuthAttemptView> {
        Box::pin(self.start_auth_impl(request))
    }

    fn auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
        Box::pin(async move {
            self.inner
                .auth
                .poll_attempt(attempt_id)
                .await
                .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)
        })
    }

    fn cancel_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
        Box::pin(async move {
            self.inner
                .auth
                .cancel_attempt(attempt_id)
                .await
                .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)
        })
    }

    fn reconcile_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(self.reconcile_account_impl(provider_account_id))
    }

    fn refresh_model_catalog<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(self.refresh_model_catalog_impl(provider_account_id))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
