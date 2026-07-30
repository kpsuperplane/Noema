use crate::ProviderError;
use reqwest::{StatusCode, header::HeaderMap};
use serde_json::Value;

/// Normalize and validate an HTTP API base URL.
pub fn normalize_base_url(value: String, label: &str) -> Result<String, ProviderError> {
    let base_url = value.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} cannot be empty"),
        });
    }
    let parsed = reqwest::Url::parse(&base_url).map_err(|_| ProviderError::InvalidRequest {
        message: format!("{label} must be an absolute URL"),
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} must use HTTP or HTTPS"),
        });
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} cannot contain credentials, a query, or a fragment"),
        });
    }

    Ok(base_url)
}
pub(crate) fn error_from_status(
    status: StatusCode,
    request_id: Option<String>,
    body_text: &str,
) -> ProviderError {
    let message = serde_json::from_str::<Value>(body_text)
        .ok()
        .and_then(|body| body.pointer("/error/message")?.as_str().map(str::to_string))
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| body_text.trim().to_string());
    let message = if message.is_empty() {
        status.canonical_reason().unwrap_or("API error").to_string()
    } else {
        message
    };

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthenticationFailure {
            message,
            request_id,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimit {
            message,
            request_id,
        },
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id,
        },
    }
}
pub(crate) fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}
