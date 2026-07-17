//! URL validation for browser-based MCP OAuth.

use url::{Host, Url};

use super::{McpOAuthError, McpOAuthErrorKind, McpOAuthResult};

pub(super) fn validate_https_or_loopback(value: &str, kind: &str) -> McpOAuthResult<Url> {
    let url = validate_http_url(value, kind)?;
    if url.scheme() == "http" && !url.host().is_some_and(is_loopback_host) {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must use HTTPS except on a loopback host"),
        ));
    }
    Ok(url)
}

pub(super) fn validate_loopback_redirect(value: &str) -> McpOAuthResult<Url> {
    let url = validate_http_url(value, "redirect URI")?;
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

fn validate_http_url(value: &str, kind: &str) -> McpOAuthResult<Url> {
    let url = Url::parse(value).map_err(|error| {
        McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("invalid {kind}: {error}"),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must use HTTP or HTTPS"),
        ));
    }
    if url.host().is_none() {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must include a host"),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must not include user information"),
        ));
    }
    if url.fragment().is_some() {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            format!("{kind} must not include a fragment"),
        ));
    }
    Ok(url)
}

fn is_loopback_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}
