use std::{fmt, sync::Arc};

use noema_capabilities::web::search::{SearchRequest, SearchResponse};

use super::WebOperationFuture;

/// Stable id for Noema's default public DuckDuckGo provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ID: &str = "duckduckgo_public";
/// Reliability label for providers that do not have a formal API contract.
pub const BEST_EFFORT_PUBLIC_CONTRACT: &str = "best_effort_public";

/// Safe failures returned by provider-owned web search backends.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebSearchError {
    /// The request timed out.
    #[error("search request timed out")]
    Timeout,
    /// The provider rate-limited or blocked the request.
    #[error("search provider rate limited or blocked the request")]
    RateLimited,
    /// The provider account is not authenticated.
    #[error("provider account unauthenticated")]
    AuthFailed,
    /// The provider transport failed.
    #[error("search provider request failed")]
    Http,
    /// The provider response could not be parsed.
    #[error("search provider response could not be parsed")]
    Parse,
}

/// Object-safe provider backend for the first-party `web.search` capability.
pub trait WebSearchBackend: Send + Sync {
    /// Execute one validated search request.
    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> WebOperationFuture<'a, SearchResponse, WebSearchError>;
}

/// Clonable, redacted handle to one web search backend.
#[derive(Clone)]
pub struct WebSearchBackendHandle(Arc<dyn WebSearchBackend>);

impl WebSearchBackendHandle {
    /// Erase one concrete web search backend behind a shared handle.
    #[must_use]
    pub fn new(backend: impl WebSearchBackend + 'static) -> Self {
        Self(Arc::new(backend))
    }

    /// Erase an existing shared web search backend.
    #[must_use]
    pub fn from_arc(backend: Arc<dyn WebSearchBackend>) -> Self {
        Self(backend)
    }

    /// Execute one validated search request.
    ///
    /// # Errors
    ///
    /// Returns a typed provider backend failure.
    pub async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, WebSearchError> {
        self.0.search(request).await
    }
}

impl fmt::Debug for WebSearchBackendHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WebSearchBackendHandle")
            .field(&"[CONFIGURED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoSearch;

    impl WebSearchBackend for EchoSearch {
        fn search<'a>(
            &'a self,
            request: &'a SearchRequest,
        ) -> WebOperationFuture<'a, SearchResponse, WebSearchError> {
            Box::pin(async move {
                Ok(SearchResponse {
                    provider: "echo".to_string(),
                    query: request.query.clone(),
                    results: Vec::new(),
                    summary: "No web results found".to_string(),
                    provider_contract: "test".to_string(),
                })
            })
        }
    }

    #[tokio::test]
    async fn search_handle_is_object_safe_and_redacted() {
        let handle = WebSearchBackendHandle::new(EchoSearch);
        let response = handle
            .search(&SearchRequest {
                query: "Noema".to_string(),
                reason: None,
                max_results: 5,
            })
            .await
            .expect("search");

        assert_eq!(response.query, "Noema");
        assert_eq!(
            format!("{handle:?}"),
            "WebSearchBackendHandle(\"[CONFIGURED]\")"
        );
    }
}
