use std::fs;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tempfile::TempDir;

use super::*;
use crate::{CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthConfig, CodexOAuthTokens};

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
