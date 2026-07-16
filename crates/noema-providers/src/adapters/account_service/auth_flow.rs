use super::{
    ProviderAccountService,
    helpers::{
        authenticated_status, credential_revision, foundation_availability_error_code,
        map_persistence_error, metadata_with_credential_revision, validate_account_identity,
    },
};
use crate::adapters::{
    codex::{
        catalog::{fetch_provider_model_catalog, persist_model_catalog_refresh},
        oauth::{CodexDeviceAuthOutcome, CodexTokenStore},
    },
    foundation::FoundationLocalProvider,
};
use crate::{
    CODEX_PROVIDER, CodexDeviceAuthRequest, CodexOAuthConfig, CodexOAuthTokens,
    DEFAULT_FOUNDATION_LOCAL_PROFILE, FoundationLocalProviderConfig, ProviderAccountOperationError,
    ProviderAccountRecord, ProviderAccountStatus, ProviderAccountStatusUpdate,
    ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    StartProviderAuthRequest, UpdateProviderAccountRequest,
};

impl ProviderAccountService {
    pub(super) async fn active_accounts_impl(
        &self,
    ) -> Result<Vec<ProviderAccountRecord>, ProviderAccountOperationError> {
        let accounts = self
            .inner
            .accounts
            .active_provider_accounts()
            .await
            .map_err(map_persistence_error)?;
        for account in &accounts {
            if account.provider_kind == "foundation_local"
                && account.is_default
                && account.status != ProviderAccountStatus::Authenticated
            {
                let _ = self.reconcile_foundation_account(account.clone()).await;
            }
        }
        self.inner
            .accounts
            .active_provider_accounts()
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn start_auth_impl(
        &self,
        request: StartProviderAuthRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderAccountOperationError> {
        if request.provider_kind != CODEX_PROVIDER {
            return Err(ProviderAccountOperationError::UnsupportedProvider);
        }
        if request.method != ProviderAuthMethod::OauthDeviceCode {
            return Err(ProviderAccountOperationError::AuthMethodMismatch);
        }
        let gate = self.inner.gates.gate(&request.provider_account_id);
        let account = {
            let _guard = gate.lock().await;
            let account = self
                .require_active_account(&request.provider_account_id)
                .await?;
            validate_account_identity(&account, &request.provider_kind, request.method)?;
            account
        };
        let session = self
            .inner
            .auth
            .begin_codex_device_code(CodexDeviceAuthRequest {
                provider_account_id: account.provider_account_id.clone(),
                account_home: self.account_home(&account),
                oauth: CodexOAuthConfig::default(),
                attempt_timeout: None,
            })
            .await
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        let attempt = session.attempt.clone();
        let attempt_id = attempt.attempt_id.clone();
        let service = self.clone();
        tokio::spawn(async move {
            service
                .complete_auth_attempt(attempt_id, account, session.completion.await)
                .await;
        });
        Ok(attempt)
    }

    async fn complete_auth_attempt(
        &self,
        attempt_id: String,
        account: ProviderAccountRecord,
        outcome: CodexDeviceAuthOutcome,
    ) {
        let (status, error_code, error_message) = match outcome {
            CodexDeviceAuthOutcome::Completed(tokens) => {
                match self.publish_codex_tokens(&account, &tokens).await {
                    Ok(()) => (ProviderAuthAttemptStatus::Completed, None, None),
                    Err(error) => (
                        ProviderAuthAttemptStatus::Failed,
                        Some("provider_auth_publication_failed".to_string()),
                        Some(error.to_string()),
                    ),
                }
            }
            CodexDeviceAuthOutcome::Cancelled => (ProviderAuthAttemptStatus::Cancelled, None, None),
            CodexDeviceAuthOutcome::Expired => (
                ProviderAuthAttemptStatus::Expired,
                Some("provider_auth_expired".to_string()),
                Some("provider auth expired".to_string()),
            ),
            CodexDeviceAuthOutcome::Failed {
                error_code,
                error_message,
            } => (
                ProviderAuthAttemptStatus::Failed,
                Some(error_code),
                Some(error_message),
            ),
        };
        self.inner
            .auth
            .mark_attempt_terminal(&attempt_id, status, error_code, error_message)
            .await;
        self.inner.auth.remove_attempt_runtime(&attempt_id).await;
    }

    pub(super) async fn publish_codex_tokens(
        &self,
        expected_account: &ProviderAccountRecord,
        tokens: &CodexOAuthTokens,
    ) -> Result<(), ProviderAccountOperationError> {
        let gate = self.inner.gates.gate(&expected_account.provider_account_id);
        let _guard = gate.lock().await;
        let current = self
            .require_active_account(&expected_account.provider_account_id)
            .await?;
        validate_account_identity(
            &current,
            CODEX_PROVIDER,
            ProviderAuthMethod::OauthDeviceCode,
        )?;
        if current.account_key != expected_account.account_key {
            return Err(ProviderAccountOperationError::Conflict);
        }
        let token_store = CodexTokenStore::new(self.account_home(&current));
        let snapshot = token_store
            .snapshot()
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        if token_store.write(tokens).is_err() {
            return if token_store.restore(&snapshot).is_err() {
                self.log_compensation_failure(
                    "complete_provider_auth",
                    &current.provider_account_id,
                );
                Err(ProviderAccountOperationError::CompensationFailed)
            } else {
                Err(ProviderAccountOperationError::ProviderUnavailable)
            };
        }
        let metadata = metadata_with_credential_revision(&current);
        if let Err(error) = self
            .inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: current.provider_account_id.clone(),
                status: Some(authenticated_status()),
                metadata: Some(metadata),
            })
            .await
        {
            if token_store.restore(&snapshot).is_err() {
                self.log_compensation_failure(
                    "complete_provider_auth",
                    &current.provider_account_id,
                );
                return Err(ProviderAccountOperationError::CompensationFailed);
            }
            return Err(map_persistence_error(error));
        }
        Ok(())
    }

    pub(super) async fn reconcile_account_impl(
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
        if account.provider_kind == "foundation_local" {
            return self.reconcile_foundation_account(account).await;
        }
        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let account = self.require_active_account(provider_account_id).await?;
        let credential_present = match account.auth_method {
            ProviderAuthMethod::SecretInput => self.secret_store(&account).load_api_key().is_ok(),
            ProviderAuthMethod::OauthDeviceCode if account.provider_kind == CODEX_PROVIDER => {
                CodexTokenStore::new(self.account_home(&account)).has_usable_tokens()
            }
            ProviderAuthMethod::None | ProviderAuthMethod::ExternalManual => return Ok(account),
            ProviderAuthMethod::OauthDeviceCode => {
                return Err(ProviderAccountOperationError::UnsupportedProvider);
            }
        };
        let desired = if credential_present {
            ProviderAccountStatus::Authenticated
        } else {
            ProviderAccountStatus::Unauthenticated
        };
        if account.status == desired {
            return Ok(account);
        }
        self.inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: account.provider_account_id,
                status: Some(ProviderAccountStatusUpdate {
                    status: desired,
                    error_code: None,
                    error_message: None,
                }),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)
    }

    async fn reconcile_foundation_account(
        &self,
        account: ProviderAccountRecord,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
            bridge_path: None,
            system_errors: Some(self.inner.system_errors.clone()),
        })
        .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        let availability = provider.check_availability().await;
        let gate = self.inner.gates.gate(&account.provider_account_id);
        let _guard = gate.lock().await;
        let current = self
            .require_active_account(&account.provider_account_id)
            .await?;
        if current.provider_kind != account.provider_kind
            || current.account_key != account.account_key
            || !current.is_default
        {
            return Err(ProviderAccountOperationError::Conflict);
        }
        let status = match availability {
            Ok(()) => authenticated_status(),
            Err(error) => ProviderAccountStatusUpdate {
                status: ProviderAccountStatus::Unavailable,
                error_code: Some(foundation_availability_error_code(&error).to_string()),
                error_message: Some(error.to_string()),
            },
        };
        self.inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: current.provider_account_id,
                status: Some(status),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn refresh_model_catalog_impl(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let account = self.require_active_account(provider_account_id).await?;
        let expected_revision = credential_revision(&account);
        let Some(catalog) = fetch_provider_model_catalog(&self.inner.credentials, &account)
            .await
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?
        else {
            return Ok(account);
        };

        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let current = self.require_active_account(provider_account_id).await?;
        if current.provider_kind != account.provider_kind
            || current.account_key != account.account_key
            || credential_revision(&current) != expected_revision
        {
            return Err(ProviderAccountOperationError::Conflict);
        }
        persist_model_catalog_refresh(self.inner.catalogs.as_ref(), &current, catalog)
            .await
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)
    }
}
