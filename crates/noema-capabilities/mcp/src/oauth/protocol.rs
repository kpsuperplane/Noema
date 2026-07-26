//! rmcp OAuth runtime and protected-resource metadata probing.

use std::{fmt, time::Duration};

use rmcp::{model::ProtocolVersion, transport::auth::OAuthState};
use serde_json::Value;
use url::Url;

use super::{
    McpOAuthBackend, McpOAuthError, McpOAuthErrorKind, McpOAuthResult, McpOAuthRuntime,
    McpOAuthStarted, OAuthFuture,
    http_client::{strict_oauth_http_client, strict_reqwest_client},
};
use crate::http_body::bounded_response_body;
use crate::{McpOAuthStoredCredentials, secrets::now_epoch_seconds};

pub(super) struct RmcpBackend;

impl McpOAuthBackend for RmcpBackend {
    fn start<'a>(&'a self, url: &'a str, redirect: &'a str) -> OAuthFuture<'a, McpOAuthStarted> {
        Box::pin(async move {
            let (state, authorization_url) = start_authorization(url, redirect).await?;
            Ok(McpOAuthStarted {
                authorization_url,
                runtime: Box::new(RmcpRuntime { state }),
            })
        })
    }
}

struct RmcpRuntime {
    state: OAuthState,
}

impl McpOAuthRuntime for RmcpRuntime {
    fn complete<'a>(
        &'a mut self,
        callback_url: &'a str,
    ) -> OAuthFuture<'a, McpOAuthStoredCredentials> {
        Box::pin(async move {
            self.state
                .handle_callback_url(callback_url)
                .await
                .map_err(auth_error)?;
            stored_credentials(&self.state).await
        })
    }
}

pub(super) async fn stored_credentials(
    state: &OAuthState,
) -> McpOAuthResult<McpOAuthStoredCredentials> {
    let (client_id, response) = state.get_credentials().await.map_err(unavailable_error)?;
    let response = response.ok_or_else(|| {
        McpOAuthError::new(
            McpOAuthErrorKind::Unavailable,
            "OAuth returned no token response",
        )
    })?;
    Ok(McpOAuthStoredCredentials {
        client_id,
        token_response: serde_json::to_value(response).map_err(unavailable_error)?,
        token_received_at: Some(now_epoch_seconds()),
    })
}

async fn start_authorization(
    endpoint: &str,
    redirect: &str,
) -> McpOAuthResult<(OAuthState, String)> {
    let primary = resolved_resource(endpoint)
        .await
        .unwrap_or_else(|| endpoint.to_string());
    match start_at(&primary, redirect).await {
        Ok(result) => Ok(result),
        Err(first) => {
            let Some(origin) = resource_origin(endpoint) else {
                return Err(first);
            };
            if resource_matches(&primary, origin.as_str()) {
                return Err(first);
            }
            start_at(origin.as_str(), redirect).await.map_err(|second| {
                McpOAuthError::new(
                    second.kind,
                    format!(
                        "primary failed: {}; origin failed: {}",
                        first.diagnostic_detail, second.diagnostic_detail
                    ),
                )
            })
        }
    }
}

async fn start_at(resource: &str, redirect: &str) -> McpOAuthResult<(OAuthState, String)> {
    let mut state = OAuthState::new_with_oauth_http_client(resource, strict_oauth_http_client())
        .await
        .map_err(unavailable_error)?;
    state
        .start_authorization(&[], redirect, Some("Noema"))
        .await
        .map_err(auth_error)?;
    let url = state.get_authorization_url().await.map_err(auth_error)?;
    Ok((state, url))
}

pub(crate) async fn resolved_resource(endpoint: &str) -> Option<String> {
    let endpoint = Url::parse(endpoint).ok()?;
    for url in metadata_candidates(&endpoint) {
        let Some(metadata) = fetch_metadata(&url).await else {
            continue;
        };
        if let Some(resource) = metadata_resource(&endpoint, &metadata) {
            return Some(resource);
        }
    }
    None
}

pub(super) async fn fetch_metadata(url: &Url) -> Option<Value> {
    let client = strict_reqwest_client(url, Duration::from_secs(10))
        .await
        .ok()?;
    let response = client
        .get(url.clone())
        .header("MCP-Protocol-Version", ProtocolVersion::LATEST.as_str())
        .send()
        .await
        .ok()?;
    if response.status().is_success() {
        return bounded_json_response(response).await;
    }
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return None;
    }
    let metadata_url = response
        .headers()
        .get_all(reqwest::header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| challenge_metadata_url(value, url))?;
    let client = strict_reqwest_client(&metadata_url, Duration::from_secs(10))
        .await
        .ok()?;
    let response = client
        .get(metadata_url)
        .header("MCP-Protocol-Version", ProtocolVersion::LATEST.as_str())
        .send()
        .await
        .ok()?;
    bounded_json_response(response).await
}

async fn bounded_json_response(response: reqwest::Response) -> Option<Value> {
    const MAX_METADATA_BYTES: usize = 1024 * 1024;
    let body = bounded_response_body(response, MAX_METADATA_BYTES)
        .await
        .ok()?;
    serde_json::from_slice(&body).ok()
}

pub(super) fn metadata_candidates(endpoint: &Url) -> Vec<Url> {
    let mut urls = Vec::new();
    let path = endpoint.path().trim_matches('/');
    if !path.is_empty() {
        urls.push(with_path(
            endpoint,
            &format!("/.well-known/oauth-protected-resource/{path}"),
        ));
    }
    urls.push(with_path(endpoint, "/.well-known/oauth-protected-resource"));
    urls
}

fn with_path(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    url.set_query(None);
    url.set_fragment(None);
    url.set_path(path);
    url
}

pub(super) fn metadata_resource(endpoint: &Url, metadata: &Value) -> Option<String> {
    let resource = metadata.get("resource")?.as_str()?;
    let has_server = metadata
        .get("authorization_server")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
        || metadata
            .get("authorization_servers")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str().is_some()));
    if !has_server {
        return None;
    }
    if resource_matches(endpoint.as_str(), resource) {
        return Some(resource.to_string());
    }
    let origin = resource_origin(endpoint.as_str())?;
    resource_matches(origin.as_str(), resource).then(|| resource.to_string())
}

pub(super) fn challenge_metadata_url(header: &str, base: &Url) -> Option<Url> {
    let prefix = "resource_metadata=\"";
    let start = header.find(prefix)? + prefix.len();
    let end = header[start..].find('"')? + start;
    let url = Url::parse(&header[start..end])
        .or_else(|_| base.join(&header[start..end]))
        .ok()?;
    (same_origin(base, &url)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host() == right.host()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn resource_matches(expected: &str, actual: &str) -> bool {
    expected == actual
        || (is_root(expected) && actual == expected.trim_end_matches('/'))
        || (is_root(actual) && expected == actual.trim_end_matches('/'))
}

fn is_root(value: &str) -> bool {
    Url::parse(value)
        .is_ok_and(|url| url.path() == "/" && url.query().is_none() && url.fragment().is_none())
}

fn resource_origin(endpoint: &str) -> Option<Url> {
    let mut url = Url::parse(endpoint).ok()?;
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

pub(super) fn auth_error(error: impl fmt::Display) -> McpOAuthError {
    McpOAuthError::new(McpOAuthErrorKind::Authentication, error.to_string())
}

pub(super) fn unavailable_error(error: impl fmt::Display) -> McpOAuthError {
    McpOAuthError::new(McpOAuthErrorKind::Unavailable, error.to_string())
}
