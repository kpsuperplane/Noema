//! Codex OAuth token storage and device-code login.

use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::{sync::oneshot, time};

use crate::{
    ProviderAuthMethod, ProviderError,
    provider::auth::{
        CodexDeviceAuthRequest, DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT, ProviderAuthAttemptRuntime,
        ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthManager,
        ensure_provider_account_home, is_terminal_status,
    },
};

/// Codex provider id used in user-facing auth state.
pub const CODEX_PROVIDER: &str = "codex";
/// Default Codex Responses API base URL.
pub const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Default Codex OAuth issuer.
pub const DEFAULT_CODEX_OAUTH_ISSUER: &str = "https://auth.openai.com";
/// Default Codex OAuth client id used by Codex/Hermes device auth.
pub const DEFAULT_CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Default Codex token endpoint.
pub const DEFAULT_CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
/// Refresh access tokens when JWT expiry is within this many seconds.
pub const CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS: u64 = 120;
/// Default request timeout for Codex OAuth calls.
pub const DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS: u64 = 20;

const TOKEN_FILE_NAME: &str = "codex_tokens.json";
const LOGIN_INSTRUCTIONS: &str = "Complete the login in your browser.";
const AUTH_EXPIRED_CODE: &str = "provider_auth_expired";
const AUTH_EXPIRED_MESSAGE: &str = "provider auth expired";

static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);

/// Noema-owned Codex OAuth token file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexOAuthTokens {
    /// Access token used as Responses API bearer token.
    pub access_token: String,
    /// Refresh token used to rotate access tokens.
    pub refresh_token: String,
    /// Unix seconds when the token file was last written.
    pub last_refresh: u64,
}

impl CodexOAuthTokens {
    fn has_required_fields(&self) -> bool {
        !self.access_token.trim().is_empty() && !self.refresh_token.trim().is_empty()
    }
}

/// Filesystem store for Noema-owned Codex OAuth tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexTokenStore {
    account_home: PathBuf,
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

/// Codex OAuth HTTP endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexOAuthConfig {
    /// OAuth issuer base URL.
    pub issuer: String,
    /// OAuth client id.
    pub client_id: String,
    /// OAuth token URL.
    pub token_url: String,
    /// HTTP timeout in seconds.
    pub timeout_seconds: u64,
}

impl Default for CodexOAuthConfig {
    fn default() -> Self {
        Self {
            issuer: DEFAULT_CODEX_OAUTH_ISSUER.to_string(),
            client_id: DEFAULT_CODEX_OAUTH_CLIENT_ID.to_string(),
            token_url: DEFAULT_CODEX_OAUTH_TOKEN_URL.to_string(),
            timeout_seconds: DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS,
        }
    }
}

/// Codex OAuth HTTP client.
#[derive(Debug, Clone)]
pub struct CodexOAuthClient {
    client: reqwest::Client,
    config: CodexOAuthConfig,
}

impl CodexOAuthClient {
    /// Build a Codex OAuth client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] for invalid endpoint config or
    /// [`ProviderError::HttpFailure`] when the HTTP client cannot be built.
    pub fn new(mut config: CodexOAuthConfig) -> Result<Self, ProviderError> {
        config.issuer = normalize_url(config.issuer, "codex OAuth issuer")?;
        config.token_url = normalize_url(config.token_url, "codex OAuth token URL")?;
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
            .map_err(|source| ProviderError::HttpFailure { source })?;
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
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
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
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
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
            .map_err(|source| ProviderError::HttpFailure { source })?;
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
            .map_err(|source| ProviderError::HttpFailure { source })?;
        self.tokens_from_response(response, Some(refresh_token))
            .await
    }

    async fn tokens_from_response(
        &self,
        response: reqwest::Response,
        fallback_refresh_token: Option<&str>,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
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

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalize_url(value: String, label: &str) -> Result<String, ProviderError> {
    let value = value.trim().trim_end_matches('/').to_string();
    if value.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} cannot be empty"),
        });
    }
    if reqwest::Url::parse(&value).is_err() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} must be an absolute URL"),
        });
    }
    Ok(value)
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
        ProviderError::HttpFailure { .. } => "Codex auth network request failed".to_string(),
        ProviderError::Timeout { operation, .. } => {
            format!("Codex auth timed out during {operation}")
        }
        ProviderError::ProtocolError { message, .. }
        | ProviderError::PartialResponse { message, .. }
        | ProviderError::UnsupportedFeature { feature: message } => message,
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_auth_id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u64_from_string_or_number"
    )]
    interval: Option<u64>,
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

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationResponse {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn token_store_does_not_treat_cli_auth_json_as_usable() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        fs::create_dir_all(&account_home).expect("account dir");
        fs::write(
            account_home.join("auth.json"),
            r#"{"tokens":{"access_token":"cli","refresh_token":"cli-refresh"}}"#,
        )
        .expect("cli auth");

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
    fn opaque_non_empty_token_is_not_preemptively_refreshed() {
        assert!(!token_needs_refresh(
            "opaque-token",
            CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS
        ));
    }
}
