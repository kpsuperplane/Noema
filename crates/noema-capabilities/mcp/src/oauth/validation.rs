//! URL validation for browser-based MCP OAuth.

use url::Url;

use super::{McpOAuthError, McpOAuthErrorKind, McpOAuthResult};
use crate::connection_url::{is_loopback_host, parse_https_or_loopback};

pub(super) fn validate_https_or_loopback(value: &str, kind: &str) -> McpOAuthResult<Url> {
    parse_https_or_loopback(value).ok_or_else(|| {
        McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must use HTTPS except on a loopback host"),
        )
    })
}

pub(super) fn validate_loopback_redirect(value: &str) -> McpOAuthResult<Url> {
    let url = validate_https_or_loopback(value, "redirect URI")?;
    if !url.host().is_some_and(is_loopback_host) {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            "redirect URI must use a loopback host",
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
