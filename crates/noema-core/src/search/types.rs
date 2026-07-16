//! Provider-neutral web search types.

use noema_capabilities::web::search::{SearchRequest, SearchResponse};
use reqwest::Client;

/// Stable id for Noema's default public DuckDuckGo provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ID: &str = "duckduckgo_public";
/// Reliability label for providers that do not have a formal API contract.
pub const BEST_EFFORT_PUBLIC_CONTRACT: &str = "best_effort_public";

/// Safe error categories returned by the first-party search tool.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SearchError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error("search request timed out")]
    Timeout,
    #[error("search provider rate limited or blocked the request")]
    RateLimited,
    #[error("provider account unauthenticated")]
    AuthFailed,
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
    Exa {
        client: crate::search::exa::ExaSearchClient,
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
            Self::Exa { client } => crate::search::exa::search_exa(client, request).await,
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
