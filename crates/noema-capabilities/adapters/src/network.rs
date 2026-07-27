//! Hardened credentialed JSON transport for reviewed fixed origins.

use crate::{HttpMethod, RetryPolicy, request::EncodedAdapterRequest};
use noema_capabilities::web::url_policy::{is_public_ip, validate_public_url};
use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use std::{future::Future, net::SocketAddr, pin::Pin, time::Duration};
use thiserror::Error;
use url::{Host, Url};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DNS_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_HEADER_BYTES: usize = 64 * 1024;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_NODES: usize = 16_384;
const MAX_JSON_COLLECTION: usize = 1_024;
const MAX_JSON_STRING_BYTES: usize = 256 * 1024;

pub(crate) type AdapterHttpFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AdapterHttpOutcome, AdapterHttpError>> + Send + 'a>>;

pub(crate) trait AdapterHttpExecutor: Send + Sync {
    fn execute(
        &self,
        method: HttpMethod,
        retry: RetryPolicy,
        request: EncodedAdapterRequest,
        credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_>;
}

#[derive(Clone)]
pub(crate) struct AdapterBearerCredential(String);

impl AdapterBearerCredential {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    #[cfg(test)]
    pub(crate) fn into_secret_for_tests(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for AdapterBearerCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AdapterBearerCredential([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AdapterHttpOutcome {
    Success(Value),
    Rejected(u16),
    AuthenticationRequired,
    Denied,
    RateLimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum AdapterHttpError {
    #[error("adapter target is unavailable")]
    Unavailable,
    #[error("adapter response is invalid")]
    InvalidResponse,
}

#[derive(Debug, Default)]
pub(crate) struct ReqwestAdapterHttpExecutor;

impl AdapterHttpExecutor for ReqwestAdapterHttpExecutor {
    fn execute(
        &self,
        method: HttpMethod,
        retry: RetryPolicy,
        request: EncodedAdapterRequest,
        credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_> {
        Box::pin(async move { execute(method, retry, request, credential).await })
    }
}

async fn execute(
    method: HttpMethod,
    retry: RetryPolicy,
    request: EncodedAdapterRequest,
    credential: Option<AdapterBearerCredential>,
) -> Result<AdapterHttpOutcome, AdapterHttpError> {
    let attempts = if retry == RetryPolicy::TransportSafeRead && method == HttpMethod::Get {
        2
    } else {
        1
    };
    let mut last_error = None;
    for _ in 0..attempts {
        let outcome = match checked_client(&request.url).await {
            Ok(client) => send_once(&client, method, &request, credential.as_ref()).await,
            Err(error) => Err(error),
        };
        match outcome {
            Ok(outcome) => return Ok(outcome),
            Err(AdapterHttpError::Unavailable) => last_error = Some(AdapterHttpError::Unavailable),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or(AdapterHttpError::Unavailable))
}

async fn checked_client(url: &Url) -> Result<Client, AdapterHttpError> {
    let checked = validate_public_url(url.as_str()).map_err(|_| AdapterHttpError::Unavailable)?;
    if checked.scheme() != "https" {
        return Err(AdapterHttpError::Unavailable);
    }
    let port = checked
        .port_or_known_default()
        .ok_or(AdapterHttpError::Unavailable)?;
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    if let Host::Domain(hostname) = checked.host().ok_or(AdapterHttpError::Unavailable)? {
        let addresses =
            tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((hostname, port)))
                .await
                .map_err(|_| AdapterHttpError::Unavailable)?
                .map_err(|_| AdapterHttpError::Unavailable)?
                .collect::<Vec<_>>();
        validate_addresses(&addresses)?;
        builder = builder.resolve_to_addrs(hostname, &addresses);
    }
    builder.build().map_err(|_| AdapterHttpError::Unavailable)
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), AdapterHttpError> {
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(AdapterHttpError::Unavailable);
    }
    Ok(())
}

async fn send_once(
    client: &Client,
    method: HttpMethod,
    request: &EncodedAdapterRequest,
    credential: Option<&AdapterBearerCredential>,
) -> Result<AdapterHttpOutcome, AdapterHttpError> {
    let mut builder = client
        .request(reqwest_method(method), request.url.clone())
        .header(header::ACCEPT_ENCODING, "identity");
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if let Some(credential) = credential {
        let mut value = header::HeaderValue::from_str(&format!("Bearer {}", credential.0))
            .map_err(|_| AdapterHttpError::Unavailable)?;
        value.set_sensitive(true);
        builder = builder.header(header::AUTHORIZATION, value);
    }
    if let Some(body) = &request.body {
        builder = builder.json(body);
    }
    let response = builder
        .send()
        .await
        .map_err(|_| AdapterHttpError::Unavailable)?;
    if response
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.as_bytes().len())
        .sum::<usize>()
        > MAX_RESPONSE_HEADER_BYTES
        || response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    match response.status() {
        StatusCode::UNAUTHORIZED => Ok(AdapterHttpOutcome::AuthenticationRequired),
        StatusCode::FORBIDDEN => Ok(AdapterHttpOutcome::Denied),
        StatusCode::TOO_MANY_REQUESTS => Ok(AdapterHttpOutcome::RateLimited),
        status if status.is_redirection() => Err(AdapterHttpError::InvalidResponse),
        status if !status.is_success() => Ok(AdapterHttpOutcome::Rejected(status.as_u16())),
        status => {
            if status == StatusCode::NO_CONTENT {
                return Ok(AdapterHttpOutcome::Success(Value::Null));
            }
            validate_json_content_type(&response)?;
            let bytes = bounded_body(response).await?;
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|_| AdapterHttpError::InvalidResponse)?;
            validate_json_shape(&value)?;
            Ok(AdapterHttpOutcome::Success(value))
        }
    }
}

fn validate_json_content_type(response: &reqwest::Response) -> Result<(), AdapterHttpError> {
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if content_type != "application/json" && !content_type.ends_with("+json") {
        return Err(AdapterHttpError::InvalidResponse);
    }
    Ok(())
}

async fn bounded_body(mut response: reqwest::Response) -> Result<Vec<u8>, AdapterHttpError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AdapterHttpError::Unavailable)?
    {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
            return Err(AdapterHttpError::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn validate_json_shape(value: &Value) -> Result<(), AdapterHttpError> {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), AdapterHttpError> {
        *nodes += 1;
        if depth > MAX_JSON_DEPTH || *nodes > MAX_JSON_NODES {
            return Err(AdapterHttpError::InvalidResponse);
        }
        match value {
            Value::String(value) if value.len() > MAX_JSON_STRING_BYTES => {
                Err(AdapterHttpError::InvalidResponse)
            }
            Value::Array(values) => {
                if values.len() > MAX_JSON_COLLECTION {
                    return Err(AdapterHttpError::InvalidResponse);
                }
                for value in values {
                    visit(value, depth + 1, nodes)?;
                }
                Ok(())
            }
            Value::Object(values) => {
                if values.len() > MAX_JSON_COLLECTION {
                    return Err(AdapterHttpError::InvalidResponse);
                }
                for (key, value) in values {
                    if key.len() > MAX_JSON_STRING_BYTES {
                        return Err(AdapterHttpError::InvalidResponse);
                    }
                    visit(value, depth + 1, nodes)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    visit(value, 0, &mut 0)
}

const fn reqwest_method(method: HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::IpAddr;

    #[test]
    fn resolved_targets_reject_empty_mixed_and_non_public_answers() {
        let public = SocketAddr::new(IpAddr::V4("8.8.8.8".parse().expect("ip")), 443);
        let private = SocketAddr::new(IpAddr::V4("127.0.0.1".parse().expect("ip")), 443);
        assert!(validate_addresses(&[public]).is_ok());
        assert_eq!(validate_addresses(&[]), Err(AdapterHttpError::Unavailable));
        assert_eq!(
            validate_addresses(&[public, private]),
            Err(AdapterHttpError::Unavailable)
        );
    }

    #[test]
    fn bounded_json_shape_rejects_large_collections_and_depth() {
        assert!(validate_json_shape(&json!({"items": [1, 2, 3]})).is_ok());
        assert_eq!(
            validate_json_shape(&Value::Array(vec![Value::Null; MAX_JSON_COLLECTION + 1])),
            Err(AdapterHttpError::InvalidResponse)
        );
        let mut deep = Value::Null;
        for _ in 0..=MAX_JSON_DEPTH {
            deep = Value::Array(vec![deep]);
        }
        assert_eq!(
            validate_json_shape(&deep),
            Err(AdapterHttpError::InvalidResponse)
        );
    }
}
