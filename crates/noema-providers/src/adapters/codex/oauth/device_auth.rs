use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::{sync::oneshot, time};

use crate::adapters::auth::{
    DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT, ProviderAuthAttemptRuntime, ProviderAuthManager,
    ensure_provider_account_home, is_terminal_status,
};
use crate::{
    CODEX_PROVIDER, CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
    ProviderAuthMethod, ProviderError,
};

use super::{
    client::{CodexOAuthClient, DeviceCodeResponse},
    token_store::CodexTokenStore,
};

const LOGIN_INSTRUCTIONS: &str = "Complete the login in your browser.";
const AUTH_EXPIRED_CODE: &str = "provider_auth_expired";
const AUTH_EXPIRED_MESSAGE: &str = "provider auth expired";

static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);

/// Start a Codex device-code auth attempt.
///
/// # Errors
///
/// Returns [`ProviderError`] when the account home cannot be prepared or the
/// initial device-code request fails.
pub(crate) async fn start_codex_device_auth(
    manager: ProviderAuthManager,
    request: CodexDeviceAuthRequest,
) -> Result<ProviderAuthAttemptView, ProviderError> {
    let attempt_timeout = request
        .attempt_timeout
        .unwrap_or(DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT);
    ensure_provider_account_home(&request.account_home).map_err(|source| {
        ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: format!("failed to prepare Codex account home: {source}"),
        }
    })?;
    let store = CodexTokenStore::new(request.account_home);
    let oauth_client = CodexOAuthClient::new(request.oauth)?;
    let device_code = oauth_client.request_device_code().await?;

    if device_code.user_code.trim().is_empty() || device_code.device_auth_id.trim().is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "Codex device-code response missing user_code or device_auth_id".to_string(),
        });
    }

    let attempt = ProviderAuthAttemptView {
        attempt_id: next_attempt_id(),
        provider_kind: CODEX_PROVIDER.to_string(),
        provider_account_id: request.provider_account_id,
        method: ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::WaitingForUser,
        verification_url: Some(oauth_client.verification_url()),
        user_code: Some(device_code.user_code.clone()),
        instructions: Some(LOGIN_INSTRUCTIONS.to_string()),
        error_code: None,
        error_message: None,
    };
    manager.upsert_attempt(attempt.clone()).await;

    let attempt_id = attempt.attempt_id.clone();
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    manager
        .upsert_attempt_runtime(
            attempt_id.clone(),
            ProviderAuthAttemptRuntime::new(cancel_sender),
        )
        .await;

    let task_manager = manager.clone();
    tokio::spawn(async move {
        let outcome = run_device_auth_polling(
            oauth_client,
            store,
            device_code,
            attempt_timeout,
            cancel_receiver,
        )
        .await;

        match outcome {
            DeviceAuthOutcome::Completed => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Completed,
                    None,
                    None,
                )
                .await;
            }
            DeviceAuthOutcome::Cancelled => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Cancelled,
                    None,
                    None,
                )
                .await;
            }
            DeviceAuthOutcome::Expired => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Expired,
                    Some(AUTH_EXPIRED_CODE),
                    Some(AUTH_EXPIRED_MESSAGE),
                )
                .await;
            }
            DeviceAuthOutcome::Failed(message) => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Failed,
                    Some("codex_device_auth_failed"),
                    Some(&message),
                )
                .await;
            }
        }

        task_manager.remove_attempt_runtime(&attempt_id).await;
    });

    Ok(attempt)
}

async fn run_device_auth_polling(
    oauth_client: CodexOAuthClient,
    store: CodexTokenStore,
    device_code: DeviceCodeResponse,
    attempt_timeout: Duration,
    mut cancel_receiver: oneshot::Receiver<()>,
) -> DeviceAuthOutcome {
    let sleep_seconds = device_code.interval.unwrap_or(5).max(3);
    let deadline = time::sleep(attempt_timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut cancel_receiver => return DeviceAuthOutcome::Cancelled,
            () = &mut deadline => return DeviceAuthOutcome::Expired,
            () = time::sleep(Duration::from_secs(sleep_seconds)) => {}
        }

        let authorization = match oauth_client
            .poll_device_authorization(&device_code.device_auth_id, &device_code.user_code)
            .await
        {
            Ok(Some(authorization)) => authorization,
            Ok(None) => continue,
            Err(error) => return DeviceAuthOutcome::Failed(safe_auth_failure_message(error)),
        };

        if authorization.authorization_code.trim().is_empty()
            || authorization.code_verifier.trim().is_empty()
        {
            return DeviceAuthOutcome::Failed(
                "Codex authorization response was incomplete".to_string(),
            );
        }

        let tokens = match oauth_client
            .exchange_authorization_code(
                &authorization.authorization_code,
                &authorization.code_verifier,
            )
            .await
        {
            Ok(tokens) => tokens,
            Err(error) => return DeviceAuthOutcome::Failed(safe_auth_failure_message(error)),
        };

        return match store.write(&tokens) {
            Ok(()) => DeviceAuthOutcome::Completed,
            Err(error) => DeviceAuthOutcome::Failed(safe_auth_failure_message(error)),
        };
    }
}

enum DeviceAuthOutcome {
    Completed,
    Cancelled,
    Expired,
    Failed(String),
}

async fn mark_terminal(
    manager: &ProviderAuthManager,
    attempt_id: &str,
    status: ProviderAuthAttemptStatus,
    error_code: Option<&str>,
    error_message: Option<&str>,
) {
    manager
        .update_attempt(attempt_id, |view| {
            if !is_terminal_status(view.status) {
                view.status = status;
                view.error_code = error_code.map(str::to_string);
                view.error_message = error_message.map(str::to_string);
            }
        })
        .await;
}

fn next_attempt_id() -> String {
    format!(
        "provider_auth_attempt:{}",
        NEXT_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn safe_auth_failure_message(error: ProviderError) -> String {
    match error {
        ProviderError::RateLimit { message, .. }
        | ProviderError::AuthenticationFailure { message, .. }
        | ProviderError::ApiError { message, .. }
        | ProviderError::MalformedResponse { message }
        | ProviderError::InvalidRequest { message }
        | ProviderError::ProviderUnavailable { message, .. } => message,
        ProviderError::MissingCredentials { credential, .. } => {
            format!("Codex auth is missing {credential}")
        }
        ProviderError::TransportFailure { .. } => "Codex auth network request failed".to_string(),
        ProviderError::Timeout { operation, .. } => {
            format!("Codex auth timed out during {operation}")
        }
        ProviderError::ProtocolError { message, .. }
        | ProviderError::PartialResponse { message, .. } => message,
    }
}
