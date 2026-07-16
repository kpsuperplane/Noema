use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::CodexOAuthTokens;

pub(super) fn token_needs_refresh(tokens: &CodexOAuthTokens, refresh_skew_seconds: u64) -> bool {
    let token = tokens.access_token.trim();
    if token.is_empty() {
        return true;
    }
    let Some(claims_segment) = token.split('.').nth(1) else {
        return false;
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(claims_segment) else {
        return false;
    };
    let Ok(claims) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    let Some(exp) = claims.get("exp").and_then(serde_json::Value::as_u64) else {
        return false;
    };
    exp <= now_unix_seconds().saturating_add(refresh_skew_seconds)
}

/// Extract the selected ChatGPT workspace from a Codex OAuth access token.
pub(crate) fn chatgpt_account_id_from_access_token(token: &str) -> Option<String> {
    let claims_segment = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(claims_segment).ok()?;
    let claims = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .or_else(|| claims.get("chatgpt_account_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToString::to_string)
}

pub(super) fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
