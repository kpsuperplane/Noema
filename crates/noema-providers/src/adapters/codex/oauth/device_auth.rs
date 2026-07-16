use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::{sync::oneshot, time};

use crate::adapters::auth::{
    DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT, ProviderAuthAttemptRuntime, ProviderAuthManager,
};
use crate::{
    CODEX_PROVIDER, CodexDeviceAuthRequest, CodexOAuthTokens, ProviderAuthAttemptStatus,
    ProviderAuthAttemptView, ProviderAuthMethod, ProviderError,
};

use super::client::{CodexOAuthClient, DeviceCodeResponse};

const LOGIN_INSTRUCTIONS: &str = "Complete the login in your browser.";

static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) type CodexDeviceAuthCompletion =
    Pin<Box<dyn Future<Output = CodexDeviceAuthOutcome> + Send + 'static>>;

pub(crate) struct CodexDeviceAuthSession {
    pub(crate) attempt: ProviderAuthAttemptView,
    pub(crate) completion: CodexDeviceAuthCompletion,
}

#[derive(Debug)]
pub(crate) enum CodexDeviceAuthOutcome {
    Completed(CodexOAuthTokens),
    Cancelled,
    Expired,
    Failed {
        error_code: String,
        error_message: String,
    },
}

/// Begin a Codex device-code auth attempt without publishing credentials.
///
/// # Errors
///
/// Returns [`ProviderError`] when OAuth client setup or the initial device-code
/// request fails.
pub(crate) async fn begin_codex_device_auth(
    manager: ProviderAuthManager,
    request: CodexDeviceAuthRequest,
) -> Result<CodexDeviceAuthSession, ProviderError> {
    let CodexDeviceAuthRequest {
        provider_account_id,
        account_home: _,
        oauth,
        attempt_timeout,
    } = request;
    let attempt_timeout = attempt_timeout.unwrap_or(DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT);
    let oauth_client = CodexOAuthClient::new(oauth)?;
    let device_code = oauth_client.request_device_code().await?;

    if device_code.user_code.trim().is_empty() || device_code.device_auth_id.trim().is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "Codex device-code response missing user_code or device_auth_id".to_string(),
        });
    }

    let attempt = ProviderAuthAttemptView {
        attempt_id: next_attempt_id(),
        provider_kind: CODEX_PROVIDER.to_string(),
        provider_account_id,
        method: ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::WaitingForUser,
        verification_url: Some(oauth_client.verification_url()),
        user_code: Some(device_code.user_code.clone()),
        instructions: Some(LOGIN_INSTRUCTIONS.to_string()),
        error_code: None,
        error_message: None,
    };
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    if !manager
        .register_attempt(
            attempt.clone(),
            ProviderAuthAttemptRuntime::new(cancel_sender),
        )
        .await
    {
        return Err(ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: "provider authentication service is shutting down".to_string(),
        });
    }

    Ok(CodexDeviceAuthSession {
        attempt,
        completion: Box::pin(run_device_auth_polling(
            oauth_client,
            device_code,
            attempt_timeout,
            cancel_receiver,
        )),
    })
}

async fn run_device_auth_polling(
    oauth_client: CodexOAuthClient,
    device_code: DeviceCodeResponse,
    attempt_timeout: Duration,
    mut cancel_receiver: oneshot::Receiver<()>,
) -> CodexDeviceAuthOutcome {
    let sleep_seconds = device_code.interval.unwrap_or(5).max(3);
    let deadline = time::sleep(attempt_timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut cancel_receiver => return CodexDeviceAuthOutcome::Cancelled,
            () = &mut deadline => return CodexDeviceAuthOutcome::Expired,
            () = time::sleep(Duration::from_secs(sleep_seconds)) => {}
        }

        let authorization = match tokio::select! {
            _ = &mut cancel_receiver => return CodexDeviceAuthOutcome::Cancelled,
            () = &mut deadline => return CodexDeviceAuthOutcome::Expired,
            result = oauth_client.poll_device_authorization(
                &device_code.device_auth_id,
                &device_code.user_code,
            ) => result,
        } {
            Ok(Some(authorization)) => authorization,
            Ok(None) => continue,
            Err(error) => return failed_outcome(error),
        };

        if authorization.authorization_code.trim().is_empty()
            || authorization.code_verifier.trim().is_empty()
        {
            return CodexDeviceAuthOutcome::Failed {
                error_code: "codex_device_auth_failed".to_string(),
                error_message: "Codex authorization response was incomplete".to_string(),
            };
        }

        return match tokio::select! {
            _ = &mut cancel_receiver => return CodexDeviceAuthOutcome::Cancelled,
            () = &mut deadline => return CodexDeviceAuthOutcome::Expired,
            result = oauth_client.exchange_authorization_code(
                &authorization.authorization_code,
                &authorization.code_verifier,
            ) => result,
        } {
            Ok(tokens) => CodexDeviceAuthOutcome::Completed(tokens),
            Err(error) => failed_outcome(error),
        };
    }
}

fn failed_outcome(error: ProviderError) -> CodexDeviceAuthOutcome {
    CodexDeviceAuthOutcome::Failed {
        error_code: "codex_device_auth_failed".to_string(),
        error_message: safe_auth_failure_message(error),
    }
}

fn next_attempt_id() -> String {
    format!(
        "provider_auth_attempt:{}",
        NEXT_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn safe_auth_failure_message(error: ProviderError) -> String {
    match error {
        ProviderError::RateLimit { .. } => "Codex login is temporarily rate-limited".to_string(),
        ProviderError::AuthenticationFailure { .. } => {
            "Codex rejected the login request".to_string()
        }
        ProviderError::ApiError { .. } => "Codex login request failed".to_string(),
        ProviderError::MalformedResponse { .. } => {
            "Codex returned an invalid login response".to_string()
        }
        ProviderError::InvalidRequest { .. } => "Codex login configuration is invalid".to_string(),
        ProviderError::ProviderUnavailable { .. } => {
            "Codex login is currently unavailable".to_string()
        }
        ProviderError::MissingCredentials { credential, .. } => {
            format!("Codex auth is missing {credential}")
        }
        ProviderError::TransportFailure { .. } => "Codex auth network request failed".to_string(),
        ProviderError::Timeout { .. } => "Codex auth request timed out".to_string(),
        ProviderError::ProtocolError { .. } | ProviderError::PartialResponse { .. } => {
            "Codex login response could not be processed".to_string()
        }
    }
}
