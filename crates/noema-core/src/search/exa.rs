//! Exa hosted web search provider.

use crate::search::types::SearchError;
use noema_capabilities::web::search::{SearchRequest, SearchResponse, SearchResult};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

pub(crate) const EXA_SEARCH_PROVIDER_ID: &str = "exa";
pub(crate) const EXA_SEARCH_CONTRACT: &str = "hosted_provider";

#[derive(Clone)]
pub struct ExaSearchClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

impl fmt::Debug for ExaSearchClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExaSearchClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("http", &"[CONFIGURED]")
            .finish()
    }
}

impl noema_providers::WebSearchBackend for ExaSearchClient {
    fn backend_id(&self) -> &str {
        EXA_SEARCH_PROVIDER_ID
    }

    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> noema_providers::WebOperationFuture<'a, SearchResponse, SearchError> {
        Box::pin(search_exa(self, request))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequest<'a> {
    query: &'a str,
    num_results: usize,
}

pub(crate) async fn search_exa(
    client: &ExaSearchClient,
    request: &SearchRequest,
) -> Result<SearchResponse, SearchError> {
    let response = client
        .http
        .post(format!("{}/search", client.base_url.trim_end_matches('/')))
        .header("x-api-key", &client.api_key)
        .json(&ExaSearchRequest {
            query: &request.query,
            num_results: request.max_results,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            return Err(SearchError::AuthFailed);
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => return Err(SearchError::RateLimited),
        status if !status.is_success() => return Err(SearchError::Http),
        _ => {}
    }
    let value: Value = response.json().await.map_err(|_| SearchError::Parse)?;
    normalize_exa_search_response(&request.query, request.max_results, &value)
}

pub(crate) fn normalize_exa_search_response(
    query: &str,
    max_results: usize,
    value: &Value,
) -> Result<SearchResponse, SearchError> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or(SearchError::Parse)?
        .iter()
        .filter_map(exa_result)
        .take(max_results)
        .enumerate()
        .map(|(index, (title, url, snippet))| SearchResult {
            rank: index + 1,
            title,
            url,
            snippet,
        })
        .collect::<Vec<_>>();
    let summary = match results.len() {
        0 => "No web results found".to_string(),
        1 => "Found 1 Exa web result".to_string(),
        count => format!("Found {count} Exa web results"),
    };
    Ok(SearchResponse {
        provider: EXA_SEARCH_PROVIDER_ID.to_string(),
        provider_contract: EXA_SEARCH_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn exa_result(value: &Value) -> Option<(String, String, String)> {
    let url = value.get("url")?.as_str()?.trim().to_string();
    if url.is_empty() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&url)
        .to_string();
    let snippet = value
        .get("highlights")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(Value::as_str))
        .or_else(|| value.get("summary").and_then(Value::as_str))
        .or_else(|| value.get("text").and_then(Value::as_str))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    Some((title, url, snippet))
}

fn map_reqwest_error(error: reqwest::Error) -> SearchError {
    if error.is_timeout() {
        SearchError::Timeout
    } else {
        SearchError::Http
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::adapters::test_support::spawn_server;
    use noema_capabilities::web::search::SearchRequest;
    use serde_json::json;

    #[test]
    fn normalizes_exa_search_results() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "highlights": ["A language empowering everyone"],
                    "text": "Full text",
                    "summary": "Summary"
                },
                {
                    "title": "Cargo",
                    "url": "https://doc.rust-lang.org/cargo/",
                    "text": "Cargo is Rust's package manager."
                }
            ]
        });

        let response = normalize_exa_search_response("rust", 1, &value).expect("response");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.provider_contract, "hosted_provider");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].rank, 1);
        assert_eq!(response.results[0].title, "Rust");
        assert_eq!(
            response.results[0].snippet,
            "A language empowering everyone"
        );
    }

    #[test]
    fn exa_search_debug_redacts_api_key() {
        let client = ExaSearchClient {
            base_url: "https://example.test".to_string(),
            api_key: "exa-search-secret".to_string(),
            http: reqwest::Client::new(),
        };
        let debug = format!("{client:?}");

        assert!(!debug.contains("exa-search-secret"));
        assert!(debug.contains("[REDACTED]"));

        let runtime = noema_providers::WebSearchBackendHandle::new(client);
        assert_eq!(
            format!("{runtime:?}"),
            "WebSearchBackendHandle(\"[CONFIGURED]\")"
        );
    }

    #[tokio::test]
    async fn sends_search_request_with_api_key() {
        let (base_url, request_rx) = spawn_server(200, r#"{"results":[]}"#).await;
        let client = ExaSearchClient {
            base_url,
            api_key: "secret".to_string(),
            http: reqwest::Client::new(),
        };

        let response = search_exa(
            &client,
            &SearchRequest {
                query: "rust".to_string(),
                reason: None,
                max_results: 3,
            },
        )
        .await
        .expect("search");

        let request = request_rx.await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/search");
        assert_eq!(
            request.headers.get("x-api-key").map(String::as_str),
            Some("secret")
        );
        assert!(request.body.contains("\"query\":\"rust\""));
        assert!(request.body.contains("\"numResults\":3"));
        assert!(response.results.is_empty());
    }
}
