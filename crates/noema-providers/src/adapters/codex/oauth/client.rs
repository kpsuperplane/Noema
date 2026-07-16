use std::{fmt, time::Duration};

use reqwest::StatusCode;
use serde::Deserialize;

use crate::adapters::{reqwest_transport_error, responses::normalize_base_url};
use crate::{CODEX_PROVIDER, CodexOAuthConfig, CodexOAuthTokens, ProviderError};

use super::claims::now_unix_seconds;

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

    pub(super) async fn request_device_code(&self) -> Result<DeviceCodeResponse, ProviderError> {
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

    pub(super) async fn poll_device_authorization(
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

    pub(super) async fn exchange_authorization_code(
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

#[derive(Deserialize)]
pub(super) struct DeviceCodeResponse {
    pub(super) user_code: String,
    pub(super) device_auth_id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u64_from_string_or_number"
    )]
    pub(super) interval: Option<u64>,
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
pub(super) struct DeviceAuthorizationResponse {
    pub(super) authorization_code: String,
    pub(super) code_verifier: String,
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
pub(super) struct TokenResponse {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
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
