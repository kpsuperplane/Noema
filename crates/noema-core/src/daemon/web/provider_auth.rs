use std::{future::Future, pin::Pin, time::Duration};

use crate::{
    NoemaStore,
    provider_auth::{
        CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
        ProviderAuthManager,
    },
    providers::codex_oauth::{CodexOAuthConfig, CodexTokenStore},
};

use super::{DaemonError, WebState, http::HttpRequestError};

const PROVIDER_AUTH_TERMINAL_PERSIST_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(crate) struct WebApiError {
    message: String,
}

impl WebApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl From<HttpRequestError> for WebApiError {
    fn from(error: HttpRequestError) -> Self {
        Self {
            message: error.message().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderAuthStartRequest {
    pub(crate) provider_kind: String,
    pub(crate) provider_account_id: String,
    pub(crate) method: crate::ProviderAuthMethod,
}

pub(crate) async fn start_provider_auth_attempt_view(
    state: &WebState,
    body: ProviderAuthStartRequest,
) -> Result<ProviderAuthAttemptView, WebApiError> {
    if body.provider_kind != "codex" {
        return Err(WebApiError::bad_request("unsupported provider"));
    }

    if body.method != crate::ProviderAuthMethod::OauthDeviceCode {
        return Err(WebApiError::bad_request("unsupported provider auth method"));
    }

    let Some(account) = state
        .store
        .get_provider_account(&body.provider_account_id)
        .await
        .map_err(|_| WebApiError::internal("provider auth status unavailable"))?
    else {
        return Err(WebApiError::not_found("provider account not found"));
    };

    if let Err(error) = validate_provider_auth_account(&account, &body.provider_kind, body.method) {
        return Err(error.into());
    }

    match start_codex_provider_auth_attempt(
        &state.provider_auth,
        &state.store,
        &state.paths,
        &account,
    )
    .await
    {
        Ok(attempt) => {
            if !should_persist_provider_auth_attempt_status(&attempt) {
                spawn_provider_auth_terminal_persistence(
                    state.provider_auth.clone(),
                    state.store.clone(),
                    attempt.attempt_id.clone(),
                );
            }
            Ok(attempt)
        }
        Err(StartProviderAuthAttemptError::ProviderUnavailable(message)) => {
            Err(WebApiError::internal(message))
        }
        Err(StartProviderAuthAttemptError::StatusUnavailable) => {
            Err(WebApiError::internal("provider auth status unavailable"))
        }
    }
}

pub(super) fn validate_provider_auth_account(
    account: &crate::ProviderAccountRecord,
    provider_kind: &str,
    method: crate::ProviderAuthMethod,
) -> Result<(), HttpRequestError> {
    if account.provider_kind != provider_kind {
        return Err(HttpRequestError::bad_request("provider account mismatch"));
    }
    if !account.is_active {
        return Err(HttpRequestError::bad_request("provider account not found"));
    }
    if account.auth_method != method {
        return Err(HttpRequestError::bad_request(
            "provider account auth method mismatch",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderAccountStatusUpdate {
    pub(super) status: crate::ProviderAccountStatus,
    pub(super) error_code: Option<String>,
    pub(super) error_message: Option<String>,
}

pub(crate) trait ProviderAccountStatusStore {
    fn update_provider_account_status<'a>(
        &'a self,
        provider_account_id: &'a str,
        status: crate::ProviderAccountStatus,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>>;
}

impl ProviderAccountStatusStore for NoemaStore {
    fn update_provider_account_status<'a>(
        &'a self,
        provider_account_id: &'a str,
        status: crate::ProviderAccountStatus,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>> {
        Box::pin(async move {
            self.update_provider_account_status(
                provider_account_id,
                status,
                error_code,
                error_message,
            )
            .await?;
            Ok(())
        })
    }
}

pub(super) trait ProviderAuthAttemptPoller {
    fn poll_provider_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>> + Send + 'a>,
    >;
}

impl ProviderAuthAttemptPoller for ProviderAuthManager {
    fn poll_provider_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.poll_attempt(attempt_id)
                .await
                .map_err(|source| DaemonError::Protocol(source.to_string()))
        })
    }
}

pub(super) trait CodexDeviceAuthStarter {
    fn start_codex_device_code<'a>(
        &'a self,
        request: CodexDeviceAuthRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>> + Send + 'a>,
    >;
}

impl CodexDeviceAuthStarter for ProviderAuthManager {
    fn start_codex_device_code<'a>(
        &'a self,
        request: CodexDeviceAuthRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>> + Send + 'a>,
    > {
        Box::pin(async move { self.start_codex_device_code(request).await })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StartProviderAuthAttemptError {
    ProviderUnavailable(String),
    StatusUnavailable,
}

pub(super) async fn start_codex_provider_auth_attempt(
    starter: &impl CodexDeviceAuthStarter,
    status_store: &impl ProviderAccountStatusStore,
    paths: &crate::NoemaPaths,
    account: &crate::ProviderAccountRecord,
) -> Result<ProviderAuthAttemptView, StartProviderAuthAttemptError> {
    let attempt = starter
        .start_codex_device_code(CodexDeviceAuthRequest {
            provider_account_id: account.provider_account_id.clone(),
            account_home: paths.provider_account_home(&account.provider_kind, &account.account_key),
            oauth: CodexOAuthConfig::default(),
            attempt_timeout: None,
        })
        .await
        .map_err(|error| StartProviderAuthAttemptError::ProviderUnavailable(error.to_string()))?;

    if should_persist_provider_auth_attempt_status(&attempt) {
        persist_provider_account_status_from_attempt(status_store, &attempt)
            .await
            .map_err(|_| StartProviderAuthAttemptError::StatusUnavailable)?;
    }

    Ok(attempt)
}

pub(super) fn provider_account_status_update_from_attempt(
    attempt: &ProviderAuthAttemptView,
) -> Option<ProviderAccountStatusUpdate> {
    match attempt.status {
        ProviderAuthAttemptStatus::Completed => Some(ProviderAccountStatusUpdate {
            status: crate::ProviderAccountStatus::Authenticated,
            error_code: None,
            error_message: None,
        }),
        ProviderAuthAttemptStatus::Failed
        | ProviderAuthAttemptStatus::Expired
        | ProviderAuthAttemptStatus::Cancelled => Some(ProviderAccountStatusUpdate {
            status: crate::ProviderAccountStatus::Unauthenticated,
            error_code: attempt.error_code.clone(),
            error_message: attempt.error_message.clone(),
        }),
        ProviderAuthAttemptStatus::Starting | ProviderAuthAttemptStatus::WaitingForUser => None,
    }
}

fn should_persist_provider_auth_attempt_status(attempt: &ProviderAuthAttemptView) -> bool {
    provider_account_status_update_from_attempt(attempt).is_some()
}

pub(crate) async fn persist_provider_account_status_from_attempt(
    store: &impl ProviderAccountStatusStore,
    attempt: &ProviderAuthAttemptView,
) -> Result<(), DaemonError> {
    let Some(update) = provider_account_status_update_from_attempt(attempt) else {
        return Ok(());
    };
    store
        .update_provider_account_status(
            &attempt.provider_account_id,
            update.status,
            update.error_code.as_deref(),
            update.error_message.as_deref(),
        )
        .await?;
    Ok(())
}

fn spawn_provider_auth_terminal_persistence(
    poller: ProviderAuthManager,
    status_store: NoemaStore,
    attempt_id: String,
) {
    tokio::spawn(async move {
        let _ = persist_provider_auth_attempt_terminal_status(
            &poller,
            &status_store,
            &attempt_id,
            PROVIDER_AUTH_TERMINAL_PERSIST_INTERVAL,
        )
        .await;
    });
}

pub(super) async fn persist_provider_auth_attempt_terminal_status(
    poller: &impl ProviderAuthAttemptPoller,
    status_store: &impl ProviderAccountStatusStore,
    attempt_id: &str,
    poll_interval: Duration,
) -> Result<(), DaemonError> {
    loop {
        let Some(attempt) = poller.poll_provider_auth_attempt(attempt_id).await? else {
            return Ok(());
        };
        if should_persist_provider_auth_attempt_status(&attempt) {
            persist_provider_account_status_from_attempt(status_store, &attempt).await?;
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }
}

pub(crate) async fn reconcile_onboarding_provider_account(
    status_store: &impl ProviderAccountStatusStore,
    paths: &crate::NoemaPaths,
    account: Option<crate::ProviderAccountRecord>,
) -> Result<Option<crate::ProviderAccountRecord>, DaemonError> {
    let Some(mut account) = account else {
        return Ok(None);
    };
    if account.status == crate::ProviderAccountStatus::Authenticated
        || !codex_account_home_has_noema_tokens(paths, &account)
    {
        return Ok(Some(account));
    }

    status_store
        .update_provider_account_status(
            &account.provider_account_id,
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    account.status = crate::ProviderAccountStatus::Authenticated;
    account.last_error_code = None;
    account.last_error_message = None;
    Ok(Some(account))
}

fn codex_account_home_has_noema_tokens(
    paths: &crate::NoemaPaths,
    account: &crate::ProviderAccountRecord,
) -> bool {
    if account.provider_kind != "codex" {
        return false;
    }
    let account_home = paths.provider_account_home(&account.provider_kind, &account.account_key);
    CodexTokenStore::new(account_home).has_usable_tokens()
}

pub(crate) fn is_user_onboarded_for_chat(account: Option<crate::ProviderAccountRecord>) -> bool {
    crate::onboarding_status_from_account(account).is_user_onboarded
}
