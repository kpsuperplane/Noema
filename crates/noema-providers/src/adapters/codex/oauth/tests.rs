use std::{fs, path::Path, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use super::*;
use crate::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexDeviceAuthRequest, CodexOAuthConfig,
    CodexOAuthTokens, ProviderAuthAttemptStatus, adapters::auth::ProviderAuthManager,
};

#[test]
fn oauth_debug_redacts_credentials_and_account_paths() {
    let token_store = CodexTokenStore::new("/private/codex-account-secret");
    let oauth_client = CodexOAuthClient::new(CodexOAuthConfig {
        client_id: "oauth-client-secret".to_string(),
        ..CodexOAuthConfig::default()
    })
    .expect("oauth client");
    let device = DeviceCodeResponse {
        user_code: "user-code-secret".to_string(),
        device_auth_id: "device-auth-secret".to_string(),
        interval: Some(5),
    };
    let authorization = DeviceAuthorizationResponse {
        authorization_code: "authorization-secret".to_string(),
        code_verifier: "verifier-secret".to_string(),
    };
    let tokens = TokenResponse {
        access_token: "access-secret".to_string(),
        refresh_token: Some("refresh-secret".to_string()),
    };
    let debug = format!("{token_store:?} {oauth_client:?} {device:?} {authorization:?} {tokens:?}");

    for secret in [
        "codex-account-secret",
        "oauth-client-secret",
        "user-code-secret",
        "device-auth-secret",
        "authorization-secret",
        "verifier-secret",
        "access-secret",
        "refresh-secret",
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn token_store_does_not_treat_cli_auth_json_as_usable() {
    let dir = TempDir::new().expect("temp dir");
    let account_home = dir.path().join("providers/codex/default");
    fs::create_dir_all(&account_home).expect("account dir");
    fs::write(
        account_home.join("auth.json"),
        r#"{"tokens":{"access_token":"local","refresh_token":"local-refresh"}}"#,
    )
    .expect("local auth");

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
    let tokens = CodexOAuthTokens {
        access_token: token,
        refresh_token: "refresh".to_string(),
        last_refresh: 123,
    };

    assert!(token_needs_refresh(
        &tokens,
        CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS
    ));
}

#[test]
fn extracts_chatgpt_workspace_from_access_token() {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let claims = URL_SAFE_NO_PAD
        .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}"#);
    let token = format!("{header}.{claims}.sig");

    assert_eq!(
        chatgpt_account_id_from_access_token(&token).as_deref(),
        Some("workspace-test")
    );
}

#[test]
fn opaque_non_empty_token_is_not_preemptively_refreshed() {
    let tokens = CodexOAuthTokens {
        access_token: "opaque-token".to_string(),
        refresh_token: "refresh".to_string(),
        last_refresh: 123,
    };

    assert!(!token_needs_refresh(
        &tokens,
        CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS
    ));
}

#[tokio::test]
async fn device_auth_completion_returns_tokens_without_writing_credentials() {
    let base_url = spawn_oauth_server(vec![
        r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}"#,
        r#"{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}"#,
        r#"{"access_token":"access-secret","refresh_token":"refresh-secret"}"#,
    ])
    .await;
    let dir = TempDir::new().expect("temp dir");
    let account_home = dir.path().join("providers/codex/default");
    let manager = ProviderAuthManager::new();

    let session = manager
        .begin_codex_device_code(device_auth_request(&base_url, &account_home, None))
        .await
        .expect("begin auth");
    assert_eq!(
        session.attempt.status,
        ProviderAuthAttemptStatus::WaitingForUser
    );

    let outcome = session.completion.await;
    let debug = format!("{outcome:?}");
    assert!(!debug.contains("access-secret"));
    assert!(!debug.contains("refresh-secret"));
    let CodexDeviceAuthOutcome::Completed(tokens) = outcome else {
        panic!("expected completed device auth");
    };
    assert_eq!(tokens.access_token, "access-secret");
    assert_eq!(tokens.refresh_token, "refresh-secret");
    assert!(!account_home.join("codex_tokens.json").exists());
}

#[tokio::test]
async fn device_auth_completion_preserves_manager_cancellation() {
    let base_url = spawn_oauth_server(vec![
        r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":5}"#,
    ])
    .await;
    let dir = TempDir::new().expect("temp dir");
    let account_home = dir.path().join("providers/codex/default");
    let manager = ProviderAuthManager::new();
    let session = manager
        .begin_codex_device_code(device_auth_request(&base_url, &account_home, None))
        .await
        .expect("begin auth");

    manager
        .cancel_attempt(&session.attempt.attempt_id)
        .await
        .expect("cancel");

    assert!(matches!(
        session.completion.await,
        CodexDeviceAuthOutcome::Cancelled
    ));
    assert_eq!(
        manager
            .poll_attempt(&session.attempt.attempt_id)
            .await
            .expect("poll")
            .expect("attempt")
            .status,
        ProviderAuthAttemptStatus::Cancelled
    );
    assert!(!account_home.join("codex_tokens.json").exists());
}

#[tokio::test]
async fn device_auth_completion_preserves_attempt_expiry() {
    let base_url = spawn_oauth_server(vec![
        r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":5}"#,
    ])
    .await;
    let dir = TempDir::new().expect("temp dir");
    let account_home = dir.path().join("providers/codex/default");
    let manager = ProviderAuthManager::new();
    let session = manager
        .begin_codex_device_code(device_auth_request(
            &base_url,
            &account_home,
            Some(Duration::from_millis(1)),
        ))
        .await
        .expect("begin auth");

    assert!(matches!(
        session.completion.await,
        CodexDeviceAuthOutcome::Expired
    ));
    assert!(!account_home.join("codex_tokens.json").exists());
}

fn device_auth_request(
    base_url: &str,
    account_home: &Path,
    attempt_timeout: Option<Duration>,
) -> CodexDeviceAuthRequest {
    CodexDeviceAuthRequest {
        provider_account_id: "provider_account:codex:default".to_string(),
        account_home: account_home.to_path_buf(),
        oauth: CodexOAuthConfig {
            issuer: base_url.to_string(),
            client_id: "test-client".to_string(),
            token_url: format!("{base_url}/oauth/token"),
            timeout_seconds: 10,
        },
        attempt_timeout,
    }
}

async fn spawn_oauth_server(response_bodies: Vec<&'static str>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        for response_body in response_bodies {
            let (mut socket, _) = listener.accept().await.expect("accept");
            read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        }
    });
    format!("http://{addr}")
}

async fn read_http_request(socket: &mut TcpStream) {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = socket.read(&mut buffer).await.expect("read request");
        assert_ne!(read, 0, "client closed before request completed");
        bytes.extend_from_slice(&buffer[..read]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if bytes.len() >= header_end + 4 + content_length {
            return;
        }
    }
}
