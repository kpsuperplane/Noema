//! Provider model/profile catalog refresh helpers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde_json::{Map, Value, json};

use crate::{
    NoemaPaths, NoemaStore, ProviderAccountRecord, ProviderAccountStatus, ProviderError,
    provider::adapters::{
        codex_oauth::{
            CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthClient, CodexOAuthConfig,
            CodexTokenStore, DEFAULT_CODEX_BASE_URL,
        },
        responses::normalize_base_url,
    },
};

const MODEL_CATALOG_TIMEOUT_SECONDS: u64 = 20;

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
    if account.status != ProviderAccountStatus::Authenticated || has_metadata_profiles(account) {
        return Ok(());
    }

    let profiles = match account.provider_kind.as_str() {
        "codex" => fetch_codex_model_profiles(paths, account).await?,
        _ => return Ok(()),
    };

    if profiles.is_empty() {
        return Ok(());
    }

    let mut metadata = account.metadata.as_object().cloned().unwrap_or_default();
    metadata.insert("profiles".to_string(), Value::Array(profiles));
    metadata.insert(
        "models_refreshed_at".to_string(),
        Value::String(now_string()),
    );
    metadata.insert(
        "models_source".to_string(),
        Value::String(format!("{}_models_endpoint", account.provider_kind)),
    );

    store
        .update_provider_account_metadata(&account.provider_account_id, Value::Object(metadata))
        .await
        .map_err(|source| ProviderError::ProviderUnavailable {
            provider: account.provider_kind.clone(),
            message: format!("failed to persist model catalog metadata: {source}"),
        })
}

fn has_metadata_profiles(account: &ProviderAccountRecord) -> bool {
    account
        .metadata
        .get("profiles")
        .and_then(Value::as_array)
        .is_some_and(|profiles| !profiles.is_empty())
}

async fn fetch_codex_model_profiles(
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
) -> Result<Vec<Value>, ProviderError> {
    let base_url = account
        .metadata
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_CODEX_BASE_URL);
    let base_url = normalize_base_url(base_url.to_string(), "codex models base URL")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECONDS))
        .build()
        .map_err(|source| ProviderError::HttpFailure { source })?;
    let token_store = CodexTokenStore::new(
        paths.provider_account_home(&account.provider_kind, &account.account_key),
    );
    let oauth_client = CodexOAuthClient::new(CodexOAuthConfig::default())?;
    let access_token = token_store
        .access_token(&oauth_client, CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS)
        .await?;
    let models_url = format!("{base_url}/models");

    match fetch_model_list(&client, &models_url, &access_token).await {
        Ok(value) => Ok(profile_values_from_model_list(&value)),
        Err(ProviderError::AuthenticationFailure { .. }) => {
            let refreshed = token_store.refresh_access_token(&oauth_client).await?;
            let value = fetch_model_list(&client, &models_url, &refreshed).await?;
            Ok(profile_values_from_model_list(&value))
        }
        Err(error) => Err(error),
    }
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
        .map_err(|source| ProviderError::HttpFailure { source })?;
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .or_else(|| response.headers().get("x-oai-request-id"))
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let text = response
        .text()
        .await
        .map_err(|source| ProviderError::HttpFailure { source })?;

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

fn profile_values_from_model_list(value: &Value) -> Vec<Value> {
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|model| model_is_visible(model))
        .filter_map(profile_value_from_model)
        .collect()
}

fn model_is_visible(model: &Value) -> bool {
    if model
        .get("hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    if model
        .get("show_in_picker")
        .and_then(Value::as_bool)
        .is_some_and(|show| !show)
    {
        return false;
    }
    true
}

fn profile_value_from_model(model: &Value) -> Option<Value> {
    let id = string_field(model, &["id", "model"])?.trim();
    if id.is_empty() {
        return None;
    }
    let label = string_field(model, &["displayName", "display_name", "name"]).unwrap_or(id);
    let mut object = Map::new();
    object.insert("id".to_string(), Value::String(id.to_string()));
    object.insert("label".to_string(), Value::String(label.to_string()));
    if let Some(model_id) = string_field(model, &["model"]) {
        object.insert("model".to_string(), Value::String(model_id.to_string()));
    }
    if let Some(default_reasoning_effort) = string_field(
        model,
        &["defaultReasoningEffort", "default_reasoning_effort"],
    ) {
        object.insert(
            "default_reasoning_effort".to_string(),
            Value::String(default_reasoning_effort.to_string()),
        );
    }
    if let Some(input_modalities) = model
        .get("inputModalities")
        .or_else(|| model.get("input_modalities"))
        .and_then(Value::as_array)
    {
        object.insert(
            "input_modalities".to_string(),
            Value::Array(input_modalities.clone()),
        );
    }
    Some(Value::Object(object))
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

/// Build provider account metadata for tests and seeded local providers.
#[must_use]
pub fn profiles_metadata(profiles: &[(&str, &str)]) -> Value {
    json!({
        "profiles": profiles
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect::<Vec<_>>()
    })
}

fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_visible_profiles_from_codex_model_list() {
        let value = json!({
            "data": [
                {
                    "id": "gpt-5.5",
                    "model": "gpt-5.5",
                    "displayName": "GPT-5.5",
                    "hidden": false,
                    "defaultReasoningEffort": "medium",
                    "inputModalities": ["text", "image"]
                },
                {
                    "id": "codex-auto-review",
                    "model": "codex-auto-review",
                    "displayName": "Codex Auto Review",
                    "hidden": true
                },
                {
                    "id": "legacy-hidden",
                    "display_name": "Legacy Hidden",
                    "show_in_picker": false
                }
            ]
        });

        let profiles = profile_values_from_model_list(&value);

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["id"], "gpt-5.5");
        assert_eq!(profiles[0]["label"], "GPT-5.5");
        assert_eq!(profiles[0]["default_reasoning_effort"], "medium");
        assert_eq!(profiles[0]["input_modalities"], json!(["text", "image"]));
    }
}
