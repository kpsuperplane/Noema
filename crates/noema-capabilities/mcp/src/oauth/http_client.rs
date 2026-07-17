//! Strict outbound HTTP boundary for every OAuth discovery and token operation.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use http::Response;
use rmcp::transport::auth::{
    OAuthHttpClient, OAuthHttpClientError, OAuthHttpClientFuture, OAuthHttpRequest,
};
use url::{Host, Url};

const MAX_OAUTH_RESPONSE_BYTES: usize = 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn strict_oauth_http_client() -> Arc<dyn OAuthHttpClient> {
    Arc::new(StrictOAuthHttpClient)
}

struct StrictOAuthHttpClient;

impl OAuthHttpClient for StrictOAuthHttpClient {
    fn execute(&self, operation: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(async move {
            let timeout = operation
                .timeout
                .unwrap_or(DEFAULT_TIMEOUT)
                .min(DEFAULT_TIMEOUT);
            let url = Url::parse(&operation.request.uri().to_string())
                .map_err(|_| oauth_error("OAuth request URL is invalid"))?;
            if operation.request.body().len() > MAX_OAUTH_RESPONSE_BYTES {
                return Err(oauth_error(
                    "OAuth request body exceeded the supported limit",
                ));
            }
            let client = strict_reqwest_client(&url, timeout).await?;
            let request = reqwest::Request::try_from(operation.request)
                .map_err(|_| oauth_error("OAuth HTTP request is invalid"))?;
            let mut response = client
                .execute(request)
                .await
                .map_err(|_| oauth_error("OAuth HTTP request failed"))?;

            let mut result = Response::builder()
                .status(response.status())
                .version(response.version());
            for (name, value) in response.headers() {
                result = result.header(name, value);
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| oauth_error("OAuth HTTP response failed"))?
            {
                if chunk.len() > MAX_OAUTH_RESPONSE_BYTES.saturating_sub(body.len()) {
                    return Err(oauth_error(
                        "OAuth HTTP response exceeded the supported limit",
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            result
                .body(body)
                .map_err(|_| oauth_error("OAuth HTTP response was invalid"))
        })
    }
}

pub(super) async fn strict_reqwest_client(
    url: &Url,
    timeout: Duration,
) -> Result<reqwest::Client, OAuthHttpClientError> {
    let resolution = resolve_allowed_target(url).await?;
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout);
    if let Some((hostname, addresses)) = resolution {
        builder = builder.resolve_to_addrs(&hostname, &addresses);
    }
    builder
        .build()
        .map_err(|_| oauth_error("OAuth HTTP client is unavailable"))
}

async fn resolve_allowed_target(
    url: &Url,
) -> Result<Option<(String, Vec<SocketAddr>)>, OAuthHttpClientError> {
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(oauth_error("OAuth request URL is invalid"));
    }
    let host = url
        .host()
        .ok_or_else(|| oauth_error("OAuth request URL has no host"))?;
    let explicit_loopback = is_explicit_loopback(host.clone());
    if url.scheme() != "https" && !(url.scheme() == "http" && explicit_loopback) {
        return Err(oauth_error(
            "OAuth requests require HTTPS except on loopback",
        ));
    }

    match host {
        Host::Ipv4(address) => {
            validate_address(IpAddr::V4(address), explicit_loopback)?;
            Ok(None)
        }
        Host::Ipv6(address) => {
            validate_address(IpAddr::V6(address), explicit_loopback)?;
            Ok(None)
        }
        Host::Domain(hostname) => {
            let port = url
                .port_or_known_default()
                .ok_or_else(|| oauth_error("OAuth request URL has no port"))?;
            let addresses = tokio::net::lookup_host((hostname, port))
                .await
                .map_err(|_| oauth_error("OAuth host resolution failed"))?
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return Err(oauth_error("OAuth host resolution returned no addresses"));
            }
            for address in &addresses {
                validate_address(address.ip(), explicit_loopback)?;
            }
            Ok(Some((hostname.to_string(), addresses)))
        }
    }
}

fn is_explicit_loopback(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

fn validate_address(address: IpAddr, explicit_loopback: bool) -> Result<(), OAuthHttpClientError> {
    let allowed = match address {
        IpAddr::V4(address) if explicit_loopback => address.is_loopback(),
        IpAddr::V6(address) if explicit_loopback => address.is_loopback(),
        IpAddr::V4(address) => !disallowed_ipv4(address),
        IpAddr::V6(address) => !disallowed_ipv6(address),
    };
    allowed
        .then_some(())
        .ok_or_else(|| oauth_error("OAuth target resolved to a disallowed address"))
}

fn disallowed_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_unspecified()
        || address.is_multicast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 198 && matches!(octets[1], 18 | 19))
}

fn disallowed_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return disallowed_ipv4(mapped);
    }
    let segments = address.segments();
    address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xfe00) == 0xfc00
}

fn oauth_error(message: &'static str) -> OAuthHttpClientError {
    OAuthHttpClientError::new(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn oauth_targets_require_https_or_explicit_loopback() {
        assert!(
            resolve_allowed_target(&Url::parse("http://127.0.0.1/token").expect("url"))
                .await
                .is_ok()
        );
        assert!(
            resolve_allowed_target(&Url::parse("http://example.com/token").expect("url"))
                .await
                .is_err()
        );
        assert!(
            resolve_allowed_target(&Url::parse("https://10.0.0.1/token").expect("url"))
                .await
                .is_err()
        );
        assert!(
            resolve_allowed_target(&Url::parse("https://[fe80::1]/token").expect("url"))
                .await
                .is_err()
        );
        assert!(validate_address(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), true).is_err());
    }
}
