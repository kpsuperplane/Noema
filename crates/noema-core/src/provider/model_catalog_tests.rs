use serde_json::json;
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

use super::*;
use crate::{provider::adapters::codex_oauth::CodexOAuthTokens, store::StoreConfig};

const TEST_CODEX_CLIENT_VERSION: &str = "0.144.1";

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
                "base_url": base_url,
                "models_client_version": TEST_CODEX_CLIENT_VERSION,
                "models_client_version_refreshed_at": now_string()
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
        format!("/models?client_version={TEST_CODEX_CLIENT_VERSION}")
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
    assert_eq!(
        updated.metadata["models_metadata_version"],
        MODEL_METADATA_VERSION
    );
    assert_eq!(
        updated.metadata["models_client_version"],
        TEST_CODEX_CLIENT_VERSION
    );
}

#[tokio::test]
async fn codex_catalog_refreshes_expired_profiles_with_latest_client_version() {
    let home = TempDir::new().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect("open store");
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
                "models_source": "codex_models_endpoint",
                "models_metadata_version": MODEL_METADATA_VERSION,
                "models_refreshed_at": "0",
                "models_client_version": "0.142.3",
                "models_client_version_refreshed_at": "0"
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

    refresh_provider_model_profiles_at_version_endpoint(
        &store,
        &paths,
        &account,
        &format!("{version_url}/latest"),
    )
    .await
    .expect("refresh profiles");

    let version_request = version_request_rx.await.expect("captured version request");
    assert_eq!(version_request.path, "/latest");
    let request = request_rx.await.expect("captured request");
    assert_eq!(
        request.path,
        format!("/models?client_version={TEST_CODEX_CLIENT_VERSION}")
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
    assert_eq!(
        updated.metadata["models_metadata_version"],
        MODEL_METADATA_VERSION
    );
    assert_eq!(
        updated.metadata["models_client_version"],
        TEST_CODEX_CLIENT_VERSION
    );
    assert!(updated.metadata["models_client_version_refreshed_at"].is_string());
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
