//! TinyFish hosted search and fetch provider.

use super::normalize::{
    SearchCandidate, raw_markdown_response, search_response, validate_fetch_url,
};
use crate::{
    ProviderCredential, WebFetchBackend, WebFetchContext, WebFetchError, WebOperationFuture,
    WebSearchBackend, WebSearchError,
};
use noema_capabilities::web::{
    fetch::{FetchRequest, FetchResponse},
    search::{SearchRequest, SearchResponse},
};
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;
/// Stable provider identifier for TinyFish web services.
pub const TINYFISH_PROVIDER_ID: &str = "tinyfish";
const SEARCH_ENDPOINT: &str = "https://api.search.tinyfish.ai";
const FETCH_ENDPOINT: &str = "https://api.fetch.tinyfish.ai";
const SEARCH_TIMEOUT: Duration = Duration::from_secs(10);
const FETCH_TIMEOUT: Duration = Duration::from_secs(150);
const HOSTED_CONTRACT: &str = "hosted_provider";
/// TinyFish client for one provider-owned credential.
#[derive(Clone, Debug)]
pub struct TinyFishWebClient {
    search_endpoint: String,
    fetch_endpoint: String,
    credential: ProviderCredential,
    http: Client,
}
impl TinyFishWebClient {
    /// Build a TinyFish client for one credential.
    #[must_use]
    pub fn new(credential: ProviderCredential) -> Self {
        Self {
            search_endpoint: SEARCH_ENDPOINT.to_string(),
            fetch_endpoint: FETCH_ENDPOINT.to_string(),
            credential,
            http: Client::new(),
        }
    }
    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        request.header("x-api-key", self.credential.expose_secret())
    }
}
impl WebSearchBackend for TinyFishWebClient {
    fn backend_id(&self) -> &str {
        TINYFISH_PROVIDER_ID
    }
    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> WebOperationFuture<'a, SearchResponse, WebSearchError> {
        Box::pin(async move {
            let response = self
                .authenticated(self.http.get(&self.search_endpoint))
                .query(&[
                    ("query", Some(request.query.as_str())),
                    ("purpose", request.reason.as_deref()),
                ])
                .timeout(SEARCH_TIMEOUT)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        WebSearchError::Timeout
                    } else {
                        WebSearchError::Http
                    }
                })?;
            match response.status() {
                reqwest::StatusCode::UNAUTHORIZED => return Err(WebSearchError::AuthFailed),
                reqwest::StatusCode::TOO_MANY_REQUESTS => return Err(WebSearchError::RateLimited),
                status if !status.is_success() => return Err(WebSearchError::Http),
                _ => {}
            }
            let response: TinyFishSearchResponse =
                response.json().await.map_err(|_| WebSearchError::Parse)?;
            Ok(search_response(
                TINYFISH_PROVIDER_ID,
                HOSTED_CONTRACT,
                &request.query,
                request.max_results,
                response.results.into_iter().map(|result| SearchCandidate {
                    title: result.title,
                    url: result.url,
                    snippet: result.snippet,
                }),
            ))
        })
    }
}
impl WebFetchBackend for TinyFishWebClient {
    fn backend_id(&self) -> &str {
        TINYFISH_PROVIDER_ID
    }
    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
        Box::pin(fetch_tinyfish(self, request))
    }
}
async fn fetch_tinyfish(
    client: &TinyFishWebClient,
    request: &FetchRequest,
) -> Result<FetchResponse, WebFetchError> {
    validate_fetch_url(&request.url)?;
    let response = client
        .authenticated(client.http.post(&client.fetch_endpoint))
        .json(&TinyFishFetchRequest {
            urls: [&request.url],
            purpose: request.reason.as_deref(),
            format: "markdown",
            links: true,
            ttl: 0,
        })
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                WebFetchError::Timeout
            } else {
                WebFetchError::Http
            }
        })?;
    map_fetch_status(response.status())?;
    let response: TinyFishFetchResponse = response
        .json()
        .await
        .map_err(|_| WebFetchError::Extraction)?;
    if let Some(result) = response
        .results
        .into_iter()
        .find(|item| item.url == request.url)
    {
        return raw_markdown_response(
            (TINYFISH_PROVIDER_ID, "tinyfish_markdown"),
            &request.url,
            Some(&result.final_url),
            result.title.as_deref(),
            result.links,
            &result.text,
            request.max_chars,
        );
    }
    match response
        .errors
        .into_iter()
        .find(|item| item.url == request.url)
    {
        Some(error) if error.error == "timeout" => Err(WebFetchError::Timeout),
        _ => Err(WebFetchError::Extraction),
    }
}
#[derive(Deserialize)]
struct TinyFishSearchResponse {
    results: Vec<TinyFishSearchResult>,
}
#[derive(Deserialize)]
struct TinyFishSearchResult {
    title: String,
    snippet: String,
    url: String,
}
#[derive(Serialize)]
struct TinyFishFetchRequest<'a> {
    urls: [&'a str; 1],
    #[serde(skip_serializing_if = "Option::is_none")]
    purpose: Option<&'a str>,
    format: &'static str,
    links: bool,
    ttl: u8,
}
#[derive(Deserialize)]
struct TinyFishFetchResponse {
    results: Vec<TinyFishFetchResult>,
    errors: Vec<TinyFishFetchError>,
}
#[derive(Deserialize)]
struct TinyFishFetchResult {
    url: String,
    final_url: String,
    title: Option<String>,
    text: String,
    #[serde(default)]
    links: Vec<String>,
}
#[derive(Deserialize)]
struct TinyFishFetchError {
    url: String,
    error: String,
}
fn map_fetch_status(status: reqwest::StatusCode) -> Result<(), WebFetchError> {
    match status {
        reqwest::StatusCode::UNAUTHORIZED => Err(WebFetchError::AuthFailed),
        reqwest::StatusCode::TOO_MANY_REQUESTS => Err(WebFetchError::RateLimited),
        reqwest::StatusCode::REQUEST_TIMEOUT | reqwest::StatusCode::GATEWAY_TIMEOUT => {
            Err(WebFetchError::Timeout)
        }
        status if !status.is_success() => Err(WebFetchError::Http),
        _ => Ok(()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::spawn_scripted_server;

    #[tokio::test]
    async fn search_and_fetch_follow_tinyfish_protocol() {
        let (base, requests) = spawn_scripted_server([
            (200, r#"{"results":[{"title":" Rust ","snippet":"safe  result","url":"https://www.rust-lang.org/"}]}"#),
            (200, r##"{"results":[{"url":"https://www.rust-lang.org/","final_url":"https://www.rust-lang.org/learn","title":"Rust","text":"# Rust","links":["https://doc.rust-lang.org/"]}],"errors":[]}"##),
            (200, r#"{"results":[],"errors":[{"url":"https://www.rust-lang.org/","error":"timeout"}]}"#),
            (429, r#"{}"#),
        ]).await;
        let client = TinyFishWebClient {
            search_endpoint: base.clone(),
            fetch_endpoint: base,
            credential: "tiny-secret".to_string().into(),
            http: Client::new(),
        };
        let search_request = SearchRequest {
            query: "rust".to_string(),
            reason: Some("learn".to_string()),
            max_results: 2,
        };
        let search = client.search(&search_request).await.expect("search");
        let fetch = fetch_tinyfish(
            &client,
            &FetchRequest {
                url: "https://www.rust-lang.org/".to_string(),
                reason: Some("learn".to_string()),
                max_chars: 1_000,
            },
        )
        .await
        .expect("fetch");
        assert_eq!(
            fetch_tinyfish(
                &client,
                &FetchRequest {
                    url: "https://www.rust-lang.org/".to_string(),
                    reason: None,
                    max_chars: 1_000,
                },
            )
            .await,
            Err(WebFetchError::Timeout)
        );
        assert!(matches!(
            client.search(&search_request).await,
            Err(WebSearchError::RateLimited)
        ));
        let requests = requests.await.expect("requests");
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[1].method, "POST");
        assert!(
            requests[0].path.contains("query=rust") && requests[0].path.contains("purpose=learn")
        );
        assert!(requests[..2].iter().all(|request| {
            request.headers.get("x-api-key").map(String::as_str) == Some("tiny-secret")
        }));
        assert!(
            requests[1].body.contains(r#""format":"markdown""#)
                && requests[1].body.contains(r#""links":true"#)
                && requests[1].body.contains(r#""ttl":0"#)
        );
        assert_eq!(search.results[0].rank, 1);
        assert_eq!(fetch.final_url, "https://www.rust-lang.org/learn");
        assert_eq!(
            map_fetch_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            Err(WebFetchError::RateLimited)
        );
        assert_eq!(
            (SEARCH_TIMEOUT, FETCH_TIMEOUT),
            (Duration::from_secs(10), Duration::from_secs(150))
        );
    }
}
