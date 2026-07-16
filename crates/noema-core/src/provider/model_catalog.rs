//! Provider model/profile catalog refresh helpers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use noema_home::NoemaPaths;
use noema_providers::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthConfig, DEFAULT_CODEX_BASE_URL,
    ProviderAccountRecord, ProviderAccountStatus, ProviderError, ProviderModelProfile,
    ReasoningEffort,
};
use reqwest::StatusCode;
use serde_json::Value;

use crate::{
    NoemaStore,
    provider::adapters::{
        codex_oauth::{CodexOAuthClient, CodexTokenStore},
        reqwest_transport_error,
        responses::normalize_base_url,
    },
};

const MODEL_CATALOG_TIMEOUT_SECONDS: u64 = 20;
const CODEX_CLIENT_VERSION_ENDPOINT: &str = "https://registry.npmjs.org/@openai%2fcodex/latest";
pub(crate) const FALLBACK_CODEX_MODELS_CLIENT_VERSION: &str = "0.144.0";
const MODEL_CATALOG_TTL_SECONDS: u64 = 6 * 60 * 60;
const MODEL_METADATA_VERSION: u64 = 3;
const CODEX_VERSION_USER_AGENT: &str = "Noema/0.1 (+https://github.com/kpsuperplane/Noema)";

/// Best-effort refresh of provider model profiles for Settings.
///
/// This stores only non-secret display metadata under provider account metadata.
/// Callers should treat errors as advisory and keep rendering existing account
/// state rather than inventing fallback model lists.
///
/// # Errors
///
/// Returns [`ProviderError`] when the provider catalog endpoint, provider
/// credentials, or metadata persistence fails.
pub async fn refresh_provider_model_profiles(
    store: &NoemaStore,
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
) -> Result<(), ProviderError> {
    refresh_provider_model_profiles_at_version_endpoint(
        store,
        paths,
        account,
        CODEX_CLIENT_VERSION_ENDPOINT,
    )
    .await
}

async fn refresh_provider_model_profiles_at_version_endpoint(
    store: &NoemaStore,
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<(), ProviderError> {
    if metadata_profiles_are_current(account) || !should_refresh_model_profiles(account) {
        return Ok(());
    }

    let catalog = match account.provider_kind.as_str() {
        "codex" => fetch_codex_model_profiles(paths, account, version_endpoint).await?,
        _ => return Ok(()),
    };

    if catalog.profiles.is_empty() {
        return Ok(());
    }

    let mut metadata = account.metadata.clone();
    ProviderModelProfile::write_account_metadata(&mut metadata, &catalog.profiles).map_err(
        |source| ProviderError::MalformedResponse {
            message: format!("failed to serialize model catalog profiles: {source}"),
        },
    )?;
    let metadata_object = metadata
        .as_object_mut()
        .expect("provider profile metadata was normalized to an object");
    metadata_object.insert(
        "models_refreshed_at".to_string(),
        Value::String(now_string()),
    );
    metadata_object.insert(
        "models_source".to_string(),
        Value::String(format!("{}_models_endpoint", account.provider_kind)),
    );
    metadata_object.insert(
        "models_metadata_version".to_string(),
        Value::from(MODEL_METADATA_VERSION),
    );
    metadata_object.insert(
        "models_client_version".to_string(),
        Value::String(catalog.client_version),
    );
    if let Some(refreshed_at) = catalog.client_version_refreshed_at {
        metadata_object.insert(
            "models_client_version_refreshed_at".to_string(),
            Value::String(refreshed_at),
        );
    }

    store
        .update_provider_account_metadata(&account.provider_account_id, metadata)
        .await
        .map_err(|source| ProviderError::ProviderUnavailable {
            provider: account.provider_kind.clone(),
            message: format!("failed to persist model catalog metadata: {source}"),
        })?;

    if account.status != ProviderAccountStatus::Authenticated {
        store
            .update_provider_account_status(
                &account.provider_account_id,
                ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .map_err(|source| ProviderError::ProviderUnavailable {
                provider: account.provider_kind.clone(),
                message: format!("failed to persist provider account status: {source}"),
            })?;
    }

    Ok(())
}

fn should_refresh_model_profiles(account: &ProviderAccountRecord) -> bool {
    match account.provider_kind.as_str() {
        // Codex OAuth credentials live in Noema's provider account home. A
        // successful catalog read is the authoritative auth check for Settings.
        "codex" => true,
        _ => account.status == ProviderAccountStatus::Authenticated,
    }
}

fn metadata_profiles_are_current(account: &ProviderAccountRecord) -> bool {
    let has_profiles = account
        .metadata
        .get("profiles")
        .and_then(Value::as_array)
        .is_some_and(|profiles| !profiles.is_empty());
    if !has_profiles {
        return false;
    }
    if account.provider_kind != "codex" {
        return true;
    }
    account
        .metadata
        .get("models_metadata_version")
        .and_then(Value::as_u64)
        == Some(MODEL_METADATA_VERSION)
        && account
            .metadata
            .get("models_client_version")
            .and_then(Value::as_str)
            .is_some_and(is_valid_codex_client_version)
        && timestamp_is_fresh(
            account.metadata.get("models_refreshed_at"),
            current_unix_timestamp(),
        )
}

async fn fetch_codex_model_profiles(
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<CodexModelCatalog, ProviderError> {
    let base_url = account
        .metadata
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_CODEX_BASE_URL);
    let base_url = normalize_base_url(base_url.to_string(), "codex models base URL")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECONDS))
        .build()
        .map_err(|source| {
            reqwest_transport_error(
                &account.provider_kind,
                "build_model_catalog_client",
                &source,
            )
        })?;
    let token_store = CodexTokenStore::new(
        paths.provider_account_home(&account.provider_kind, &account.account_key),
    );
    let oauth_client = CodexOAuthClient::new(CodexOAuthConfig::default())?;
    let access_token = token_store
        .access_token(&oauth_client, CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS)
        .await?;
    let client_version =
        resolve_codex_client_version(&client, &account.metadata, version_endpoint).await;
    let models_url = format!("{base_url}/models?client_version={}", client_version.value);

    let value = match fetch_model_list(&client, &models_url, &access_token).await {
        Ok(value) => value,
        Err(ProviderError::AuthenticationFailure { .. }) => {
            let refreshed = token_store.refresh_access_token(&oauth_client).await?;
            fetch_model_list(&client, &models_url, &refreshed).await?
        }
        Err(error) => return Err(error),
    };
    Ok(CodexModelCatalog {
        profiles: profile_values_from_model_list(&value),
        client_version: client_version.value,
        client_version_refreshed_at: client_version.refreshed_at,
    })
}

#[derive(Debug)]
struct CodexModelCatalog {
    profiles: Vec<ProviderModelProfile>,
    client_version: String,
    client_version_refreshed_at: Option<String>,
}

#[derive(Debug)]
struct ResolvedCodexClientVersion {
    value: String,
    refreshed_at: Option<String>,
}

async fn resolve_codex_client_version(
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
            refreshed_at: None,
        };
    }

    match fetch_latest_codex_client_version(client, version_endpoint).await {
        Ok(value) => ResolvedCodexClientVersion {
            value,
            refreshed_at: Some(now_string()),
        },
        Err(_) => ResolvedCodexClientVersion {
            value: cached_version
                .unwrap_or_else(|| FALLBACK_CODEX_MODELS_CLIENT_VERSION.to_string()),
            refreshed_at: None,
        },
    }
}

async fn fetch_latest_codex_client_version(
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

async fn fetch_model_list(
    client: &reqwest::Client,
    models_url: &str,
    bearer_token: &str,
) -> Result<Value, ProviderError> {
    let response = client
        .get(models_url)
        .bearer_auth(bearer_token)
        .send()
        .await
        .map_err(|source| reqwest_transport_error("codex", "fetch_model_catalog", &source))?;
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .or_else(|| response.headers().get("x-oai-request-id"))
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let text = response.text().await.map_err(|source| {
        reqwest_transport_error("codex", "read_model_catalog_response", &source)
    })?;

    if !status.is_success() {
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(ProviderError::AuthenticationFailure {
                message: "model catalog authentication failed".to_string(),
                request_id,
            });
        }
        return Err(ProviderError::ApiError {
            status: status.as_u16(),
            message: text,
            request_id,
        });
    }

    serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
        message: format!("failed to parse model catalog JSON: {source}"),
    })
}

fn profile_values_from_model_list(value: &Value) -> Vec<ProviderModelProfile> {
    value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|model| model_is_visible(model))
        .filter_map(profile_value_from_model)
        .collect()
}

fn model_is_visible(model: &Value) -> bool {
    model
        .get("visibility")
        .and_then(Value::as_str)
        .is_some_and(|visibility| visibility == "list")
}

fn profile_value_from_model(model: &Value) -> Option<ProviderModelProfile> {
    let id = string_field(model, &["slug"])?.trim();
    if id.is_empty() {
        return None;
    }
    let label = string_field(model, &["display_name"]).unwrap_or(id);
    Some(ProviderModelProfile {
        id: id.to_string(),
        label: label.to_string(),
        reasoning_efforts: reasoning_efforts_from_model(model),
        default_reasoning_effort: string_field(
            model,
            &["default_reasoning_level", "default_reasoning_effort"],
        )
        .and_then(normalize_reasoning_effort),
    })
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn reasoning_efforts_from_model(model: &Value) -> Vec<ReasoningEffort> {
    let explicit = [
        "supported_reasoning_levels",
        "reasoning_levels",
        "reasoning_efforts",
    ]
    .iter()
    .find_map(|key| model.get(*key).and_then(Value::as_array))
    .map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .filter_map(normalize_reasoning_effort)
            .collect::<Vec<_>>()
    })
    .filter(|values| !values.is_empty());
    if let Some(explicit) = explicit {
        return explicit;
    }
    string_field(
        model,
        &["default_reasoning_level", "default_reasoning_effort"],
    )
    .and_then(normalize_reasoning_effort)
    .map_or_else(Vec::new, |_| {
        vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ]
    })
}

fn normalize_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    ReasoningEffort::from_persistence_str(&value.trim().to_ascii_lowercase())
}

fn is_valid_codex_client_version(value: &str) -> bool {
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

fn timestamp_is_fresh(value: Option<&Value>, now: u64) -> bool {
    let Some(timestamp) = value
        .and_then(Value::as_str)
        .and_then(|timestamp| timestamp.parse::<u64>().ok())
    else {
        return false;
    };
    timestamp <= now && now.saturating_sub(timestamp) < MODEL_CATALOG_TTL_SECONDS
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn now_string() -> String {
    current_unix_timestamp().to_string()
}

#[cfg(test)]
#[path = "model_catalog_tests.rs"]
mod tests;
