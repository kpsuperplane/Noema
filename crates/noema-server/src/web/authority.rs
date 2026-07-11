//! Canonical loopback authority validation for the local web server.

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

#[cfg(all(feature = "dev-no-auth", debug_assertions))]
use std::net::Ipv4Addr;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// The exact authority of the listener serving this process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalAuthority(String);

impl CanonicalAuthority {
    pub(crate) fn from_socket_addr(address: SocketAddr) -> Self {
        Self(address.to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn origin(&self) -> String {
        format!("http://{}", self.0)
    }
}

pub(super) fn parse_bind_ip(host: &str) -> Result<IpAddr, String> {
    let address = IpAddr::from_str(host).map_err(|_| "web host must be a numeric IP address")?;
    if address.is_loopback() || dev_allows_unspecified_bind(address) {
        return Ok(address);
    }
    Err("web host must be a loopback IP address".to_string())
}

#[cfg(all(feature = "dev-no-auth", debug_assertions))]
fn dev_allows_unspecified_bind(address: IpAddr) -> bool {
    address == IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

#[cfg(not(all(feature = "dev-no-auth", debug_assertions)))]
const fn dev_allows_unspecified_bind(_address: IpAddr) -> bool {
    false
}

pub(super) async fn enforce_authority(
    State(authority): State<CanonicalAuthority>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let dev_permissive = dev_permissive_authority();
    let headers = request.headers();
    let valid_host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host == authority.as_str());
    if !valid_host && !dev_permissive {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let needs_origin = (request.method() == Method::POST && request.uri().path() == "/graphql")
        || (request.method() == Method::GET && request.uri().path() == "/graphql/ws");
    if needs_origin {
        let valid_origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| origin == authority.origin());
        if !valid_origin && !dev_permissive {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

#[cfg(all(feature = "dev-no-auth", debug_assertions))]
const fn dev_permissive_authority() -> bool {
    true
}

#[cfg(not(all(feature = "dev-no-auth", debug_assertions)))]
const fn dev_permissive_authority() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_loopback_bind_hosts() {
        assert!(parse_bind_ip("localhost").is_err());
        assert_eq!(
            parse_bind_ip("127.0.0.1").expect("numeric loopback"),
            "127.0.0.1".parse::<std::net::IpAddr>().expect("IP")
        );
    }

    #[test]
    fn bind_ip_allows_wildcard_only_for_dev_feature() {
        assert_eq!(
            parse_bind_ip("127.0.0.1").expect("loopback IP"),
            "127.0.0.1".parse::<std::net::IpAddr>().expect("IP")
        );

        #[cfg(all(feature = "dev-no-auth", debug_assertions))]
        assert_eq!(
            parse_bind_ip("0.0.0.0").expect("development wildcard IP"),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );

        #[cfg(not(all(feature = "dev-no-auth", debug_assertions)))]
        assert!(parse_bind_ip("0.0.0.0").is_err());
    }

    #[test]
    fn formats_ipv4_and_ipv6_authorities() {
        assert_eq!(
            CanonicalAuthority::from_socket_addr("127.0.0.1:3737".parse().expect("socket"))
                .as_str(),
            "127.0.0.1:3737"
        );
        assert_eq!(
            CanonicalAuthority::from_socket_addr("[::1]:3737".parse().expect("socket")).as_str(),
            "[::1]:3737"
        );
    }
}
