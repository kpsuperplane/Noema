//! Direct-HTTP DNS resolution layered on the shared pure public URL policy.

use crate::url_policy::{PublicUrlError, is_public_ip};
use std::net::{IpAddr, ToSocketAddrs};
use url::Url;

/// Validate and resolve one public URL.
///
/// # Errors
/// Returns [`WebFetchError`] when the URL or its resolved addresses are unsafe.
pub async fn validate_public_url(raw_url: &str) -> Result<Url, WebFetchError> {
    let url = crate::url_policy::validate_public_url(raw_url).map_err(map_policy_error)?;
    resolve_public_url(url).await
}

async fn resolve_public_url(url: Url) -> Result<Url, WebFetchError> {
    let port = url
        .port_or_known_default()
        .ok_or(WebFetchError::MalformedUrl)?;
    let host = url.host_str().ok_or(WebFetchError::MalformedUrl)?;
    if host.parse::<IpAddr>().is_ok() {
        return Ok(url);
    }

    let lookup_host = host.to_string();
    let resolved_addrs = tokio::task::spawn_blocking(move || {
        (lookup_host.as_str(), port)
            .to_socket_addrs()
            .map(|addrs| addrs.collect::<Vec<_>>())
    })
    .await
    .map_err(|_| WebFetchError::Dns)?
    .map_err(|_| WebFetchError::Dns)?;
    if resolved_addrs.is_empty() {
        return Err(WebFetchError::Dns);
    }
    if resolved_addrs.iter().any(|addr| !is_public_ip(addr.ip())) {
        return Err(WebFetchError::BlockedTarget);
    }
    Ok(url)
}

fn map_policy_error(error: PublicUrlError) -> WebFetchError {
    match error {
        PublicUrlError::UnsupportedScheme => WebFetchError::UnsupportedScheme,
        PublicUrlError::Malformed => WebFetchError::MalformedUrl,
        PublicUrlError::BlockedTarget => WebFetchError::BlockedTarget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shared_policy_rejects_private_literal_before_network_work() {
        let error = validate_public_url("http://127.0.0.1/")
            .await
            .expect_err("private target rejected");
        assert!(matches!(error, WebFetchError::BlockedTarget));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebFetchError {
    #[error("unsupported URL scheme")]
    UnsupportedScheme,
    #[error("malformed URL")]
    MalformedUrl,
    #[error("blocked private or local target")]
    BlockedTarget,
    #[error("DNS lookup failed")]
    Dns,
}
