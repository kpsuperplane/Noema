//! URL validation for browser-based MCP OAuth.

use url::Url;

use super::{McpOAuthError, McpOAuthErrorKind, McpOAuthResult};
use crate::connection_url::parse_https_or_loopback;

pub(super) fn validate_https_or_loopback(value: &str, kind: &str) -> McpOAuthResult<Url> {
    parse_https_or_loopback(value).ok_or_else(|| {
        McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must use HTTPS except on a loopback host"),
        )
    })
}

const CALLBACK_PATH: &str = "/mcp/oauth/callback";

pub(super) fn validate_redirect(value: &str) -> McpOAuthResult<Url> {
    let url = validate_https_or_loopback(value, "redirect URI")?;
    if url.path() != CALLBACK_PATH || url.query().is_some() || url.fragment().is_some() {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            "redirect URI must use the exact callback path without query or fragment data",
        ));
    }
    Ok(url)
}

pub(super) fn callback_uri(mut redirect: Url, attempt_id: &str) -> Url {
    redirect
        .query_pairs_mut()
        .append_pair("attemptId", attempt_id);
    redirect
}

pub(super) fn validate_callback_url(
    value: &str,
    callback_base: &Url,
    attempt_id: &str,
) -> McpOAuthResult<()> {
    let callback = validate_https_or_loopback(value, "callback URL")?;
    let same_target =
        callback.origin() == callback_base.origin() && callback.path() == callback_base.path();
    let matching_attempts = callback
        .query_pairs()
        .filter(|(name, value)| name == "attemptId" && value == attempt_id)
        .count();
    let attempt_fields = callback
        .query_pairs()
        .filter(|(name, _)| name == "attemptId")
        .count();
    if !same_target || matching_attempts != 1 || attempt_fields != 1 {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            "callback URL does not match the initiating callback target",
        ));
    }
    Ok(())
}
