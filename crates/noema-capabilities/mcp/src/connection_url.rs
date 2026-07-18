//! Shared validation and DNS pinning for MCP connection URLs.

use std::net::{IpAddr, SocketAddr};

use noema_capabilities::web::url_policy::is_public_ip;
use url::{Host, Url};

pub(crate) fn parse_https_or_loopback(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && (url.scheme() == "https" || url.host().is_some_and(is_loopback_host)))
    .then_some(url)
}

pub(crate) fn is_loopback_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetResolutionError {
    InvalidUrl,
    MissingPort,
    Dns,
    NoAddresses,
    DisallowedAddress,
}

pub(crate) async fn resolve_allowed_target(
    url: &Url,
) -> Result<Option<(String, Vec<SocketAddr>)>, TargetResolutionError> {
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(TargetResolutionError::InvalidUrl);
    }
    let host = url.host().ok_or(TargetResolutionError::InvalidUrl)?;
    let loopback = is_loopback_host(host.clone());
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(TargetResolutionError::InvalidUrl);
    }
    let allowed = |address: IpAddr| {
        if loopback {
            address.is_loopback()
        } else {
            is_public_ip(address)
        }
    };
    match host {
        Host::Ipv4(address) if allowed(address.into()) => Ok(None),
        Host::Ipv6(address) if allowed(address.into()) => Ok(None),
        Host::Ipv4(_) | Host::Ipv6(_) => Err(TargetResolutionError::DisallowedAddress),
        Host::Domain(hostname) => {
            let port = url
                .port_or_known_default()
                .ok_or(TargetResolutionError::MissingPort)?;
            let addresses = tokio::net::lookup_host((hostname, port))
                .await
                .map_err(|_| TargetResolutionError::Dns)?
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return Err(TargetResolutionError::NoAddresses);
            }
            if addresses.iter().any(|address| !allowed(address.ip())) {
                return Err(TargetResolutionError::DisallowedAddress);
            }
            Ok(Some((hostname.to_string(), addresses)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn target_resolution_allows_public_https_and_explicit_loopback_only() {
        for allowed in ["http://127.0.0.1/mcp", "https://8.8.8.8/mcp"] {
            assert!(
                resolve_allowed_target(&Url::parse(allowed).expect("URL"))
                    .await
                    .is_ok(),
                "{allowed}"
            );
        }
        for blocked in [
            "http://8.8.8.8/mcp",
            "https://10.0.0.1/mcp",
            "https://[fe80::1]/mcp",
            "https://user:secret@example.com/mcp",
        ] {
            assert!(
                resolve_allowed_target(&Url::parse(blocked).expect("URL"))
                    .await
                    .is_err(),
                "{blocked}"
            );
        }
    }
}
