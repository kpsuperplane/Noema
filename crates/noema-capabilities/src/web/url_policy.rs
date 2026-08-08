//! Pure public URL policy shared by web-fetch provider implementations.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use thiserror::Error;
use url::{Host, Url};

/// Pure URL-policy rejection.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PublicUrlError {
    /// Only HTTP and HTTPS are supported.
    #[error("unsupported URL scheme")]
    UnsupportedScheme,
    /// URL could not be parsed or lacks a usable host/port.
    #[error("malformed URL")]
    Malformed,
    /// URL names a private, internal, local, or sensitive target.
    #[error("blocked private or local target")]
    BlockedTarget,
}

/// Parse and apply scheme, credential, host, and literal-IP policy.
/// DNS resolution, socket pinning, and redirect enforcement belong to the
/// concrete HTTP provider.
///
/// # Errors
///
/// Returns [`PublicUrlError`] when pure public-target policy rejects the URL.
pub fn validate_public_url(raw_url: &str) -> Result<Url, PublicUrlError> {
    let url = Url::parse(raw_url).map_err(|_| PublicUrlError::Malformed)?;
    validate_parsed_public_url(url)
}

/// Normalize one structured URL for exact observed-URL matching.
///
/// Fragments are discarded because they are not sent in an HTTP request.
/// Credentials, unsupported schemes, and non-public literal targets remain
/// rejected by the normal public URL policy.
///
/// # Errors
///
/// Returns [`PublicUrlError`] when the value is malformed or unsafe.
pub fn normalize_observed_url(raw_url: &str) -> Result<String, PublicUrlError> {
    let mut url = Url::parse(raw_url.trim()).map_err(|_| PublicUrlError::Malformed)?;
    url.set_fragment(None);
    validate_parsed_public_url(url).map(|url| url.to_string())
}

/// Apply pure policy to a parsed URL.
///
/// # Errors
///
/// Returns [`PublicUrlError`] when pure public-target policy rejects the URL.
pub fn validate_parsed_public_url(url: Url) -> Result<Url, PublicUrlError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PublicUrlError::UnsupportedScheme);
    }
    if url_has_credentials(&url) {
        return Err(PublicUrlError::BlockedTarget);
    }
    url.port_or_known_default()
        .ok_or(PublicUrlError::Malformed)?;
    if match url.host().ok_or(PublicUrlError::Malformed)? {
        Host::Ipv4(ip) => !is_public_ip(IpAddr::V4(ip)),
        Host::Ipv6(ip) => !is_public_ip(IpAddr::V6(ip)),
        Host::Domain(domain) => {
            is_blocked_hostname(domain)
                || is_alternate_ipv4_literal(domain).is_some_and(|ip| !is_public_ip(IpAddr::V4(ip)))
        }
    } {
        return Err(PublicUrlError::BlockedTarget);
    }
    Ok(url)
}

/// Return whether a URL contains username/password credentials.
#[must_use]
pub fn url_has_credentials(url: &Url) -> bool {
    !url.username().is_empty() || url.password().is_some()
}

/// Return whether a hostname is reserved for local/internal use.
#[must_use]
fn is_blocked_hostname(host: &str) -> bool {
    let normalized = host.trim_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".test")
        || normalized.ends_with(".invalid")
        || normalized.ends_with(".example")
}

/// Return whether an IP address is globally routable under Noema's fetch policy.
#[must_use]
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || a == 0
        || a == 100 && (64..=127).contains(&b)
        || a == 192 && b == 0
        || a == 192 && b == 88 && c == 99
        || a == 198 && matches!(b, 18 | 19)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(ipv4);
    }
    let segments = ip.segments();
    if segments[0] & 0xe000 != 0x2000
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || is_ipv6_site_local(ip)
        || is_ipv6_nat64(ip)
        || segments[0] == 0x2002
        || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
        || (segments[0] == 0x3fff && segments[1] & 0xf000 == 0)
        || is_ipv6_documentation(ip)
    {
        return false;
    }
    if let Some(ipv4) = ip.to_ipv4() {
        return is_public_ipv4(ipv4);
    }
    true
}

fn is_ipv6_site_local(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xffc0 == 0xfec0
}

fn is_ipv6_nat64(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    segments[0] == 0x0064
        && segments[1] == 0xff9b
        && (segments[2] == 0x0001 || segments[2..6].iter().all(|segment| *segment == 0))
}

fn is_ipv6_documentation(ip: Ipv6Addr) -> bool {
    ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8
}

fn is_alternate_ipv4_literal(host: &str) -> Option<Ipv4Addr> {
    host.parse::<u32>().ok().map(Ipv4Addr::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_public_url_without_resolving_dns() {
        let url = validate_public_url("https://www.rust-lang.org/learn").expect("public URL");
        assert_eq!(url.host_str(), Some("www.rust-lang.org"));
        assert_eq!(
            normalize_observed_url(" HTTPS://Example.COM:443/a/../b?q=1#section ")
                .expect("normalize URL"),
            "https://example.com/b?q=1"
        );
        assert!(normalize_observed_url("https://user:secret@example.com/").is_err());
        assert!(is_public_ip(
            "2606:4700:4700::1111".parse().expect("public IPv6")
        ));
        assert!(is_public_ip(
            "3fff:1000::1".parse().expect("neighboring public IPv6")
        ));
    }

    #[test]
    fn rejects_private_literals_reserved_hosts_and_credentials() {
        for url in [
            "file:///etc/passwd",
            "http://localhost/",
            "http://127.0.0.1/",
            "http://10.0.0.1/",
            "http://172.16.0.1/",
            "http://192.168.0.1/",
            "http://[::1]/",
            "http://[fc00::1]/",
            "http://[fe80::1]/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://017700000001/",
            "http://127.1/",
            "http://192.88.99.1/",
            "http://[fec0::1]/",
            "http://[64:ff9b::7f00:1]/",
            "http://[64:ff9b:1::7f00:1]/",
            "http://[2002:7f00:1::]/",
            "http://[100::1]/",
            "http://[2001::1]/",
            "http://[3fff::1]/",
            "http://[4000::1]/",
            "https://user:secret@example.com/",
        ] {
            assert!(validate_public_url(url).is_err(), "expected blocked: {url}");
        }
        assert_eq!(
            validate_public_url("https://example.com/path#section")
                .expect("ordinary fragment")
                .fragment(),
            Some("section")
        );
    }
}
