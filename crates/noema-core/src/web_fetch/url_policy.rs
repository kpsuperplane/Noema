//! Public web-fetch URL policy.

use crate::web_fetch::types::FetchError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUrl {
    pub url: Url,
    pub resolved_addrs: Vec<SocketAddr>,
}

pub async fn validate_public_web_fetch_url(raw_url: &str) -> Result<CheckedUrl, FetchError> {
    let url = Url::parse(raw_url).map_err(|_| FetchError::MalformedUrl)?;
    validate_public_web_fetch_url_parsed(url).await
}

pub async fn validate_public_web_fetch_url_parsed(url: Url) -> Result<CheckedUrl, FetchError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::UnsupportedScheme);
    }
    if url_has_sensitive_components(&url) {
        return Err(FetchError::BlockedTarget);
    }
    let port = url
        .port_or_known_default()
        .ok_or(FetchError::MalformedUrl)?;
    if let Some(host) = url.host()
        && let Some(ip) = ip_from_url_host(host)
    {
        if !is_public_ip(ip) {
            return Err(FetchError::BlockedTarget);
        }
        return Ok(CheckedUrl {
            url,
            resolved_addrs: vec![SocketAddr::new(ip, port)],
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

#[must_use]
pub fn url_has_sensitive_components(url: &Url) -> bool {
    !url.username().is_empty() || url.password().is_some() || url.fragment().is_some()
}

#[must_use]
pub fn is_blocked_hostname(host: &str) -> bool {
    let normalized = host.trim_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".test")
        || normalized.ends_with(".invalid")
        || normalized.ends_with(".example")
}

#[must_use]
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || a == 0
        || a == 100 && (64..=127).contains(&b)
        || a == 192 && b == 0
        || a == 198 && matches!(b, 18 | 19)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(ipv4);
    }
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || is_ipv6_documentation(ip)
    {
        return false;
    }
    if let Some(ipv4) = ip.to_ipv4() {
        return is_public_ipv4(ipv4);
    }
    true
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
        assert!(!checked.resolved_addrs.is_empty());
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
            "http://0.1.2.3/",
            "http://198.18.0.1/",
            "http://198.19.255.255/",
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
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:169.254.169.254]/",
            "http://[::ffff:10.0.0.1]/",
            "http://[ff02::1]/",
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

    #[tokio::test]
    async fn rejects_credentials_and_fragments() {
        for url in [
            "https://user:secret@example.com/",
            "https://example.com/path#secret",
        ] {
            let error = validate_public_web_fetch_url(url).await.expect_err(url);
            assert!(matches!(error, FetchError::BlockedTarget), "{url}: {error}");
        }
    }
}
