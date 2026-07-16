//! Codex OAuth token storage and device-code login.

use std::{
    fmt, fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::{sync::oneshot, time};

use super::{reqwest_transport_error, responses::normalize_base_url};
use crate::provider::auth::{
    DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT, ProviderAuthAttemptRuntime, ProviderAuthManager,
    ensure_provider_account_home, is_terminal_status,
};
use noema_providers::{
    CODEX_PROVIDER, CodexDeviceAuthRequest, CodexOAuthConfig, CodexOAuthTokens,
    ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod, ProviderError,
};

const TOKEN_FILE_NAME: &str = "codex_tokens.json";
const LOGIN_INSTRUCTIONS: &str = "Complete the login in your browser.";
const AUTH_EXPIRED_CODE: &str = "provider_auth_expired";
const AUTH_EXPIRED_MESSAGE: &str = "provider auth expired";

static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);

/// Filesystem store for Noema-owned Codex OAuth tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexTokenStore {
    account_home: PathBuf,
}

impl fmt::Debug for CodexTokenStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexTokenStore")
            .field("account_home", &"[REDACTED]")
            .finish()
    }
}

impl CodexTokenStore {
    /// Build a token store under the provider account home.
    #[must_use]
    pub fn new(account_home: impl Into<PathBuf>) -> Self {
        Self {
            account_home: account_home.into(),
        }
    }

    /// Return the token file path.
    #[must_use]
    pub fn token_path(&self) -> PathBuf {
        self.account_home.join(TOKEN_FILE_NAME)
    }

    /// Return true when a Noema-owned token file exists and has required fields.
    #[must_use]
    pub fn has_usable_tokens(&self) -> bool {
        self.read().is_ok_and(|tokens| tokens.has_required_fields())
    }

    /// Read Codex OAuth tokens.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MissingCredentials`] when tokens are absent or
    /// incomplete, and [`ProviderError::MalformedResponse`] for invalid JSON.
    pub fn read(&self) -> Result<CodexOAuthTokens, ProviderError> {
        let token_path = self.token_path();
        let text = fs::read_to_string(&token_path).map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => ProviderError::MissingCredentials {
                provider: CODEX_PROVIDER.to_string(),
                credential: TOKEN_FILE_NAME.to_string(),
            },
            _ => ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to read Codex token file: {source}"),
            },
        })?;
        let tokens: CodexOAuthTokens =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse Codex token file: {source}"),
            })?;
        if !tokens.has_required_fields() {
            return Err(ProviderError::MissingCredentials {
                provider: CODEX_PROVIDER.to_string(),
                credential: "access_token and refresh_token".to_string(),
            });
        }
        Ok(tokens)
    }

    /// Write Codex OAuth tokens atomically enough for a single local daemon.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::ProviderUnavailable`] if the account home or
    /// token file cannot be written.
    pub fn write(&self, tokens: &CodexOAuthTokens) -> Result<(), ProviderError> {
        ensure_provider_account_home(&self.account_home).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to prepare Codex token directory: {source}"),
            }
        })?;
        let text = serde_json::to_string_pretty(tokens).map_err(|source| {
            ProviderError::MalformedResponse {
                message: format!("failed to serialize Codex tokens: {source}"),
            }
        })?;
        fs::write(self.token_path(), text).map_err(|source| ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: format!("failed to write Codex token file: {source}"),
        })
    }

    /// Resolve a usable access token, refreshing when expiry is near.
    ///
    /// # Errors
    ///
    /// Returns provider/auth errors when credentials are missing or refresh
    /// fails.
    pub async fn access_token(
        &self,
        client: &CodexOAuthClient,
        refresh_skew_seconds: u64,
    ) -> Result<String, ProviderError> {
        let tokens = self.read()?;
        if !token_needs_refresh(&tokens.access_token, refresh_skew_seconds) {
            return Ok(tokens.access_token);
        }
        let refreshed = client.refresh_tokens(&tokens.refresh_token).await?;
        self.write(&refreshed)?;
        Ok(refreshed.access_token)
    }

    /// Force-refresh tokens and persist the result.
    ///
    /// # Errors
    ///
    /// Returns provider/auth errors when credentials are missing or refresh
    /// fails.
    pub async fn refresh_access_token(
        &self,
        client: &CodexOAuthClient,
    ) -> Result<String, ProviderError> {
        let tokens = self.read()?;
        let refreshed = client.refresh_tokens(&tokens.refresh_token).await?;
        self.write(&refreshed)?;
        Ok(refreshed.access_token)
    }
}

/// Codex OAuth HTTP client.
#[derive(Clone)]
pub struct CodexOAuthClient {
    client: reqwest::Client,
    config: CodexOAuthConfig,
}

impl fmt::Debug for CodexOAuthClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexOAuthClient")
            .field("client", &"[CONFIGURED]")
            .field("config", &self.config)
            .finish()
    }
}

impl CodexOAuthClient {
    /// Build a Codex OAuth client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] for invalid endpoint config or
    /// [`ProviderError::TransportFailure`] when the HTTP client cannot be built.
    pub fn new(mut config: CodexOAuthConfig) -> Result<Self, ProviderError> {
        config.issuer = normalize_base_url(config.issuer, "codex OAuth issuer")?;
        config.token_url = normalize_base_url(config.token_url, "codex OAuth token URL")?;
        config.client_id = config.client_id.trim().to_string();
        if config.client_id.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex OAuth client id cannot be empty".to_string(),
            });
        }
        if config.timeout_seconds == 0 {
            return Err(ProviderError::InvalidRequest {
                message: "codex OAuth timeout must be greater than zero seconds".to_string(),
            });
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| {
                reqwest_transport_error(CODEX_PROVIDER, "build_oauth_client", &source)
            })?;
        Ok(Self { client, config })
    }

    /// Return the browser verification URL for Codex device auth.
    #[must_use]
    pub fn verification_url(&self) -> String {
        format!("{}/codex/device", self.config.issuer)
    }

    async fn request_device_code(&self) -> Result<DeviceCodeResponse, ProviderError> {
        let response = self
            .client
            .post(format!(
                "{}/api/accounts/deviceauth/usercode",
                self.config.issuer
            ))
            .json(&serde_json::json!({ "client_id": self.config.client_id }))
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(CODEX_PROVIDER, "request_device_code", &source)
            })?;
        let status = response.status();
        let text = response.text().await.map_err(|source| {
            reqwest_transport_error(CODEX_PROVIDER, "read_device_code_response", &source)
        })?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimit {
                message: "OpenAI is rate-limiting Codex login requests".to_string(),
                request_id: None,
            });
        }
        if !status.is_success() {
            return Err(ProviderError::ApiError {
                status: status.as_u16(),
                message: text,
                request_id: None,
            });
        }
        serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
            message: format!("failed to parse Codex device-code response: {source}"),
        })
    }

    async fn poll_device_authorization(
        &self,
        device_auth_id: &str,
        user_code: &str,
    ) -> Result<Option<DeviceAuthorizationResponse>, ProviderError> {
        let response = self
            .client
            .post(format!(
                "{}/api/accounts/deviceauth/token",
                self.config.issuer
            ))
            .json(&serde_json::json!({
                "device_auth_id": device_auth_id,
                "user_code": user_code,
            }))
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(CODEX_PROVIDER, "poll_device_authorization", &source)
            })?;
        let status = response.status();
        let text = response.text().await.map_err(|source| {
            reqwest_transport_error(
                CODEX_PROVIDER,
                "read_device_authorization_response",
                &source,
            )
        })?;
        if status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(ProviderError::ApiError {
                status: status.as_u16(),
                message: text,
                request_id: None,
            });
        }
        let authorization =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse Codex authorization response: {source}"),
            })?;
        Ok(Some(authorization))
    }

    async fn exchange_authorization_code(
        &self,
        authorization_code: &str,
        code_verifier: &str,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        let redirect_uri = format!("{}/deviceauth/callback", self.config.issuer);
        let response = self
            .client
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", authorization_code),
                ("redirect_uri", redirect_uri.as_str()),
                ("client_id", self.config.client_id.as_str()),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(CODEX_PROVIDER, "exchange_authorization_code", &source)
            })?;
        self.tokens_from_response(response, None).await
    }

    /// Refresh Codex OAuth tokens.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::AuthenticationFailure`] when re-login is
    /// required, [`ProviderError::RateLimit`] for 429, and provider errors for
    /// malformed token responses.
    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        if refresh_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: CODEX_PROVIDER.to_string(),
                credential: "refresh_token".to_string(),
            });
        }
        let response = self
            .client
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", self.config.client_id.as_str()),
            ])
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(CODEX_PROVIDER, "refresh_oauth_tokens", &source)
            })?;
        self.tokens_from_response(response, Some(refresh_token))
            .await
    }

    async fn tokens_from_response(
        &self,
        response: reqwest::Response,
        fallback_refresh_token: Option<&str>,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        let status = response.status();
        let text = response.text().await.map_err(|source| {
            reqwest_transport_error(CODEX_PROVIDER, "read_oauth_token_response", &source)
        })?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimit {
                message: "Codex OAuth token endpoint is rate-limited".to_string(),
                request_id: None,
            });
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthenticationFailure {
                message: oauth_error_message(&text)
                    .unwrap_or_else(|| "Codex OAuth credentials were rejected".to_string()),
                request_id: None,
            });
        }
        if !status.is_success() {
            let message = oauth_error_message(&text).unwrap_or(text);
            if oauth_error_requires_relogin(&message) {
                return Err(ProviderError::AuthenticationFailure {
                    message,
                    request_id: None,
                });
            }
            return Err(ProviderError::ApiError {
                status: status.as_u16(),
                message,
                request_id: None,
            });
        }
        let payload: TokenResponse =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse Codex token response: {source}"),
            })?;
        let access_token = payload.access_token.trim().to_string();
        if access_token.is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "Codex token response was missing access_token".to_string(),
            });
        }
        let refresh_token = payload
            .refresh_token
            .as_deref()
            .or(fallback_refresh_token)
            .unwrap_or_default()
            .trim()
            .to_string();
        if refresh_token.is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "Codex token response was missing refresh_token".to_string(),
            });
        }
        Ok(CodexOAuthTokens {
            access_token,
            refresh_token,
            last_refresh: now_unix_seconds(),
        })
    }
}

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

fn token_needs_refresh(access_token: &str, refresh_skew_seconds: u64) -> bool {
    let token = access_token.trim();
    if token.is_empty() {
        return true;
    }
    let Some(claims_segment) = token.split('.').nth(1) else {
        return false;
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(claims_segment) else {
        return false;
    };
    let Ok(claims) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    let Some(exp) = claims.get("exp").and_then(serde_json::Value::as_u64) else {
        return false;
    };
    exp <= now_unix_seconds().saturating_add(refresh_skew_seconds)
}

/// Extract the selected ChatGPT workspace from a Codex OAuth access token.
pub(crate) fn chatgpt_account_id_from_access_token(token: &str) -> Option<String> {
    let claims_segment = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(claims_segment).ok()?;
    let claims = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .or_else(|| claims.get("chatgpt_account_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToString::to_string)
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn oauth_error_message(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let error = value.get("error")?;
    if let Some(object) = error.as_object() {
        return object
            .get("message")
            .and_then(serde_json::Value::as_str)
            .or_else(|| object.get("code").and_then(serde_json::Value::as_str))
            .map(ToString::to_string);
    }
    error.as_str().map(ToString::to_string).or_else(|| {
        value
            .get("error_description")?
            .as_str()
            .map(ToString::to_string)
    })
}

fn oauth_error_requires_relogin(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    ["invalid_grant", "invalid_token", "refresh_token_reused"]
        .iter()
        .any(|needle| normalized.contains(needle))
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

#[derive(Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_auth_id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u64_from_string_or_number"
    )]
    interval: Option<u64>,
}

impl fmt::Debug for DeviceCodeResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceCodeResponse")
            .field("user_code", &"[REDACTED]")
            .field("device_auth_id", &"[REDACTED]")
            .field("interval", &self.interval)
            .finish()
    }
}

fn deserialize_optional_u64_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(number) => number
            .as_u64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("expected unsigned integer")),
        serde_json::Value::String(value) => value
            .parse::<u64>()
            .map(Some)
            .map_err(|source| serde::de::Error::custom(source.to_string())),
        _ => Err(serde::de::Error::custom(
            "expected unsigned integer or string",
        )),
    }
}

#[derive(Deserialize)]
struct DeviceAuthorizationResponse {
    authorization_code: String,
    code_verifier: String,
}

impl fmt::Debug for DeviceAuthorizationResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceAuthorizationResponse")
            .field("authorization_code", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .finish()
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenResponse")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_providers::CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS;
    use tempfile::TempDir;

    #[test]
    fn oauth_debug_redacts_credentials_and_account_paths() {
        let token_store = CodexTokenStore::new("/private/codex-account-secret");
        let oauth_client = CodexOAuthClient::new(CodexOAuthConfig {
            client_id: "oauth-client-secret".to_string(),
            ..CodexOAuthConfig::default()
        })
        .expect("oauth client");
        let device = DeviceCodeResponse {
            user_code: "user-code-secret".to_string(),
            device_auth_id: "device-auth-secret".to_string(),
            interval: Some(5),
        };
        let authorization = DeviceAuthorizationResponse {
            authorization_code: "authorization-secret".to_string(),
            code_verifier: "verifier-secret".to_string(),
        };
        let tokens = TokenResponse {
            access_token: "access-secret".to_string(),
            refresh_token: Some("refresh-secret".to_string()),
        };
        let debug =
            format!("{token_store:?} {oauth_client:?} {device:?} {authorization:?} {tokens:?}");

        for secret in [
            "codex-account-secret",
            "oauth-client-secret",
            "user-code-secret",
            "device-auth-secret",
            "authorization-secret",
            "verifier-secret",
            "access-secret",
            "refresh-secret",
        ] {
            assert!(!debug.contains(secret));
        }
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn token_store_does_not_treat_cli_auth_json_as_usable() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        fs::create_dir_all(&account_home).expect("account dir");
        fs::write(
            account_home.join("auth.json"),
            r#"{"tokens":{"access_token":"local","refresh_token":"local-refresh"}}"#,
        )
        .expect("local auth");

        let store = CodexTokenStore::new(account_home);

        assert!(!store.has_usable_tokens());
    }

    #[test]
    fn token_store_round_trips_noema_tokens() {
        let dir = TempDir::new().expect("temp dir");
        let store = CodexTokenStore::new(dir.path().join("providers/codex/default"));
        let tokens = CodexOAuthTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            last_refresh: 123,
        };

        store.write(&tokens).expect("write");

        assert_eq!(store.read().expect("read"), tokens);
    }

    #[test]
    fn device_code_response_accepts_string_interval() {
        let response: DeviceCodeResponse = serde_json::from_str(
            r#"{
                "device_auth_id": "deviceauth_test",
                "user_code": "ABCD-EFGH",
                "interval": "5",
                "expires_at": "2026-06-28T00:38:54.083987+00:00"
            }"#,
        )
        .expect("device code response");

        assert_eq!(response.interval, Some(5));
    }

    #[test]
    fn device_code_response_accepts_numeric_interval() {
        let response: DeviceCodeResponse = serde_json::from_str(
            r#"{
                "device_auth_id": "deviceauth_test",
                "user_code": "ABCD-EFGH",
                "interval": 5
            }"#,
        )
        .expect("device code response");

        assert_eq!(response.interval, Some(5));
    }

    #[test]
    fn token_refresh_check_uses_jwt_exp_when_present() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let claims = URL_SAFE_NO_PAD
            .encode(format!(r#"{{"exp":{}}}"#, now_unix_seconds().saturating_sub(1)).as_bytes());
        let token = format!("{header}.{claims}.sig");

        assert!(token_needs_refresh(
            &token,
            CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS
        ));
    }

    #[test]
    fn extracts_chatgpt_workspace_from_access_token() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let claims = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}"#);
        let token = format!("{header}.{claims}.sig");

        assert_eq!(
            chatgpt_account_id_from_access_token(&token).as_deref(),
            Some("workspace-test")
        );
    }

    #[test]
    fn opaque_non_empty_token_is_not_preemptively_refreshed() {
        assert!(!token_needs_refresh(
            "opaque-token",
            CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS
        ));
    }
}
