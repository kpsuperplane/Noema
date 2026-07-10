//! Canonical loopback authority validation for the local web server.

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

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

pub(super) fn parse_loopback_ip(host: &str) -> Result<IpAddr, String> {
    let address = IpAddr::from_str(host).map_err(|_| "web host must be a numeric IP address")?;
    if !address.is_loopback() {
        return Err("web host must be a loopback IP address".to_string());
    }
    Ok(address)
}

pub(super) async fn enforce_authority(
    State(authority): State<CanonicalAuthority>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let headers = request.headers();
    let valid_host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host == authority.as_str());
    if !valid_host {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let needs_origin = (request.method() == Method::POST && request.uri().path() == "/graphql")
        || (request.method() == Method::GET && request.uri().path() == "/graphql/ws");
    if needs_origin {
        let valid_origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| origin == authority.origin());
        if !valid_origin {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_loopback_bind_hosts() {
        assert!(parse_loopback_ip("localhost").is_err());
        assert!(parse_loopback_ip("0.0.0.0").is_err());
        assert_eq!(
            parse_loopback_ip("127.0.0.1").expect("numeric loopback"),
            "127.0.0.1".parse::<std::net::IpAddr>().expect("IP")
        );
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
