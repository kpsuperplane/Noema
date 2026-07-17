use serde_json::json;
use std::sync::{Arc, Mutex};

use super::*;
use crate::{
    PersistedProviderAccountRecord, ProviderAccountRecord, ProviderAuthMethod, ProviderCredential,
    ProviderCredentialAccess, ProviderCredentialAccessHandle, ProviderCredentialFuture,
    ProviderPersistenceError, ProviderPersistenceFuture, ReasoningEffort,
    adapters::test_support::spawn_server,
};

const TEST_CODEX_CLIENT_VERSION: &str = "0.144.1";

struct RecordingCatalogPersistence {
    requests: Mutex<Vec<PersistProviderModelCatalogRequest>>,
    result: Result<PersistedProviderAccountRecord, ProviderPersistenceError>,
}

struct StaticCredentials;

impl ProviderCredentialAccess for StaticCredentials {
    fn exa_api_key<'a>(&'a self, _provider_account_id: &'a str) -> ProviderCredentialFuture<'a> {
        static_credential()
    }

    fn codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        static_credential()
    }

    fn refresh_codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        static_credential()
    }
}

fn static_credential() -> ProviderCredentialFuture<'static> {
    Box::pin(async { Ok(ProviderCredential::from("access-token".to_string())) })
}

fn credential_access() -> ProviderCredentialAccessHandle {
    Arc::new(StaticCredentials)
}

impl ProviderModelCatalogPersistence for RecordingCatalogPersistence {
    fn persist_provider_model_catalog(
        &self,
        request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        self.requests.lock().expect("requests lock").push(request);
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

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
    assert_eq!(profiles[0].id, "gpt-5.5");
    assert_eq!(profiles[0].label, "GPT-5.5");
    assert_eq!(
        profiles[0].reasoning_efforts,
        vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh
        ]
    );
    assert_eq!(
        profiles[0].default_reasoning_effort,
        Some(ReasoningEffort::Medium)
    );
}

#[test]
fn accepts_stable_and_prerelease_codex_versions_only() {
    assert!(is_valid_codex_client_version("0.144.1"));
    assert!(is_valid_codex_client_version("0.144.1-alpha.1"));
    assert!(!is_valid_codex_client_version("latest"));
    assert!(!is_valid_codex_client_version("0.144"));
    assert!(!is_valid_codex_client_version("0.144.1+build"));
}

#[test]
fn catalog_timestamp_expires_after_ttl() {
    let now = MODEL_CATALOG_TTL_SECONDS + 10_000;
    let fresh = json!((now - MODEL_CATALOG_TTL_SECONDS + 1).to_string());
    let expired = json!((now - MODEL_CATALOG_TTL_SECONDS - 1).to_string());

    assert!(timestamp_is_fresh(Some(&fresh), now));
    assert!(!timestamp_is_fresh(Some(&expired), now));
    assert!(!timestamp_is_fresh(Some(&json!(now + 1)), now));
    assert!(!timestamp_is_fresh(Some(&json!("not-a-timestamp")), now));
}

#[tokio::test]
async fn fetches_latest_codex_client_version_from_registry_shape() {
    let (version_url, request_rx) = spawn_server(
        200,
        json!({"version": TEST_CODEX_CLIENT_VERSION}).to_string(),
    )
    .await;
    let client = reqwest::Client::new();

    let version = fetch_latest_codex_client_version(&client, &format!("{version_url}/latest"))
        .await
        .expect("latest Codex version");

    let request = request_rx.await.expect("captured version request");
    assert_eq!(request.path, "/latest");
    assert_eq!(version, TEST_CODEX_CLIENT_VERSION);
}

#[tokio::test]
async fn codex_catalog_refresh_with_credentials_marks_unknown_account_authenticated() {
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
    let account = codex_account(json!({
        "base_url": base_url,
        "models_client_version": TEST_CODEX_CLIENT_VERSION,
        "models_client_version_refreshed_at": now_string()
    }));
    let persistence = RecordingCatalogPersistence {
        requests: Mutex::new(Vec::new()),
        result: Ok(account.clone().into()),
    };

    let catalog = fetch_provider_model_catalog(&credential_access(), &account)
        .await
        .expect("fetch profiles")
        .expect("catalog needed");
    persist_model_catalog_refresh(&persistence, &account, catalog)
        .await
        .expect("persist profiles");
    {
        let requests = persistence.requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].provider_account_id, account.provider_account_id);
        assert_eq!(requests[0].profiles[0].id, "gpt-live");
        assert_eq!(
            requests[0].resulting_status,
            ProviderAccountStatus::Authenticated
        );
    }

    let request = request_rx.await.expect("captured request");
    assert_eq!(
        request.path,
        format!("/models?client_version={TEST_CODEX_CLIENT_VERSION}")
    );
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("Bearer access-token")
    );
}

#[tokio::test]
async fn codex_catalog_refreshes_expired_profiles_with_latest_client_version() {
    let (version_url, version_request_rx) = spawn_server(
        200,
        json!({"version": TEST_CODEX_CLIENT_VERSION}).to_string(),
    )
    .await;
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
    let account = codex_account(json!({
        "base_url": base_url,
        "profiles": [{
            "id": "gpt-live",
            "label": "GPT Live"
        }],
        "models_source": "codex_models_endpoint",
        "models_metadata_version": MODEL_METADATA_VERSION,
        "models_refreshed_at": "0",
        "models_client_version": "0.142.3",
        "models_client_version_refreshed_at": "0"
    }));
    let persistence = RecordingCatalogPersistence {
        requests: Mutex::new(Vec::new()),
        result: Ok(account.clone().into()),
    };

    let catalog = fetch_provider_model_catalog_at_version_endpoint(
        &credential_access(),
        &account,
        &format!("{version_url}/latest"),
    )
    .await
    .expect("fetch profiles")
    .expect("catalog needed");
    persist_model_catalog_refresh(&persistence, &account, catalog)
        .await
        .expect("persist profiles");

    let version_request = version_request_rx.await.expect("captured version request");
    assert_eq!(version_request.path, "/latest");
    let request = request_rx.await.expect("captured request");
    assert_eq!(
        request.path,
        format!("/models?client_version={TEST_CODEX_CLIENT_VERSION}")
    );
    let requests = persistence.requests.lock().expect("requests lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].profiles[0].reasoning_efforts,
        vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh
        ]
    );
    assert_eq!(
        requests[0].profiles[0].default_reasoning_effort,
        Some(ReasoningEffort::Medium)
    );
    assert_eq!(requests[0].metadata_version, MODEL_METADATA_VERSION);
    assert_eq!(requests[0].client_version, TEST_CODEX_CLIENT_VERSION);
    assert!(requests[0].client_version_refreshed_at_unix.is_some());
}

#[tokio::test]
async fn catalog_persistence_failure_remains_a_provider_availability_error() {
    let account = ProviderAccountRecord {
        provider_account_id: "provider_account:codex:default".to_string(),
        provider_kind: "codex".to_string(),
        account_key: "default".to_string(),
        display_name: "Codex".to_string(),
        auth_method: ProviderAuthMethod::OauthDeviceCode,
        is_active: true,
        is_default: true,
        status: ProviderAccountStatus::Unknown,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata: json!({}),
        capabilities: Vec::new(),
    };
    let persistence = RecordingCatalogPersistence {
        requests: Mutex::new(Vec::new()),
        result: Err(ProviderPersistenceError::Persistence {
            operation: "persist_provider_model_catalog",
        }),
    };

    let error = persist_model_catalog_refresh(
        &persistence,
        &account,
        CodexModelCatalog {
            profiles: vec![ProviderModelProfile {
                id: "gpt-live".to_string(),
                label: "GPT Live".to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            }],
            client_version: TEST_CODEX_CLIENT_VERSION.to_string(),
            client_version_refreshed_at_unix: None,
        },
    )
    .await
    .expect_err("persistence failure");

    assert!(matches!(
        error,
        ProviderError::ProviderUnavailable { provider, message }
            if provider == "codex"
                && message.contains("provider persistence operation failed")
    ));
    assert_eq!(persistence.requests.lock().expect("requests lock").len(), 1);
}

fn codex_account(metadata: serde_json::Value) -> ProviderAccountRecord {
    ProviderAccountRecord {
        provider_account_id: "provider_account:codex:default".to_string(),
        provider_kind: "codex".to_string(),
        account_key: "default".to_string(),
        display_name: "Codex".to_string(),
        auth_method: ProviderAuthMethod::OauthDeviceCode,
        is_active: true,
        is_default: true,
        status: ProviderAccountStatus::Unknown,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata,
        capabilities: Vec::new(),
    }
}
