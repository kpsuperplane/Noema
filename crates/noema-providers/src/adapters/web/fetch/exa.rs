//! Exa hosted web fetch provider.

use super::super::{
    exa_transport::ExaWebClient,
    normalize::{raw_markdown_response, validate_fetch_url},
};
use crate::{WebFetchBackend, WebFetchContext, WebFetchError, WebOperationFuture};
use noema_capabilities::web::fetch::{FetchRequest, FetchResponse};
use serde::Serialize;
use serde_json::Value;
/// Stable provider identifier for Exa web fetch.
pub const EXA_FETCH_PROVIDER_ID: &str = "exa";
/// Extraction label for Exa's hosted contents API.
const EXA_EXTRACTION: &str = "exa_contents";
impl WebFetchBackend for ExaWebClient {
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
    client: &ExaWebClient,
    request: &FetchRequest,
) -> Result<FetchResponse, WebFetchError> {
    validate_fetch_url(&request.url)?;
    let response = client
        .post("/contents")
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
            reqwest::StatusCode::TOO_MANY_REQUESTS => WebFetchError::RateLimited,
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
    let final_url = result.get("url").and_then(Value::as_str);
    let title = result.get("title").and_then(Value::as_str);
    let raw_content = result
        .get("text")
        .and_then(Value::as_str)
        .ok_or(WebFetchError::Extraction)?;
    raw_markdown_response(
        (EXA_FETCH_PROVIDER_ID, EXA_EXTRACTION),
        requested_url,
        final_url,
        title,
        [],
        raw_content,
        max_chars,
    )
}
fn map_reqwest_error(error: reqwest::Error) -> WebFetchError {
    if error.is_timeout() {
        WebFetchError::Timeout
    } else {
        WebFetchError::Http
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::spawn_server;
    use noema_capabilities::web::fetch::FetchRequest;

    #[tokio::test]
    async fn sends_contents_request_with_api_key() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"Rust"}]}"#,
        )
        .await;
        let client = ExaWebClient::with_client(
            base_url,
            "secret".to_string().into(),
            reqwest::Client::new(),
        );

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
