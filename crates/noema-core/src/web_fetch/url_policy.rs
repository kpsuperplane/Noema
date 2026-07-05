//! Public web-fetch URL policy.

use crate::web_fetch::types::FetchError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedUrl {
    pub url: Url,
    pub resolved_ips: Vec<IpAddr>,
}

pub(crate) async fn validate_public_web_fetch_url(raw_url: &str) -> Result<CheckedUrl, FetchError> {
    let url = Url::parse(raw_url).map_err(|_| FetchError::MalformedUrl)?;
    validate_public_web_fetch_url_parsed(url).await
}

pub(crate) async fn validate_public_web_fetch_url_parsed(
    url: Url,
) -> Result<CheckedUrl, FetchError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::UnsupportedScheme);
    }
    if let Some(host) = url.host()
        && let Some(ip) = ip_from_url_host(host)
    {
        if !is_public_ip(ip) {
            return Err(FetchError::BlockedTarget);
        }
        return Ok(CheckedUrl {
            url,
            resolved_ips: vec![ip],
        });
    }
    let host = url.host_str().ok_or(FetchError::MalformedUrl)?;
    if is_blocked_hostname(host) || is_alternate_ipv4_literal(host).is_some() {
        return Err(FetchError::BlockedTarget);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err(FetchError::BlockedTarget);
        }
        return Ok(CheckedUrl {
            url,
            resolved_ips: vec![ip],
        });
    }

    let lookup_host = host.to_string();
    let port = url
        .port_or_known_default()
        .ok_or(FetchError::MalformedUrl)?;
    let resolved_ips = tokio::task::spawn_blocking(move || {
        (lookup_host.as_str(), port)
            .to_socket_addrs()
            .map(|addrs| addrs.map(|addr| addr.ip()).collect::<Vec<_>>())
    })
    .await
    .map_err(|_| FetchError::Dns)?
    .map_err(|_| FetchError::Dns)?;

    if resolved_ips.is_empty() {
        return Err(FetchError::Dns);
    }
    if resolved_ips.iter().any(|ip| !is_public_ip(*ip)) {
        return Err(FetchError::BlockedTarget);
    }
    Ok(CheckedUrl { url, resolved_ips })
}

pub(crate) fn is_blocked_hostname(host: &str) -> bool {
    let normalized = host.trim_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".test")
        || normalized.ends_with(".invalid")
        || normalized.ends_with(".example")
}

pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
        || ip.octets()[0] >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || is_ipv6_documentation(ip))
}

fn is_ipv6_documentation(ip: Ipv6Addr) -> bool {
    ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8
}

fn is_alternate_ipv4_literal(host: &str) -> Option<Ipv4Addr> {
    let value = host.parse::<u32>().ok()?;
    Some(Ipv4Addr::from(value))
}

fn ip_from_url_host(host: Host<&str>) -> Option<IpAddr> {
    match host {
        Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        Host::Domain(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn accepts_ordinary_public_https_url() {
        let checked = validate_public_web_fetch_url("https://www.rust-lang.org/learn")
            .await
            .expect("public URL");

        assert_eq!(checked.url.scheme(), "https");
        assert_eq!(checked.url.host_str(), Some("www.rust-lang.org"));
    }

    #[tokio::test]
    async fn rejects_non_http_schemes() {
        let error = validate_public_web_fetch_url("file:///etc/passwd")
            .await
            .expect_err("scheme rejected");

        assert!(matches!(error, FetchError::UnsupportedScheme));
    }

    #[tokio::test]
    async fn rejects_localhost_name() {
        let error = validate_public_web_fetch_url("http://localhost:3737/graphql")
            .await
            .expect_err("localhost rejected");

        assert!(matches!(error, FetchError::BlockedTarget));
    }

    #[tokio::test]
    async fn rejects_ipv4_loopback_and_private_ranges() {
        for url in [
            "http://127.0.0.1/",
            "http://10.0.0.1/",
            "http://172.16.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/",
            "http://0.0.0.0/",
        ] {
            let error = validate_public_web_fetch_url(url).await.expect_err(url);
            assert!(matches!(error, FetchError::BlockedTarget), "{url}: {error}");
        }
    }

    #[tokio::test]
    async fn rejects_ipv6_local_ranges() {
        for url in [
            "http://[::1]/",
            "http://[fc00::1]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://[2001:db8::1]/",
        ] {
            let error = validate_public_web_fetch_url(url).await.expect_err(url);
            assert!(matches!(error, FetchError::BlockedTarget), "{url}: {error}");
        }
    }

    #[tokio::test]
    async fn rejects_integer_encoded_loopback_host() {
        let error = validate_public_web_fetch_url("http://2130706433/")
            .await
            .expect_err("alternate IPv4 rejected");

        assert!(matches!(error, FetchError::BlockedTarget | FetchError::Dns));
    }
}
