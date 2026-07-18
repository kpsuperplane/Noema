use std::{fs, path::Path, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tempfile::TempDir;
use tokio::sync::oneshot;

use super::*;
use crate::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexDeviceAuthRequest, CodexOAuthConfig,
    CodexOAuthTokens, ProviderAuthAttemptStatus,
    adapters::{
        auth::ProviderAuthManager,
        test_support::{spawn_blocking_server, spawn_scripted_server},
    },
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
fn device_code_response_accepts_string_and_numeric_intervals() {
    for interval in [r#""5""#, "5"] {
        let response: DeviceCodeResponse = serde_json::from_str(&format!(
            r#"{{"device_auth_id":"deviceauth_test","user_code":"ABCD-EFGH","interval":{interval}}}"#
        ))
        .expect("device code response");
        assert_eq!(response.interval, Some(5));
    }
}

#[test]
fn token_refresh_check_uses_jwt_exp_and_accepts_opaque_tokens() {
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
    let opaque = CodexOAuthTokens {
        access_token: "opaque-token".to_string(),
        refresh_token: "refresh".to_string(),
        last_refresh: 123,
    };
    assert!(!token_needs_refresh(
        &opaque,
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

#[tokio::test]
async fn device_auth_cancels_an_inflight_token_exchange() {
    let (base_url, exchange_started, release_exchange) =
        spawn_blocking_token_exchange_server().await;
    let dir = TempDir::new().expect("temp dir");
    let account_home = dir.path().join("providers/codex/default");
    let manager = ProviderAuthManager::new();
    let session = manager
        .begin_codex_device_code(device_auth_request(&base_url, &account_home, None))
        .await
        .expect("begin auth");
    let attempt_id = session.attempt.attempt_id.clone();
    let completion = tokio::spawn(session.completion);

    exchange_started.await.expect("exchange started");
    manager
        .cancel_attempt(&attempt_id)
        .await
        .expect("cancel attempt");
    let outcome = completion.await.expect("completion task");
    release_exchange.send(()).expect("release exchange");

    assert!(matches!(outcome, CodexDeviceAuthOutcome::Cancelled));
    assert_eq!(
        manager
            .poll_attempt(&attempt_id)
            .await
            .expect("poll")
            .expect("attempt")
            .status,
        ProviderAuthAttemptStatus::Cancelled
    );
}

#[tokio::test]
async fn device_auth_failure_does_not_expose_remote_response_text() {
    let base_url = spawn_oauth_server_with_status(vec![
        (
            200,
            r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}"#,
        ),
        (
            500,
            r#"{"error":{"message":"echoed ABCD-EFGH device-secret authorization-secret"}}"#,
        ),
    ])
    .await;
    let dir = TempDir::new().expect("temp dir");
    let manager = ProviderAuthManager::new();
    let session = manager
        .begin_codex_device_code(device_auth_request(&base_url, dir.path(), None))
        .await
        .expect("begin auth");

    let outcome = session.completion.await;
    let CodexDeviceAuthOutcome::Failed { error_message, .. } = outcome else {
        panic!("expected failed auth");
    };
    assert_eq!(error_message, "Codex login request failed");
    for secret in ["ABCD-EFGH", "device-secret", "authorization-secret"] {
        assert!(!error_message.contains(secret));
    }
}

#[tokio::test]
async fn token_endpoint_failure_does_not_expose_remote_response_text() {
    let base_url = spawn_oauth_server_with_status(vec![(
        400,
        r#"{"error":{"code":"invalid_grant","message":"echoed refresh-secret"}}"#,
    )])
    .await;
    let client = CodexOAuthClient::new(CodexOAuthConfig {
        issuer: base_url.clone(),
        client_id: "test-client".to_string(),
        token_url: format!("{base_url}/oauth/token"),
        timeout_seconds: 10,
    })
    .expect("OAuth client");

    let error = client
        .refresh_tokens("refresh-secret")
        .await
        .expect_err("refresh must fail");
    let message = error.to_string();

    assert!(matches!(
        error,
        crate::ProviderError::AuthenticationFailure { .. }
    ));
    assert!(!message.contains("refresh-secret"));
    assert!(!message.contains("echoed"));
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
    let (base_url, _requests) =
        spawn_scripted_server(response_bodies.into_iter().map(|body| (200, body))).await;
    base_url
}

async fn spawn_oauth_server_with_status(responses: Vec<(u16, &'static str)>) -> String {
    let (base_url, _requests) = spawn_scripted_server(responses).await;
    base_url
}

async fn spawn_blocking_token_exchange_server()
-> (String, oneshot::Receiver<()>, oneshot::Sender<()>) {
    spawn_blocking_server(
        vec![
            (
                200,
                r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}"#
                    .to_string(),
            ),
            (
                200,
                r#"{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}"#
                    .to_string(),
            ),
        ],
        200,
        r#"{"access_token":"late-access","refresh_token":"late-refresh"}"#,
    )
    .await
}
