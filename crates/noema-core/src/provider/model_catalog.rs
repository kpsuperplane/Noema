//! Provider model/profile catalog refresh helpers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde_json::{Map, Value};

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
const CODEX_MODELS_CLIENT_VERSION: &str = "0.142.3";
const MODEL_METADATA_VERSION: u64 = 2;

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
    if metadata_profiles_are_current(account) || !should_refresh_model_profiles(account) {
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
    metadata.insert(
        "models_metadata_version".to_string(),
        Value::from(MODEL_METADATA_VERSION),
    );

    store
        .update_provider_account_metadata(&account.provider_account_id, Value::Object(metadata))
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
    let models_url = format!("{base_url}/models?client_version={CODEX_MODELS_CLIENT_VERSION}");

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

fn profile_value_from_model(model: &Value) -> Option<Value> {
    let id = string_field(model, &["slug"])?.trim();
    if id.is_empty() {
        return None;
    }
    let label = string_field(model, &["display_name"]).unwrap_or(id);
    let mut object = Map::new();
    object.insert("id".to_string(), Value::String(id.to_string()));
    object.insert("label".to_string(), Value::String(label.to_string()));
    if let Some(reasoning_efforts) = reasoning_efforts_from_model(model) {
        object.insert("reasoning_efforts".to_string(), reasoning_efforts);
    }
    if let Some(default_reasoning_effort) = string_field(
        model,
        &["default_reasoning_level", "default_reasoning_effort"],
    )
    .and_then(normalize_reasoning_effort)
    {
        object.insert(
            "default_reasoning_effort".to_string(),
            Value::String(default_reasoning_effort.to_string()),
        );
    }
    Some(Value::Object(object))
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn reasoning_efforts_from_model(model: &Value) -> Option<Value> {
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
            .map(|effort| Value::String(effort.to_string()))
            .collect::<Vec<_>>()
    })
    .filter(|values| !values.is_empty());
    if explicit.is_some() {
        return explicit.map(Value::Array);
    }
    string_field(
        model,
        &["default_reasoning_level", "default_reasoning_effort"],
    )
    .and_then(normalize_reasoning_effort)
    .map(|_| {
        Value::Array(
            ["low", "medium", "high", "xhigh"]
                .into_iter()
                .map(|effort| Value::String(effort.to_string()))
                .collect(),
        )
    })
}

fn normalize_reasoning_effort(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some("none"),
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        _ => None,
    }
}

fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::*;
    use crate::{provider::adapters::codex_oauth::CodexOAuthTokens, store::StoreConfig};

    #[test]
    fn extracts_visible_profiles_from_codex_model_list() {
        let value = json!({
            "models": [
                {
                    "slug": "gpt-5.5",
                    "display_name": "GPT-5.5",
                    "visibility": "list",
                    "default_reasoning_level": "medium",
                    "supported_reasoning_levels": ["low", "medium", "high", "xhigh"],
                    "input_modalities": ["text", "image"]
                },
                {
                    "slug": "codex-auto-review",
                    "display_name": "Codex Auto Review",
                    "visibility": "hide"
                }
            ]
        });

        let profiles = profile_values_from_model_list(&value);

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["id"], "gpt-5.5");
        assert_eq!(profiles[0]["label"], "GPT-5.5");
        assert_eq!(
            profiles[0]["reasoning_efforts"],
            json!(["low", "medium", "high", "xhigh"])
        );
        assert_eq!(profiles[0]["default_reasoning_effort"], "medium");
    }

    #[tokio::test]
    async fn codex_catalog_refresh_with_tokens_marks_unknown_account_authenticated() {
        let home = TempDir::new().expect("temp noema home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let (base_url, request_rx) = spawn_server(
            200,
            json!({
                "models": [
                    {
                        "slug": "gpt-live",
                        "display_name": "GPT Live",
                        "visibility": "list"
                    }
                ]
            })
            .to_string(),
        )
        .await;
        let created = store
            .ensure_default_provider_account()
            .await
            .expect("default account");
        store
            .update_provider_account_metadata(
                &created.provider_account_id,
                json!({
                    "base_url": base_url
                }),
            )
            .await
            .expect("set catalog base url");
        CodexTokenStore::new(paths.provider_account_home("codex", "default"))
            .write(&CodexOAuthTokens {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                last_refresh: 123,
            })
            .expect("write tokens");
        let account = store
            .get_provider_account(&created.provider_account_id)
            .await
            .expect("get account")
            .expect("account exists");

        refresh_provider_model_profiles(&store, &paths, &account)
            .await
            .expect("refresh profiles");

        let request = request_rx.await.expect("captured request");
        assert_eq!(
            request.path,
            format!("/models?client_version={CODEX_MODELS_CLIENT_VERSION}")
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer access-token")
        );
        let updated = store
            .get_provider_account(&created.provider_account_id)
            .await
            .expect("get updated account")
            .expect("updated account exists");
        assert_eq!(updated.status, ProviderAccountStatus::Authenticated);
        assert_eq!(updated.metadata["profiles"][0]["id"], "gpt-live");
        assert_eq!(updated.metadata["profiles"][0]["label"], "GPT Live");
        assert_eq!(updated.metadata["models_metadata_version"], 2);
    }

    #[tokio::test]
    async fn codex_catalog_refresh_replaces_stale_profiles_without_reasoning_metadata() {
        let home = TempDir::new().expect("temp noema home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let (base_url, request_rx) = spawn_server(
            200,
            json!({
                "models": [
                    {
                        "slug": "gpt-live",
                        "display_name": "GPT Live",
                        "visibility": "list",
                        "default_reasoning_level": "medium",
                        "supported_reasoning_levels": ["low", "medium", "high", "xhigh"]
                    }
                ]
            })
            .to_string(),
        )
        .await;
        let created = store
            .ensure_default_provider_account()
            .await
            .expect("default account");
        store
            .update_provider_account_metadata(
                &created.provider_account_id,
                json!({
                    "base_url": base_url,
                    "profiles": [{
                        "id": "gpt-live",
                        "label": "GPT Live"
                    }],
                    "models_source": "codex_models_endpoint"
                }),
            )
            .await
            .expect("set stale metadata");
        CodexTokenStore::new(paths.provider_account_home("codex", "default"))
            .write(&CodexOAuthTokens {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                last_refresh: 123,
            })
            .expect("write tokens");
        let account = store
            .get_provider_account(&created.provider_account_id)
            .await
            .expect("get account")
            .expect("account exists");

        refresh_provider_model_profiles(&store, &paths, &account)
            .await
            .expect("refresh profiles");

        let request = request_rx.await.expect("captured request");
        assert_eq!(
            request.path,
            format!("/models?client_version={CODEX_MODELS_CLIENT_VERSION}")
        );
        let updated = store
            .get_provider_account(&created.provider_account_id)
            .await
            .expect("get updated account")
            .expect("updated account exists");
        assert_eq!(
            updated.metadata["profiles"][0]["reasoning_efforts"],
            json!(["low", "medium", "high", "xhigh"])
        );
        assert_eq!(
            updated.metadata["profiles"][0]["default_reasoning_effort"],
            "medium"
        );
        assert_eq!(updated.metadata["models_metadata_version"], 2);
    }

    #[derive(Debug)]
    struct CapturedRequest {
        path: String,
        headers: std::collections::HashMap<String, String>,
    }

    async fn spawn_server(
        status: u16,
        response_body: String,
    ) -> (String, oneshot::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (request_tx, request_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut socket).await;
            let _ = request_tx.send(request);
            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                429 => "Too Many Requests",
                _ => "Error",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });

        (format!("http://{addr}"), request_rx)
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> CapturedRequest {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = socket.read(&mut buffer).await.expect("read request");
            assert_ne!(read, 0, "client closed before complete request");
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut lines = text.lines();
        let path = lines
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or_default()
            .to_string();
        let headers = lines
            .take_while(|line| !line.is_empty())
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
            .collect();
        CapturedRequest { path, headers }
    }
}
