//! Strict outbound HTTP boundary for every OAuth discovery and token operation.

use std::{sync::Arc, time::Duration};

use http::Response;
use rmcp::transport::auth::{
    OAuthHttpClient, OAuthHttpClientError, OAuthHttpClientFuture, OAuthHttpRequest,
};
use url::Url;

use crate::{
    connection_url::resolve_allowed_target,
    http_body::{BodyReadError, bounded_response_body},
};

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
            let response = client
                .execute(request)
                .await
                .map_err(|_| oauth_error("OAuth HTTP request failed"))?;

            let mut result = Response::builder()
                .status(response.status())
                .version(response.version());
            for (name, value) in response.headers() {
                result = result.header(name, value);
            }
            let body = bounded_response_body(response, MAX_OAUTH_RESPONSE_BYTES)
                .await
                .map_err(|error| match error {
                    BodyReadError::Request(_) => oauth_error("OAuth HTTP response failed"),
                    BodyReadError::Limit => {
                        oauth_error("OAuth HTTP response exceeded the supported limit")
                    }
                })?;
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
    let resolution = resolve_allowed_target(url)
        .await
        .map_err(|_| oauth_error("OAuth request target is not public"))?;
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

fn oauth_error(message: &'static str) -> OAuthHttpClientError {
    OAuthHttpClientError::new(message)
}
