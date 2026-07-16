//! Exa hosted web fetch provider.

use super::super::exa_transport::{EXA_API_BASE_URL, production_http_client};
use crate::{WebFetchBackend, WebFetchContext, WebFetchError, WebOperationFuture};
use noema_capabilities::web::fetch::{
    FetchContentKind, FetchRequest, FetchResponse, FetchSummaryStrategy, sanitized_display_url,
};
use noema_capabilities::web::url_policy::PublicUrlError;
use serde::Serialize;
use serde_json::Value;
use std::fmt;

/// Stable provider identifier for Exa web fetch.
pub const EXA_FETCH_PROVIDER_ID: &str = "exa";
/// Extraction label for Exa's hosted contents API.
pub const EXA_EXTRACTION: &str = "exa_contents";

/// Exa hosted contents client.
#[derive(Clone)]
pub struct ExaFetchClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl ExaFetchClient {
    /// Build a production Exa fetch client.
    ///
    /// # Errors
    ///
    /// Returns [`WebFetchError::Http`] when the configured HTTP client cannot
    /// be built.
    pub fn new(api_key: String) -> Result<Self, WebFetchError> {
        let http = production_http_client().map_err(|_| WebFetchError::Http)?;
        Ok(Self::with_client(
            EXA_API_BASE_URL.to_string(),
            api_key,
            http,
        ))
    }

    /// Build an Exa fetch client with an injected endpoint and HTTP transport.
    #[must_use]
    pub fn with_client(base_url: String, api_key: String, http: reqwest::Client) -> Self {
        Self {
            base_url,
            api_key,
            http,
        }
    }
}

impl fmt::Debug for ExaFetchClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExaFetchClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("http", &"[CONFIGURED]")
            .finish()
    }
}

impl WebFetchBackend for ExaFetchClient {
    fn backend_id(&self) -> &str {
        EXA_FETCH_PROVIDER_ID
    }

    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
        Box::pin(fetch_exa(self, request))
    }
}

#[derive(Debug, Serialize)]
struct ExaContentsRequest<'a> {
    urls: [&'a str; 1],
    text: bool,
}

async fn fetch_exa(
    client: &ExaFetchClient,
    request: &FetchRequest,
) -> Result<FetchResponse, WebFetchError> {
    noema_capabilities::web::url_policy::validate_public_url(&request.url)
        .map_err(map_public_url_error)?;
    let response = client
        .http
        .post(format!(
            "{}/contents",
            client.base_url.trim_end_matches('/')
        ))
        .header("x-api-key", &client.api_key)
        .json(&ExaContentsRequest {
            urls: [&request.url],
            text: true,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    if !response.status().is_success() {
        return Err(match response.status() {
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                WebFetchError::AuthFailed
            }
            reqwest::StatusCode::REQUEST_TIMEOUT | reqwest::StatusCode::GATEWAY_TIMEOUT => {
                WebFetchError::Timeout
            }
            _ => WebFetchError::Http,
        });
    }
    let value: Value = response
        .json()
        .await
        .map_err(|_| WebFetchError::Extraction)?;
    normalize_exa_contents_response(&request.url, request.max_chars, &value)
}

fn normalize_exa_contents_response(
    requested_url: &str,
    max_chars: usize,
    value: &Value,
) -> Result<FetchResponse, WebFetchError> {
    let result = value
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or(WebFetchError::Extraction)?;
    let final_url = result
        .get("url")
        .and_then(Value::as_str)
        .map(sanitized_display_url)
        .unwrap_or_else(|| sanitized_display_url(requested_url));
    let title = result
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let raw_content = result
        .get("text")
        .and_then(Value::as_str)
        .ok_or(WebFetchError::Extraction)?;
    let raw_chars = raw_content.chars().count();
    let content = raw_content.chars().take(max_chars).collect::<String>();
    let returned_chars = content.chars().count();

    Ok(FetchResponse {
        provider: EXA_FETCH_PROVIDER_ID.to_string(),
        url: sanitized_display_url(requested_url),
        final_url,
        title,
        format: "markdown".to_string(),
        extraction: EXA_EXTRACTION.to_string(),
        content_kind: FetchContentKind::RawMarkdown,
        content,
        raw_excerpt: None,
        raw_chars,
        returned_chars,
        summary_model: None,
        summary_strategy: FetchSummaryStrategy::NotSummarized,
        truncated: raw_chars > returned_chars,
    })
}

fn map_reqwest_error(error: reqwest::Error) -> WebFetchError {
    if error.is_timeout() {
        WebFetchError::Timeout
    } else {
        WebFetchError::Http
    }
}

fn map_public_url_error(error: PublicUrlError) -> WebFetchError {
    match error {
        PublicUrlError::UnsupportedScheme => WebFetchError::UnsupportedScheme,
        PublicUrlError::Malformed => WebFetchError::MalformedUrl,
        PublicUrlError::BlockedTarget => WebFetchError::BlockedTarget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::spawn_server;
    use noema_capabilities::web::fetch::FetchRequest;
    use serde_json::json;

    #[test]
    fn normalizes_exa_contents_response() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "text": "# Rust\nFast and reliable."
                }
            ]
        });

        let response = normalize_exa_contents_response("https://www.rust-lang.org/", 30, &value)
            .expect("fetch");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.url, "https://www.rust-lang.org/");
        assert_eq!(response.final_url, "https://www.rust-lang.org/");
        assert_eq!(response.title.as_deref(), Some("Rust"));
        assert_eq!(response.content, "# Rust\nFast and reliable.");
        assert_eq!(response.extraction, "exa_contents");
        assert!(!response.truncated);
    }

    #[test]
    fn normalizer_redacts_sensitive_remote_final_url() {
        let value = json!({
            "results": [{
                "url": "https://user:secret@example.com/private#token",
                "text": "safe"
            }]
        });
        let response =
            normalize_exa_contents_response("https://example.com", 30, &value).expect("fetch");
        assert_eq!(
            response.final_url,
            noema_capabilities::web::fetch::REDACTED_SENSITIVE_URL
        );
    }

    #[test]
    fn exa_fetch_debug_redacts_api_key() {
        let client = ExaFetchClient::with_client(
            "https://example.test".to_string(),
            "exa-fetch-secret".to_string(),
            reqwest::Client::new(),
        );
        let debug = format!("{client:?}");

        assert!(!debug.contains("exa-fetch-secret"));
        assert!(debug.contains("[REDACTED]"));

        let runtime = crate::WebFetchBackendHandle::new(client);
        assert_eq!(
            format!("{runtime:?}"),
            "WebFetchBackendHandle(\"[CONFIGURED]\")"
        );
    }

    #[test]
    fn production_constructor_owns_exa_endpoint_policy() {
        let client = ExaFetchClient::new("secret".to_string()).expect("client");

        assert_eq!(client.base_url, EXA_API_BASE_URL);
    }

    #[tokio::test]
    async fn sends_contents_request_with_api_key() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"Rust"}]}"#,
        )
        .await;
        let client =
            ExaFetchClient::with_client(base_url, "secret".to_string(), reqwest::Client::new());

        let response = fetch_exa(
            &client,
            &FetchRequest {
                url: "https://noema-remote-resolution-check-404.com/".to_string(),
                reason: None,
                max_chars: 8_000,
            },
        )
        .await
        .expect("fetch");

        let request = request_rx.await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/contents");
        assert_eq!(
            request.headers.get("x-api-key").map(String::as_str),
            Some("secret")
        );
        assert!(
            request
                .body
                .contains("\"urls\":[\"https://noema-remote-resolution-check-404.com/\"]",)
        );
        assert!(request.body.contains("\"text\":true"));
        assert_eq!(response.content, "Rust");
    }
}
