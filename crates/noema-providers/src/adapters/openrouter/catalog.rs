//! Authenticated OpenRouter account and model-catalog validation.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::StatusCode;
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;

use crate::{
    DEFAULT_OPENROUTER_BASE_URL, ProviderError, ProviderModelProfile, ReasoningEffort,
    adapters::reqwest_transport_error,
};

const CATALOG_TIMEOUT: Duration = Duration::from_secs(20);
const MINIMUM_CONTEXT_TOKENS: u64 = 32_768;

pub(crate) struct OpenRouterPkceStart {
    pub verifier: String,
    pub authorization_url: String,
}

pub(crate) fn begin_pkce(callback_url: &str) -> Result<OpenRouterPkceStart, ProviderError> {
    let mut verifier_bytes = [0_u8; 32];
    SystemRandom::new().fill(&mut verifier_bytes).map_err(|_| {
        ProviderError::ProviderUnavailable {
            provider: "openrouter".to_string(),
            message: "secure randomness is unavailable".to_string(),
        }
    })?;
    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()));
    let mut url = url::Url::parse("https://openrouter.ai/auth").map_err(|_| {
        ProviderError::InvalidRequest {
            message: "OpenRouter authorization URL is invalid".to_string(),
        }
    })?;
    url.query_pairs_mut()
        .append_pair("callback_url", callback_url)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(OpenRouterPkceStart {
        verifier,
        authorization_url: url.into(),
    })
}

pub(crate) async fn exchange_pkce_code(
    code: &str,
    verifier: &str,
) -> Result<(String, Vec<ProviderModelProfile>), ProviderError> {
    let client = reqwest::Client::builder()
        .timeout(CATALOG_TIMEOUT)
        .build()
        .map_err(|source| reqwest_transport_error("openrouter", "build_oauth_client", &source))?;
    let response = client
        .post(format!("{DEFAULT_OPENROUTER_BASE_URL}/auth/keys"))
        .json(&serde_json::json!({
            "code": code,
            "code_verifier": verifier,
            "code_challenge_method": "S256"
        }))
        .send()
        .await
        .map_err(|source| reqwest_transport_error("openrouter", "exchange_oauth_code", &source))?;
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|source| reqwest_transport_error("openrouter", "read_oauth_exchange", &source))?;
    if !status.is_success() {
        return Err(ProviderError::AuthenticationFailure {
            message: "OpenRouter authorization code was rejected".to_string(),
            request_id: None,
        });
    }
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProviderError::MalformedResponse {
            message: "OpenRouter key exchange returned no key".to_string(),
        })?
        .to_string();
    let profiles = validate_api_key_with_client(&client, DEFAULT_OPENROUTER_BASE_URL, &key).await?;
    Ok((key, profiles))
}

pub(crate) async fn validate_api_key(
    api_key: &str,
) -> Result<Vec<ProviderModelProfile>, ProviderError> {
    let client = reqwest::Client::builder()
        .timeout(CATALOG_TIMEOUT)
        .build()
        .map_err(|source| reqwest_transport_error("openrouter", "build_catalog_client", &source))?;
    validate_api_key_with_client(&client, DEFAULT_OPENROUTER_BASE_URL, api_key).await
}

pub(crate) async fn validate_api_key_with_client(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ProviderModelProfile>, ProviderError> {
    fetch_json(client, &format!("{base_url}/key"), api_key).await?;
    let models = fetch_json(client, &format!("{base_url}/models/user"), api_key).await?;
    let profiles = profiles_from_models(&models);
    if profiles.is_empty() {
        return Err(ProviderError::ProviderUnavailable {
            provider: "openrouter".to_string(),
            message: "OpenRouter account has no compatible models".to_string(),
        });
    }
    Ok(profiles)
}

async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<Value, ProviderError> {
    let response = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|source| reqwest_transport_error("openrouter", "validate_account", &source))?;
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let text = response.text().await.map_err(|source| {
        reqwest_transport_error("openrouter", "read_account_validation", &source)
    })?;
    if !status.is_success() {
        return if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            Err(ProviderError::AuthenticationFailure {
                message: "OpenRouter rejected the credential".to_string(),
                request_id,
            })
        } else {
            Err(ProviderError::ApiError {
                status: status.as_u16(),
                message: text,
                request_id,
            })
        };
    }
    serde_json::from_str(&text).map_err(|error| ProviderError::MalformedResponse {
        message: format!("failed to parse OpenRouter response: {error}"),
    })
}

pub(crate) fn profiles_from_models(value: &Value) -> Vec<ProviderModelProfile> {
    let mut profiles = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(profile_from_model)
        .collect::<Vec<_>>();
    if !profiles.is_empty()
        && !profiles
            .iter()
            .any(|profile| profile.id == "openrouter/auto")
    {
        profiles.push(ProviderModelProfile {
            id: "openrouter/auto".to_string(),
            label: "OpenRouter Auto".to_string(),
            reasoning_efforts: Vec::new(),
            default_reasoning_effort: None,
        });
    }
    profiles.sort_by(|left, right| {
        let left_auto = left.id == "openrouter/auto";
        let right_auto = right.id == "openrouter/auto";
        right_auto
            .cmp(&left_auto)
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    profiles
}

fn profile_from_model(model: &Value) -> Option<ProviderModelProfile> {
    let id = model.get("id")?.as_str()?.trim();
    let context = model.get("context_length")?.as_u64()?;
    let outputs = model
        .pointer("/architecture/output_modalities")?
        .as_array()?;
    let parameters = model.get("supported_parameters")?.as_array()?;
    let supports = |name: &str| parameters.iter().any(|value| value.as_str() == Some(name));
    if id.is_empty()
        || context < MINIMUM_CONTEXT_TOKENS
        || !outputs.iter().any(|value| value.as_str() == Some("text"))
        || !supports("tools")
        || !supports("tool_choice")
    {
        return None;
    }
    let label = model
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let efforts = model
        .pointer("/reasoning/supported_efforts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(reasoning_effort)
        .collect();
    let default_reasoning_effort = model
        .pointer("/reasoning/default_effort")
        .and_then(Value::as_str)
        .and_then(reasoning_effort);
    Some(ProviderModelProfile {
        id: id.to_string(),
        label: label.to_string(),
        reasoning_efforts: efforts,
        default_reasoning_effort,
    })
}

fn reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_ascii_lowercase().as_str() {
        "max" => Some(ReasoningEffort::XHigh),
        value => ReasoningEffort::from_persistence_str(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_filters_incompatible_models_and_maps_reasoning() {
        let profiles = profiles_from_models(&json!({"data": [
            {
                "id": "vendor/zeta",
                "name": "Zeta",
                "context_length": 65536,
                "architecture": {"output_modalities": ["text"]},
                "supported_parameters": ["tools", "tool_choice", "structured_outputs"],
                "reasoning": {"supported_efforts": ["low", "max"], "default_effort": "max"}
            },
            {
                "id": "vendor/no-tools",
                "name": "No tools",
                "context_length": 65536,
                "architecture": {"output_modalities": ["text"]},
                "supported_parameters": ["structured_outputs"]
            },
            {
                "id": "vendor/short",
                "name": "Short",
                "context_length": 8192,
                "architecture": {"output_modalities": ["text"]},
                "supported_parameters": ["tools", "tool_choice", "structured_outputs"]
            }
        ]}));

        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["openrouter/auto", "vendor/zeta"]
        );
        assert_eq!(
            profiles[1].reasoning_efforts,
            vec![ReasoningEffort::Low, ReasoningEffort::XHigh]
        );
        assert_eq!(
            profiles[1].default_reasoning_effort,
            Some(ReasoningEffort::XHigh)
        );
    }

    #[test]
    fn pkce_start_uses_s256_and_never_places_the_verifier_in_the_url() {
        let start = begin_pkce("http://127.0.0.1:43123/provider/oauth/callback/opaque")
            .expect("PKCE start");
        let url = url::Url::parse(&start.authorization_url).expect("authorization URL");
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
        assert_eq!(
            query.get("callback_url").map(|value| value.as_ref()),
            Some("http://127.0.0.1:43123/provider/oauth/callback/opaque")
        );
        assert_ne!(
            query.get("code_challenge").map(|value| value.as_ref()),
            Some(start.verifier.as_str())
        );
        assert!(start.verifier.len() >= 43);
    }
}
