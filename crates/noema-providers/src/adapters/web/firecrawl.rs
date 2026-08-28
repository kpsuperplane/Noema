//! Firecrawl hosted search and fetch provider.

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
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
/// Stable provider identifier for Firecrawl web services.
pub const FIRECRAWL_PROVIDER_ID: &str = "firecrawl";
const API_BASE_URL: &str = "https://api.firecrawl.dev/v2";
const HTTP_TIMEOUT: Duration = Duration::from_secs(75);
const PROVIDER_TIMEOUT_MS: u64 = 60_000;
const HOSTED_CONTRACT: &str = "hosted_provider";
/// Firecrawl client for authenticated or credential-free access.
#[derive(Clone, Debug)]
pub struct FirecrawlWebClient {
    base_url: String,
    credential: Option<ProviderCredential>,
    http: Client,
}
impl FirecrawlWebClient {
    /// Build a Firecrawl client with an optional credential.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be configured.
    pub fn new(credential: Option<ProviderCredential>) -> Result<Self, reqwest::Error> {
        Ok(Self {
            base_url: API_BASE_URL.to_string(),
            credential,
            http: Client::builder().timeout(HTTP_TIMEOUT).build()?,
        })
    }
    fn post(&self, path: &str) -> RequestBuilder {
        let request = self
            .http
            .post(format!("{}{path}", self.base_url.trim_end_matches('/')));
        match &self.credential {
            Some(credential) => request.bearer_auth(credential.expose_secret()),
            None => request,
        }
    }
}
impl WebSearchBackend for FirecrawlWebClient {
    fn backend_id(&self) -> &str {
        FIRECRAWL_PROVIDER_ID
    }
    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> WebOperationFuture<'a, SearchResponse, WebSearchError> {
        Box::pin(search_firecrawl(self, request))
    }
}
async fn search_firecrawl(
    client: &FirecrawlWebClient,
    request: &SearchRequest,
) -> Result<SearchResponse, WebSearchError> {
    let response = client
        .post("/search")
        .json(&FirecrawlSearchRequest {
            query: &request.query,
            limit: request.max_results,
            sources: ["web"],
            timeout: PROVIDER_TIMEOUT_MS,
        })
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                WebSearchError::Timeout
            } else {
                WebSearchError::Http
            }
        })?;
    map_search_status(response.status(), client.credential.is_some())?;
    let response: FirecrawlResponse<FirecrawlSearchData> =
        response.json().await.map_err(|_| WebSearchError::Parse)?;
    if !response.success {
        return Err(WebSearchError::Parse);
    }
    let results = response.data.ok_or(WebSearchError::Parse)?.web;
    Ok(search_response(
        FIRECRAWL_PROVIDER_ID,
        HOSTED_CONTRACT,
        &request.query,
        request.max_results,
        results.into_iter().map(|result| SearchCandidate {
            title: result.title.unwrap_or_default(),
            url: result.url,
            snippet: result.description.unwrap_or_default(),
        }),
    ))
}
impl WebFetchBackend for FirecrawlWebClient {
    fn backend_id(&self) -> &str {
        FIRECRAWL_PROVIDER_ID
    }
    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
        Box::pin(fetch_firecrawl(self, request))
    }
}
async fn fetch_firecrawl(
    client: &FirecrawlWebClient,
    request: &FetchRequest,
) -> Result<FetchResponse, WebFetchError> {
    validate_fetch_url(&request.url)?;
    let response = client
        .post("/scrape")
        .json(&FirecrawlScrapeRequest {
            url: &request.url,
            formats: ["markdown", "links"],
            only_main_content: true,
            skip_tls_verification: false,
            max_age: 0,
            timeout: PROVIDER_TIMEOUT_MS,
        })
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                WebFetchError::Timeout
            } else {
                WebFetchError::Http
            }
        })?;
    map_fetch_status(response.status(), client.credential.is_some())?;
    let response: FirecrawlResponse<FirecrawlScrapeData> = response
        .json()
        .await
        .map_err(|_| WebFetchError::Extraction)?;
    if !response.success {
        return Err(WebFetchError::Extraction);
    }
    let data = response.data.ok_or(WebFetchError::Extraction)?;
    let final_url = data
        .metadata
        .url
        .as_deref()
        .or(data.metadata.source_url.as_deref());
    raw_markdown_response(
        (FIRECRAWL_PROVIDER_ID, "firecrawl_markdown"),
        &request.url,
        final_url,
        data.metadata.title.as_deref(),
        data.links,
        &data.markdown,
        request.max_chars,
    )
}
#[derive(Serialize)]
struct FirecrawlSearchRequest<'a> {
    query: &'a str,
    limit: usize,
    sources: [&'static str; 1],
    timeout: u64,
}
#[derive(Deserialize)]
struct FirecrawlResponse<T> {
    success: bool,
    data: Option<T>,
}
#[derive(Deserialize)]
struct FirecrawlSearchData {
    #[serde(default)]
    web: Vec<FirecrawlSearchResult>,
}
#[derive(Deserialize)]
struct FirecrawlSearchResult {
    url: String,
    title: Option<String>,
    description: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlScrapeRequest<'a> {
    url: &'a str,
    formats: [&'static str; 2],
    only_main_content: bool,
    skip_tls_verification: bool,
    max_age: u8,
    timeout: u64,
}
#[derive(Deserialize)]
struct FirecrawlScrapeData {
    markdown: String,
    #[serde(default)]
    links: Vec<String>,
    metadata: FirecrawlMetadata,
}
#[derive(Deserialize)]
struct FirecrawlMetadata {
    title: Option<String>,
    #[serde(rename = "sourceURL")]
    source_url: Option<String>,
    url: Option<String>,
}
fn map_search_status(status: StatusCode, authenticated: bool) -> Result<(), WebSearchError> {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN if authenticated => {
            Err(WebSearchError::AuthFailed)
        }
        StatusCode::TOO_MANY_REQUESTS => Err(WebSearchError::RateLimited),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => Err(WebSearchError::Timeout),
        status if !status.is_success() => Err(WebSearchError::Http),
        _ => Ok(()),
    }
}
fn map_fetch_status(status: StatusCode, authenticated: bool) -> Result<(), WebFetchError> {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN if authenticated => {
            Err(WebFetchError::AuthFailed)
        }
        StatusCode::TOO_MANY_REQUESTS => Err(WebFetchError::RateLimited),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => Err(WebFetchError::Timeout),
        status if !status.is_success() => Err(WebFetchError::Http),
        _ => Ok(()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::spawn_scripted_server;

    #[tokio::test]
    async fn authenticated_and_keyless_requests_follow_firecrawl_protocol() {
        let (base, requests) = spawn_scripted_server([
            (200, r#"{"success":true,"data":{"web":[{"title":"Rust","description":"Language","url":"https://www.rust-lang.org/"}]}}"#),
            (200, r##"{"success":true,"data":{"markdown":"# Rust","links":["https://doc.rust-lang.org/"],"metadata":{"title":"Rust","sourceURL":"https://www.rust-lang.org/","url":"https://www.rust-lang.org/learn"}}}"##),
            (200, r#"{"success":true,"data":{"web":[]}}"#),
        ]).await;
        let authenticated = FirecrawlWebClient {
            base_url: base.clone(),
            credential: Some("fire-secret".to_string().into()),
            http: Client::new(),
        };
        let keyless = FirecrawlWebClient {
            base_url: base,
            credential: None,
            http: Client::new(),
        };
        let search_request = SearchRequest {
            query: "rust".to_string(),
            reason: None,
            max_results: 3,
        };
        let search = search_firecrawl(&authenticated, &search_request)
            .await
            .expect("search");
        let fetch = fetch_firecrawl(
            &authenticated,
            &FetchRequest {
                url: "https://www.rust-lang.org/".to_string(),
                reason: None,
                max_chars: 1_000,
            },
        )
        .await
        .expect("fetch");
        search_firecrawl(&keyless, &search_request)
            .await
            .expect("keyless search");
        let requests = requests.await.expect("requests");
        assert!(requests.iter().all(|request| request.method == "POST"));
        assert!(requests[..2].iter().all(|request| {
            request.headers.get("authorization").map(String::as_str) == Some("Bearer fire-secret")
        }));
        assert!(!requests[2].headers.contains_key("authorization"));
        assert!(
            requests[0].body.contains(r#""sources":["web"]"#)
                && !requests[0].body.contains("scrapeOptions")
        );
        assert!(
            requests[1]
                .body
                .contains(r#""formats":["markdown","links"]"#)
                && requests[1].body.contains(r#""onlyMainContent":true"#)
        );
        assert_eq!(search.results[0].rank, 1);
        assert_eq!(fetch.final_url, "https://www.rust-lang.org/learn");
        assert_eq!(HTTP_TIMEOUT, Duration::from_secs(75));
    }

    #[test]
    fn status_mapping_keeps_keyless_failures_credential_free() {
        assert_eq!(
            map_search_status(StatusCode::UNAUTHORIZED, true),
            Err(WebSearchError::AuthFailed)
        );
        assert_eq!(
            map_search_status(StatusCode::UNAUTHORIZED, false),
            Err(WebSearchError::Http)
        );
        assert_eq!(
            map_search_status(StatusCode::REQUEST_TIMEOUT, false),
            Err(WebSearchError::Timeout)
        );
        assert_eq!(
            map_search_status(StatusCode::TOO_MANY_REQUESTS, false),
            Err(WebSearchError::RateLimited)
        );
        assert_eq!(
            map_fetch_status(StatusCode::FORBIDDEN, false),
            Err(WebFetchError::Http)
        );
        assert_eq!(
            map_fetch_status(StatusCode::TOO_MANY_REQUESTS, true),
            Err(WebFetchError::RateLimited)
        );
        assert_eq!(
            map_fetch_status(StatusCode::GATEWAY_TIMEOUT, true),
            Err(WebFetchError::Timeout)
        );
    }
}
