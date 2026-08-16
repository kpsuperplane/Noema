//! Canonical loopback authority validation for the local web server.

use std::{net::IpAddr, str::FromStr};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// The exact browser-visible origin and authority serving this process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalAuthority {
    authority: String,
    origin: String,
    rp_id: String,
    secure: bool,
}

impl CanonicalAuthority {
    pub(crate) fn from_web_config(
        config: &noema_host::WebConfig,
        listener_port: u16,
    ) -> Result<Self, String> {
        let origin = config
            .public_origin
            .clone()
            .unwrap_or_else(|| format!("http://localhost:{listener_port}"));
        Self::from_public_origin(&origin, &config.rp_id)
    }

    pub(crate) fn from_public_origin(origin: &str, rp_id: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(origin).map_err(|_| "web public_origin must be a URL")?;
        let host = parsed
            .host_str()
            .ok_or("web public_origin must include a host")?;
        let secure = match parsed.scheme() {
            "https" => true,
            "http" if host == "localhost" => false,
            _ => return Err("web public_origin must use HTTPS, except for localhost".to_string()),
        };
        if parsed.username() != ""
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(
                "web public_origin must contain only a scheme, host, and optional port".to_string(),
            );
        }
        if host != "localhost" && IpAddr::from_str(host).is_ok() {
            return Err("web public_origin must use a domain name for passkeys".to_string());
        }
        if rp_id.trim() != rp_id
            || rp_id.is_empty()
            || rp_id.contains(['/', ':'])
            || (rp_id != "localhost" && IpAddr::from_str(rp_id).is_ok())
        {
            return Err("web rp_id must be a domain name without a scheme or port".to_string());
        }
        let authority = parsed[url::Position::BeforeHost..url::Position::AfterPort].to_string();
        Ok(Self {
            authority,
            origin: parsed.origin().ascii_serialization(),
            rp_id: rp_id.to_string(),
            secure,
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.authority
    }

    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    pub(super) fn rp_id(&self) -> &str {
        &self.rp_id
    }

    pub(crate) const fn secure(&self) -> bool {
        self.secure
    }
}

pub(super) fn parse_bind_ip(host: &str) -> Result<IpAddr, String> {
    IpAddr::from_str(host).map_err(|_| "web host must be a numeric IP address".to_string())
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

    let oauth_approval =
        request.method() == Method::POST && request.uri().path() == "/oauth/authorize";
    let needs_origin = (request.method() == Method::POST
        && (request.uri().path() == "/graphql"
            || request.uri().path().starts_with("/auth/")
            || oauth_approval))
        || (request.method() == Method::GET && request.uri().path() == "/graphql/ws");
    let has_authorization = headers.get(header::AUTHORIZATION).is_some();
    let authorization_replaces_origin =
        has_authorization && matches!(request.uri().path(), "/graphql" | "/graphql/ws");
    if needs_origin && !authorization_replaces_origin {
        let valid_origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| origin == authority.origin());
        let opaque_oauth_origin = oauth_approval
            && headers
                .get(header::ORIGIN)
                .is_none_or(|origin| origin == "null");
        if !valid_origin && !opaque_oauth_origin {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::Request, middleware};
    use tower::ServiceExt as _;

    #[test]
    fn bind_ip_requires_an_explicit_numeric_address() {
        assert!(parse_bind_ip("localhost").is_err());
        assert_eq!(
            parse_bind_ip("127.0.0.1").expect("numeric loopback"),
            "127.0.0.1".parse::<std::net::IpAddr>().expect("IP")
        );

        assert_eq!(
            parse_bind_ip("0.0.0.0").expect("explicit wildcard IP"),
            "0.0.0.0".parse::<IpAddr>().expect("IP")
        );
    }

    #[test]
    fn public_origin_is_exact_https_domain_or_localhost() {
        let deployed = CanonicalAuthority::from_public_origin(
            "https://login.noema.example:8443",
            "noema.example",
        )
        .expect("deployed origin");
        assert_eq!(deployed.as_str(), "login.noema.example:8443");
        assert_eq!(deployed.origin(), "https://login.noema.example:8443");
        assert_eq!(deployed.rp_id(), "noema.example");
        assert!(deployed.secure());

        let local = CanonicalAuthority::from_public_origin("http://localhost:3737", "localhost")
            .expect("localhost origin");
        assert!(!local.secure());
        for invalid in [
            "http://noema.example",
            "https://127.0.0.1",
            "https://noema.example/path",
            "https://user@noema.example",
        ] {
            assert!(
                CanonicalAuthority::from_public_origin(invalid, "noema.example").is_err(),
                "{invalid}"
            );
        }
        assert!(
            CanonicalAuthority::from_public_origin(
                "https://noema.example",
                "https://noema.example"
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn only_oauth_approval_allows_an_opaque_origin() {
        let authority =
            CanonicalAuthority::from_public_origin("https://noema.example", "noema.example")
                .expect("authority");
        let router = Router::new()
            .fallback(|| async { StatusCode::NO_CONTENT })
            .layer(middleware::from_fn_with_state(authority, enforce_authority));

        for (path, origin, expected) in [
            ("/oauth/authorize", None, StatusCode::NO_CONTENT),
            (
                "/oauth/authorize",
                Some("https://noema.example"),
                StatusCode::NO_CONTENT,
            ),
            ("/oauth/authorize", Some("null"), StatusCode::NO_CONTENT),
            (
                "/oauth/authorize",
                Some("https://attacker.example"),
                StatusCode::FORBIDDEN,
            ),
            ("/auth/logout", None, StatusCode::FORBIDDEN),
            ("/auth/logout", Some("null"), StatusCode::FORBIDDEN),
        ] {
            let mut request = Request::post(path)
                .header(header::HOST, "noema.example")
                .body(Body::empty())
                .expect("request");
            if let Some(origin) = origin {
                request
                    .headers_mut()
                    .insert(header::ORIGIN, origin.parse().expect("origin header"));
            }
            assert_eq!(
                router
                    .clone()
                    .oneshot(request)
                    .await
                    .expect("response")
                    .status(),
                expected
            );
        }
    }
}
