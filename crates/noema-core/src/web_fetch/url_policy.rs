//! Direct-HTTP DNS resolution layered on the shared pure public URL policy.

use crate::web_fetch::types::FetchError;
use noema_capabilities::web::url_policy::{
    PublicUrlError, is_public_ip, validate_parsed_public_url, validate_public_url,
};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUrl {
    pub url: Url,
    pub resolved_addrs: Vec<SocketAddr>,
}

pub async fn validate_public_web_fetch_url(raw_url: &str) -> Result<CheckedUrl, FetchError> {
    let url = validate_public_url(raw_url).map_err(map_policy_error)?;
    resolve_public_url(url).await
}

pub async fn validate_public_web_fetch_url_parsed(url: Url) -> Result<CheckedUrl, FetchError> {
    let url = validate_parsed_public_url(url).map_err(map_policy_error)?;
    resolve_public_url(url).await
}

async fn resolve_public_url(url: Url) -> Result<CheckedUrl, FetchError> {
    let port = url
        .port_or_known_default()
        .ok_or(FetchError::MalformedUrl)?;
    let host = url.host_str().ok_or(FetchError::MalformedUrl)?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(CheckedUrl {
            url,
            resolved_addrs: vec![SocketAddr::new(ip, port)],
        });
    }

    let lookup_host = host.to_string();
    let resolved_addrs = tokio::task::spawn_blocking(move || {
        (lookup_host.as_str(), port)
            .to_socket_addrs()
            .map(|addrs| addrs.collect::<Vec<_>>())
    })
    .await
    .map_err(|_| FetchError::Dns)?
    .map_err(|_| FetchError::Dns)?;
    if resolved_addrs.is_empty() {
        return Err(FetchError::Dns);
    }
    if resolved_addrs.iter().any(|addr| !is_public_ip(addr.ip())) {
        return Err(FetchError::BlockedTarget);
    }
    Ok(CheckedUrl {
        url,
        resolved_addrs,
    })
}

fn map_policy_error(error: PublicUrlError) -> FetchError {
    match error {
        PublicUrlError::UnsupportedScheme => FetchError::UnsupportedScheme,
        PublicUrlError::Malformed => FetchError::MalformedUrl,
        PublicUrlError::BlockedTarget => FetchError::BlockedTarget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolves_public_dns_only_after_shared_policy() {
        let checked = validate_public_web_fetch_url("https://www.rust-lang.org/learn")
            .await
            .expect("public URL");
        assert!(!checked.resolved_addrs.is_empty());
    }

    #[tokio::test]
    async fn shared_policy_rejects_private_literal_before_network_work() {
        let error = validate_public_web_fetch_url("http://127.0.0.1/")
            .await
            .expect_err("private target rejected");
        assert!(matches!(error, FetchError::BlockedTarget));
    }
}
