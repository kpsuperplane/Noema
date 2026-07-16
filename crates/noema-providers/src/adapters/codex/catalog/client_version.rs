use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::ProviderError;
use crate::adapters::reqwest_transport_error;

pub(super) const MODEL_CATALOG_TIMEOUT_SECONDS: u64 = 20;
pub(super) const CODEX_CLIENT_VERSION_ENDPOINT: &str =
    "https://registry.npmjs.org/@openai%2fcodex/latest";
pub(super) const MODEL_CATALOG_TTL_SECONDS: u64 = 6 * 60 * 60;
pub(crate) const FALLBACK_CODEX_MODELS_CLIENT_VERSION: &str = "0.144.0";

const CODEX_VERSION_USER_AGENT: &str = "Noema/0.1 (+https://github.com/kpsuperplane/Noema)";

#[derive(Debug)]
pub(super) struct ResolvedCodexClientVersion {
    pub(super) value: String,
    pub(super) refreshed_at_unix: Option<u64>,
}

pub(super) async fn resolve_codex_client_version(
    client: &reqwest::Client,
    metadata: &Value,
    version_endpoint: &str,
) -> ResolvedCodexClientVersion {
    let cached_version = metadata
        .get("models_client_version")
        .and_then(Value::as_str)
        .filter(|version| is_valid_codex_client_version(version))
        .map(ToString::to_string);
    if let Some(cached_version) = cached_version.as_ref()
        && timestamp_is_fresh(
            metadata.get("models_client_version_refreshed_at"),
            current_unix_timestamp(),
        )
    {
        return ResolvedCodexClientVersion {
            value: cached_version.clone(),
            refreshed_at_unix: None,
        };
    }

    match fetch_latest_codex_client_version(client, version_endpoint).await {
        Ok(value) => ResolvedCodexClientVersion {
            value,
            refreshed_at_unix: Some(current_unix_timestamp()),
        },
        Err(_) => ResolvedCodexClientVersion {
            value: cached_version
                .unwrap_or_else(|| FALLBACK_CODEX_MODELS_CLIENT_VERSION.to_string()),
            refreshed_at_unix: None,
        },
    }
}

pub(super) async fn fetch_latest_codex_client_version(
    client: &reqwest::Client,
    version_endpoint: &str,
) -> Result<String, ProviderError> {
    let response = client
        .get(version_endpoint)
        .timeout(Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECONDS))
        .header(reqwest::header::USER_AGENT, CODEX_VERSION_USER_AGENT)
        .send()
        .await
        .map_err(|source| reqwest_transport_error("codex", "fetch_client_version", &source))?;
    let status = response.status();
    let text = response.text().await.map_err(|source| {
        reqwest_transport_error("codex", "read_client_version_response", &source)
    })?;
    if !status.is_success() {
        return Err(ProviderError::ApiError {
            status: status.as_u16(),
            message: text,
            request_id: None,
        });
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
            message: format!("failed to parse Codex client version JSON: {source}"),
        })?;
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|version| is_valid_codex_client_version(version))
        .ok_or_else(|| ProviderError::MalformedResponse {
            message: "Codex client version response did not contain a valid version".to_string(),
        })?;
    Ok(version.to_string())
}

pub(crate) async fn latest_codex_client_version(client: &reqwest::Client) -> String {
    fetch_latest_codex_client_version(client, CODEX_CLIENT_VERSION_ENDPOINT)
        .await
        .unwrap_or_else(|_| FALLBACK_CODEX_MODELS_CLIENT_VERSION.to_string())
}

pub(super) fn is_valid_codex_client_version(value: &str) -> bool {
    let value = value.trim();
    let (core, prerelease) = value
        .split_once('-')
        .map_or((value, None), |(core, suffix)| (core, Some(suffix)));
    let mut components = core.split('.');
    let valid_core = (0..3).all(|_| {
        components.next().is_some_and(|component| {
            !component.is_empty()
                && component
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
    }) && components.next().is_none();
    let valid_prerelease = prerelease.is_none_or(|suffix| {
        !suffix.is_empty()
            && suffix.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
            })
    });
    valid_core && valid_prerelease
}

pub(super) fn timestamp_is_fresh(value: Option<&Value>, now: u64) -> bool {
    let Some(timestamp) = value
        .and_then(Value::as_str)
        .and_then(|timestamp| timestamp.parse::<u64>().ok())
    else {
        return false;
    };
    timestamp <= now && now.saturating_sub(timestamp) < MODEL_CATALOG_TTL_SECONDS
}

pub(super) fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
