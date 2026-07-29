use super::{
    ProviderAccountService,
    helpers::{
        authenticated_status, credential_revision, foundation_availability_error_code,
        map_persistence_error, metadata_with_credential_revision, validate_account_identity,
    },
};
use crate::adapters::{
    SecretInputStore,
    codex::{
        catalog::{fetch_provider_model_catalog, persist_model_catalog_refresh},
        oauth::{CodexDeviceAuthOutcome, CodexTokenStore},
    },
    foundation::FoundationLocalProvider,
};
use crate::{
    CODEX_PROVIDER, CodexDeviceAuthRequest, CodexOAuthTokens, CompleteProviderAuthCallbackRequest,
    DEFAULT_FOUNDATION_LOCAL_PROFILE, FoundationLocalProviderConfig, NewProviderAccount,
    ProviderAccountOperationError, ProviderAccountRecord, ProviderAccountStatus,
    ProviderAccountStatusUpdate, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
    ProviderAuthMethod, ProviderModelProfile, StartProviderAuthRequest,
    UpdateProviderAccountRequest, provider_account_from_persisted,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};

impl ProviderAccountService {
    pub(super) async fn active_accounts_impl(
        &self,
    ) -> Result<Vec<ProviderAccountRecord>, ProviderAccountOperationError> {
        let accounts = self
            .inner
            .accounts
            .active_provider_accounts()
            .await
            .map_err(map_persistence_error)?
            .into_iter()
            .map(provider_account_from_persisted)
            .collect::<Vec<_>>();
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
            .map(|accounts| {
                accounts
                    .into_iter()
                    .map(provider_account_from_persisted)
                    .collect()
            })
    }

    pub(super) async fn start_auth_impl(
        &self,
        request: StartProviderAuthRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderAccountOperationError> {
        if request.provider_kind == "openrouter" {
            return self.start_openrouter_auth(request).await;
        }
        if request.provider_kind != CODEX_PROVIDER {
            return Err(ProviderAccountOperationError::UnsupportedProvider);
        }
        if request.method != ProviderAuthMethod::OauthDeviceCode {
            return Err(ProviderAccountOperationError::AuthMethodMismatch);
        }
        let gate = self.inner.gates.gate(&request.provider_account_id);
        let account = {
            let _guard = gate.lock().await;
            let account = match self
                .require_active_account(&request.provider_account_id)
                .await
            {
                Ok(account) => account,
                Err(ProviderAccountOperationError::AccountNotFound) => pending_codex_account(),
                Err(error) => return Err(error),
            };
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

    async fn start_openrouter_auth(
        &self,
        request: StartProviderAuthRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderAccountOperationError> {
        if request.method != ProviderAuthMethod::OauthPkce {
            return Err(ProviderAccountOperationError::AuthMethodMismatch);
        }
        let callback_base = request
            .callback_url
            .ok_or(ProviderAccountOperationError::ProviderUnavailable)?;
        let attempt_id = random_attempt_id()?;
        let mut callback = url::Url::parse(&callback_base)
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        callback
            .query_pairs_mut()
            .append_pair("attemptId", &attempt_id);
        let pkce = crate::adapters::openrouter::catalog::begin_pkce(callback.as_str())
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        let attempt = ProviderAuthAttemptView {
            attempt_id: attempt_id.clone(),
            provider_kind: "openrouter".to_string(),
            provider_account_id: "provider_account:openrouter:default".to_string(),
            method: ProviderAuthMethod::OauthPkce,
            status: ProviderAuthAttemptStatus::WaitingForUser,
            verification_url: Some(pkce.authorization_url),
            user_code: None,
            instructions: Some("Complete the connection in your browser.".to_string()),
            error_code: None,
            error_message: None,
        };
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        if !self
            .inner
            .auth
            .register_attempt(
                attempt.clone(),
                crate::adapters::auth::ProviderAuthAttemptRuntime::new(cancel),
            )
            .await
        {
            return Err(ProviderAccountOperationError::ProviderUnavailable);
        }
        self.inner
            .openrouter_pkce
            .lock()
            .await
            .insert(attempt_id.clone(), pkce.verifier);
        let service = self.clone();
        self.inner.auth_tasks.lock().await.push(tokio::spawn(async move {
            tokio::select! {
                _ = cancelled => {
                    service.inner.openrouter_pkce.lock().await.remove(&attempt_id);
                }
                () = tokio::time::sleep(crate::adapters::auth::DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT) => {
                    if service.inner.auth.claim_attempt_completion(&attempt_id).await {
                        service.inner.openrouter_pkce.lock().await.remove(&attempt_id);
                        service.inner.auth.finish_claimed_attempt(
                            &attempt_id,
                            ProviderAuthAttemptStatus::Expired,
                            Some("provider_auth_expired".to_string()),
                            Some("Provider authentication expired".to_string()),
                        ).await;
                    }
                }
            }
        }));
        Ok(attempt)
    }

    pub(super) async fn complete_auth_callback_impl(
        &self,
        request: CompleteProviderAuthCallbackRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderAccountOperationError> {
        if !self
            .inner
            .auth
            .claim_attempt_completion(&request.attempt_id)
            .await
        {
            return Err(ProviderAccountOperationError::Conflict);
        }
        let verifier = self
            .inner
            .openrouter_pkce
            .lock()
            .await
            .remove(&request.attempt_id)
            .ok_or(ProviderAccountOperationError::Conflict)?;
        let publication = match crate::adapters::openrouter::catalog::exchange_pkce_code(
            &request.code,
            &verifier,
        )
        .await
        {
            Ok((key, profiles)) => {
                self.publish_openrouter_key(&key, &profiles, ProviderAuthMethod::OauthPkce)
                    .await
            }
            Err(_) => Err(ProviderAccountOperationError::ProviderUnavailable),
        };
        let (status, code, message) = match publication {
            Ok(()) => (ProviderAuthAttemptStatus::Completed, None, None),
            Err(error) => (
                ProviderAuthAttemptStatus::Failed,
                Some("provider_auth_publication_failed".to_string()),
                Some(auth_publication_failure_message(&error).to_string()),
            ),
        };
        self.inner
            .auth
            .finish_claimed_attempt(&request.attempt_id, status, code, message)
            .await
            .ok_or(ProviderAccountOperationError::Conflict)
    }

    async fn publish_openrouter_key(
        &self,
        key: &str,
        profiles: &[ProviderModelProfile],
        auth_method: ProviderAuthMethod,
    ) -> Result<(), ProviderAccountOperationError> {
        let expected = pending_openrouter_account(auth_method, profiles)?;
        let gate = self.inner.gates.gate(&expected.provider_account_id);
        let _guard = gate.lock().await;
        let existing = self
            .inner
            .accounts
            .provider_account(&expected.provider_account_id)
            .await
            .map_err(map_persistence_error)?;
        let account_exists = existing.is_some();
        let mut current = existing
            .map(provider_account_from_persisted)
            .unwrap_or(expected);
        if account_exists {
            let mut metadata = current.metadata.clone();
            metadata["credentialRevision"] =
                serde_json::Value::from(credential_revision(&current).saturating_add(1));
            metadata["secretConfigured"] = serde_json::Value::Bool(true);
            ProviderModelProfile::write_account_metadata(&mut metadata, profiles)
                .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
            current.metadata = metadata;
        }
        let store = SecretInputStore::new(self.account_home(&current));
        let snapshot = store
            .snapshot()
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        store
            .save_api_key(key)
            .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
        let published = if account_exists {
            self.inner
                .accounts
                .update_provider_account(UpdateProviderAccountRequest {
                    provider_account_id: current.provider_account_id.clone(),
                    auth_method: Some(auth_method),
                    status: Some(authenticated_status()),
                    metadata: Some(current.metadata.clone()),
                })
                .await
        } else {
            self.inner
                .accounts
                .create_provider_account(NewProviderAccount {
                    provider_kind: "openrouter".to_string(),
                    display_name: Some("OpenRouter".to_string()),
                    auth_method,
                    status: ProviderAccountStatus::Authenticated,
                    metadata: current.metadata.clone(),
                })
                .await
        };
        let published = match published {
            Ok(account) => provider_account_from_persisted(account),
            Err(error) => {
                let _ = store.restore(&snapshot);
                return Err(map_persistence_error(error));
            }
        };
        if let Err(error) = self
            .initialize_model_account(&published, crate::DEFAULT_OPENROUTER_MODEL)
            .await
        {
            let credential_restored = store.restore(&snapshot).is_ok();
            let account_restored = account_exists
                || self
                    .inner
                    .accounts
                    .delete_provider_account(&published.provider_account_id)
                    .await
                    .is_ok_and(|deleted| deleted);
            if !credential_restored || !account_restored {
                return Err(ProviderAccountOperationError::CompensationFailed);
            }
            return Err(error);
        }
        Ok(())
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
        let existing = self
            .inner
            .accounts
            .provider_account(&expected_account.provider_account_id)
            .await
            .map_err(map_persistence_error)?;
        let account_exists = existing.is_some();
        let current = existing
            .map(provider_account_from_persisted)
            .unwrap_or_else(|| expected_account.clone());
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
        let publication = if account_exists {
            self.inner
                .accounts
                .update_provider_account(UpdateProviderAccountRequest {
                    provider_account_id: current.provider_account_id.clone(),
                    auth_method: None,
                    status: Some(authenticated_status()),
                    metadata: Some(metadata),
                })
                .await
        } else {
            self.inner
                .accounts
                .create_provider_account(NewProviderAccount {
                    provider_kind: CODEX_PROVIDER.to_string(),
                    display_name: Some("Codex".to_string()),
                    auth_method: ProviderAuthMethod::OauthDeviceCode,
                    status: ProviderAccountStatus::Authenticated,
                    metadata,
                })
                .await
        };
        let mut published = match publication {
            Ok(account) => provider_account_from_persisted(account),
            Err(error) => {
                if token_store.restore(&snapshot).is_err() {
                    self.log_compensation_failure(
                        "complete_provider_auth",
                        &current.provider_account_id,
                    );
                    return Err(ProviderAccountOperationError::CompensationFailed);
                }
                return Err(map_persistence_error(error));
            }
        };
        let catalog = match fetch_provider_model_catalog(&self.inner.credentials, &published).await
        {
            Ok(catalog) => catalog,
            Err(_) => {
                let credential_restored = token_store.restore(&snapshot).is_ok();
                let account_restored = account_exists
                    || self
                        .inner
                        .accounts
                        .delete_provider_account(&published.provider_account_id)
                        .await
                        .is_ok_and(|deleted| deleted);
                if !credential_restored || !account_restored {
                    return Err(ProviderAccountOperationError::CompensationFailed);
                }
                return Err(ProviderAccountOperationError::ProviderUnavailable);
            }
        };
        if let Some(catalog) = catalog {
            published = match persist_model_catalog_refresh(
                self.inner.catalogs.as_ref(),
                &published,
                catalog,
            )
            .await
            {
                Ok(account) => account,
                Err(_) => {
                    let credential_restored = token_store.restore(&snapshot).is_ok();
                    let account_restored = account_exists
                        || self
                            .inner
                            .accounts
                            .delete_provider_account(&published.provider_account_id)
                            .await
                            .is_ok_and(|deleted| deleted);
                    if !credential_restored || !account_restored {
                        return Err(ProviderAccountOperationError::CompensationFailed);
                    }
                    return Err(ProviderAccountOperationError::ProviderUnavailable);
                }
            };
        }
        if let Err(error) = self
            .initialize_model_account(&published, crate::DEFAULT_CODEX_MODEL)
            .await
        {
            let credential_restored = token_store.restore(&snapshot).is_ok();
            let account_restored = account_exists
                || self
                    .inner
                    .accounts
                    .delete_provider_account(&published.provider_account_id)
                    .await
                    .is_ok_and(|deleted| deleted);
            if !credential_restored || !account_restored {
                self.log_compensation_failure(
                    "complete_provider_auth",
                    &current.provider_account_id,
                );
                return Err(ProviderAccountOperationError::CompensationFailed);
            }
            return Err(error);
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
        let current = match self
            .require_active_account(&expected_account.provider_account_id)
            .await
        {
            Ok(current) => current,
            Err(ProviderAccountOperationError::AccountNotFound) => return Ok(()),
            Err(error) => return Err(error),
        };
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
                auth_method: None,
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
        let account = provider_account_from_persisted(account);
        if account.provider_kind == "foundation_local" {
            return self.reconcile_foundation_account(account).await;
        }
        let gate = self.inner.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let account = self.require_active_account(provider_account_id).await?;
        let credential_present = match account.auth_method {
            ProviderAuthMethod::SecretInput | ProviderAuthMethod::OauthPkce => {
                self.secret_store(&account).load_api_key().is_ok()
            }
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
                auth_method: None,
                status: Some(ProviderAccountStatusUpdate {
                    status: desired,
                    error_code: None,
                    error_message: None,
                }),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)
            .map(provider_account_from_persisted)
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
                auth_method: None,
                status: Some(ProviderAccountStatusUpdate {
                    status: ProviderAccountStatus::Unauthenticated,
                    error_code: Some("auth_failed".to_string()),
                    error_message: Some("Provider rejected the configured credentials".to_string()),
                }),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)
            .map(provider_account_from_persisted)
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
                auth_method: None,
                status: Some(status),
                metadata: None,
            })
            .await
            .map_err(map_persistence_error)
            .map(provider_account_from_persisted)
    }

    pub(super) async fn refresh_model_catalog_impl(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
        let account = self.require_active_account(provider_account_id).await?;
        let expected_revision = credential_revision(&account);
        let catalog = match fetch_provider_model_catalog(&self.inner.credentials, &account).await {
            Ok(Some(catalog)) => catalog,
            Ok(None) => return Ok(account),
            Err(crate::ProviderError::AuthenticationFailure { .. })
                if account.provider_kind == "openrouter" =>
            {
                return self
                    .record_auth_failure_impl(provider_account_id, expected_revision)
                    .await;
            }
            Err(_) => return Err(ProviderAccountOperationError::ProviderUnavailable),
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

fn pending_codex_account() -> ProviderAccountRecord {
    ProviderAccountRecord {
        provider_account_id: "provider_account:codex:default".to_string(),
        provider_kind: CODEX_PROVIDER.to_string(),
        account_key: "default".to_string(),
        display_name: "Codex".to_string(),
        auth_method: ProviderAuthMethod::OauthDeviceCode,
        is_active: true,
        is_default: true,
        status: ProviderAccountStatus::Unauthenticated,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata: serde_json::json!({"credentialRevision": 0}),
        capabilities: crate::capabilities_for_provider_account(
            CODEX_PROVIDER,
            "default",
            ProviderAccountStatus::Unauthenticated,
        ),
    }
}

fn pending_openrouter_account(
    auth_method: ProviderAuthMethod,
    profiles: &[ProviderModelProfile],
) -> Result<ProviderAccountRecord, ProviderAccountOperationError> {
    let mut metadata = serde_json::json!({
        "credentialRevision": 1,
        "secretConfigured": true,
    });
    ProviderModelProfile::write_account_metadata(&mut metadata, profiles)
        .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
    Ok(ProviderAccountRecord {
        provider_account_id: "provider_account:openrouter:default".to_string(),
        provider_kind: "openrouter".to_string(),
        account_key: "default".to_string(),
        display_name: "OpenRouter".to_string(),
        auth_method,
        is_active: true,
        is_default: true,
        status: ProviderAccountStatus::Authenticated,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata,
        capabilities: crate::capabilities_for_provider_account(
            "openrouter",
            "default",
            ProviderAccountStatus::Authenticated,
        ),
    })
}

fn random_attempt_id() -> Result<String, ProviderAccountOperationError> {
    let mut bytes = [0_u8; 24];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| ProviderAccountOperationError::ProviderUnavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
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
