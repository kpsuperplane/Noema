//! Provider-neutral web search types.

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Stable id for Noema's default public DuckDuckGo provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ID: &str = "duckduckgo_public";
/// Reliability label for providers that do not have a formal API contract.
pub const BEST_EFFORT_PUBLIC_CONTRACT: &str = "best_effort_public";

/// Normalized search request after tool argument validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub reason: Option<String>,
    pub max_results: usize,
}

/// Normalized successful search response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchResponse {
    pub provider: String,
    pub provider_contract: String,
    pub query: String,
    pub results: Vec<SearchResult>,
    pub summary: String,
}

/// One normalized search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchResult {
    pub rank: usize,
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Safe error categories returned by the first-party search tool.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SearchError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error("search request timed out")]
    Timeout,
    #[error("search provider rate limited or blocked the request")]
    RateLimited,
    #[error("search provider request failed")]
    Http,
    #[error("search provider response could not be parsed")]
    Parse,
}

/// Runtime provider selected for the first-party `web.search` tool.
#[derive(Debug, Clone)]
pub(crate) enum SearchRuntimeProvider {
    DuckDuckGoPublic {
        client: Client,
    },
    #[cfg(test)]
    Static {
        response: SearchResponse,
    },
}

impl Default for SearchRuntimeProvider {
    fn default() -> Self {
        Self::DuckDuckGoPublic {
            client: Client::new(),
        }
    }
}

impl SearchRuntimeProvider {
    pub(crate) async fn search(
        &self,
        request: &SearchRequest,
    ) -> Result<SearchResponse, SearchError> {
        match self {
            Self::DuckDuckGoPublic { client } => {
                crate::search::duckduckgo::search_duckduckgo_public(client, request).await
            }
            #[cfg(test)]
            Self::Static { response } => {
                let mut response = response.clone();
                response.query = request.query.clone();
                response.results.truncate(request.max_results);
                response.summary = match response.results.len() {
                    0 => "No web results found".to_string(),
                    1 => "Found 1 web result".to_string(),
                    count => format!("Found {count} web results"),
                };
                Ok(response)
            }
        }
    }
}
