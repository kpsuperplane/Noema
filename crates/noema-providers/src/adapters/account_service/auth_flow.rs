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
    CODEX_PROVIDER, CodexDeviceAuthRequest, CodexOAuthTokens, DEFAULT_FOUNDATION_LOCAL_PROFILE,
    FoundationLocalProviderConfig, ProviderAccountOperationError, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAccountStatusUpdate, ProviderAuthAttemptStatus,
    ProviderAuthAttemptView, ProviderAuthMethod, StartProviderAuthRequest,
    UpdateProviderAccountRequest,
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
                oauth: self.inner.codex_oauth.clone(),
                attempt_timeout: None,
            })
            .await
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        let attempt = session.attempt.clone();
        let attempt_id = attempt.attempt_id.clone();
        let expected_credential_revision = credential_revision(&account);
        let mut auth_tasks = self.inner.auth_tasks.lock().await;
        if self.inner.auth.is_shutting_down().await {
            return Err(ProviderAccountOperationError::ProviderUnavailable);
        }
        let service = self.clone();
        auth_tasks.push(tokio::spawn(async move {
            service
                .complete_auth_attempt(
                    attempt_id,
                    account,
                    expected_credential_revision,
                    session.completion.await,
                )
                .await;
        }));
        Ok(attempt)
    }

    pub(super) async fn complete_auth_attempt(
        &self,
        attempt_id: String,
        account: ProviderAccountRecord,
        expected_credential_revision: u64,
        outcome: CodexDeviceAuthOutcome,
    ) {
        if matches!(outcome, CodexDeviceAuthOutcome::Cancelled) {
            self.persist_cancelled_attempt_if_current(
                &attempt_id,
                &account,
                expected_credential_revision,
            )
            .await;
            return;
        }
        if !self.inner.auth.claim_attempt_completion(&attempt_id).await {
            self.persist_cancelled_attempt_if_current(
                &attempt_id,
                &account,
                expected_credential_revision,
            )
            .await;
            return;
        }

        let (status, error_code, error_message) = match outcome {
            CodexDeviceAuthOutcome::Completed(tokens) => {
                match self
                    .publish_codex_tokens(&account, expected_credential_revision, &tokens)
                    .await
                {
                    Ok(()) => (ProviderAuthAttemptStatus::Completed, None, None),
                    Err(error) => (
                        ProviderAuthAttemptStatus::Failed,
                        Some("provider_auth_publication_failed".to_string()),
                        Some(auth_publication_failure_message(&error).to_string()),
                    ),
                }
            }
            CodexDeviceAuthOutcome::Cancelled => unreachable!("cancelled outcomes return above"),
            CodexDeviceAuthOutcome::Expired => (
                ProviderAuthAttemptStatus::Expired,
                Some("provider_auth_expired".to_string()),
                Some("Provider authentication expired".to_string()),
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
        if status != ProviderAuthAttemptStatus::Completed {
            let _ = self
                .persist_auth_terminal(
                    &attempt_id,
                    &account,
                    expected_credential_revision,
                    status,
                    error_code.clone(),
                    error_message.clone(),
                )
                .await;
        }
        self.inner
            .auth
            .finish_claimed_attempt(&attempt_id, status, error_code, error_message)
            .await;
    }

    async fn persist_cancelled_attempt_if_current(
        &self,
        attempt_id: &str,
        account: &ProviderAccountRecord,
        expected_credential_revision: u64,
    ) {
        let Ok(Some(attempt)) = self.inner.auth.poll_attempt(attempt_id).await else {
            return;
        };
        if attempt.status != ProviderAuthAttemptStatus::Cancelled {
            return;
        }
        let _ = self
            .persist_auth_terminal(
                attempt_id,
                account,
                expected_credential_revision,
                ProviderAuthAttemptStatus::Cancelled,
                Some("provider_auth_cancelled".to_string()),
                Some("Provider authentication was cancelled".to_string()),
            )
            .await;
    }

    pub(super) async fn publish_codex_tokens(
        &self,
        expected_account: &ProviderAccountRecord,
        expected_credential_revision: u64,
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
        if current.account_key != expected_account.account_key
            || credential_revision(&current) != expected_credential_revision
        {
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

    pub(super) async fn persist_auth_terminal(
        &self,
        attempt_id: &str,
        expected_account: &ProviderAccountRecord,
        expected_credential_revision: u64,
        attempt_status: ProviderAuthAttemptStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Result<(), ProviderAccountOperationError> {
        debug_assert!(matches!(
            attempt_status,
            ProviderAuthAttemptStatus::Failed
                | ProviderAuthAttemptStatus::Expired
                | ProviderAuthAttemptStatus::Cancelled
        ));
        let gate = self.inner.gates.gate(&expected_account.provider_account_id);
        let _guard = gate.lock().await;
        if !self.inner.auth.is_latest_attempt(attempt_id).await {
            return Err(ProviderAccountOperationError::Conflict);
        }
        let current = self
            .require_active_account(&expected_account.provider_account_id)
            .await?;
        validate_account_identity(
            &current,
            CODEX_PROVIDER,
            ProviderAuthMethod::OauthDeviceCode,
        )?;
        if current.account_key != expected_account.account_key
            || credential_revision(&current) != expected_credential_revision
            || current.status != expected_account.status
            || current.last_checked_at != expected_account.last_checked_at
            || current.last_error_code != expected_account.last_error_code
            || current.last_error_message != expected_account.last_error_message
        {
            return Err(ProviderAccountOperationError::Conflict);
        }
        self.inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: current.provider_account_id,
                status: Some(ProviderAccountStatusUpdate {
                    status: ProviderAccountStatus::Unauthenticated,
                    error_code,
                    error_message,
                }),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)?;
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

    pub(super) async fn record_auth_failure_impl(
        &self,
        provider_account_id: &str,
        expected_credential_revision: u64,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let current = self.secret_input_account(provider_account_id).await?;
        if credential_revision(&current) != expected_credential_revision {
            return Err(ProviderAccountOperationError::Conflict);
        }
        self.inner
            .accounts
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: current.provider_account_id,
                status: Some(ProviderAccountStatusUpdate {
                    status: ProviderAccountStatus::Unauthenticated,
                    error_code: Some("auth_failed".to_string()),
                    error_message: Some("Provider rejected the configured credentials".to_string()),
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

fn auth_publication_failure_message(error: &ProviderAccountOperationError) -> &'static str {
    match error {
        ProviderAccountOperationError::Conflict => {
            "Provider account credentials changed during authentication"
        }
        ProviderAccountOperationError::CompensationFailed => "Provider credential recovery failed",
        _ => "Provider credentials could not be saved",
    }
}
