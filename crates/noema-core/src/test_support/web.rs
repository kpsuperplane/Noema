use noema_capabilities::web::{fetch::FetchRequest, search::SearchRequest};
use noema_providers::{
    BEST_EFFORT_PUBLIC_CONTRACT, DIRECT_HTTP_PROVIDER_ID, DUCKDUCKGO_PUBLIC_PROVIDER_ID,
    WebFetchBackend, WebFetchBackendHandle, WebFetchContext, WebFetchError, WebOperationFuture,
    WebSearchBackend, WebSearchBackendHandle, WebSearchError,
};

pub(super) fn test_search_backend() -> WebSearchBackendHandle {
    WebSearchBackendHandle::new(TestWebSearchBackend)
}

pub(super) fn test_fetch_backend() -> WebFetchBackendHandle {
    WebFetchBackendHandle::new(TestWebFetchBackend)
}

struct TestWebSearchBackend;

impl WebSearchBackend for TestWebSearchBackend {
    fn backend_id(&self) -> &str {
        DUCKDUCKGO_PUBLIC_PROVIDER_ID
    }

    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> WebOperationFuture<'a, noema_capabilities::web::search::SearchResponse, WebSearchError>
    {
        Box::pin(async move {
            Ok(noema_capabilities::web::search::SearchResponse {
                provider: DUCKDUCKGO_PUBLIC_PROVIDER_ID.to_string(),
                provider_contract: BEST_EFFORT_PUBLIC_CONTRACT.to_string(),
                query: request.query.clone(),
                results: Vec::new(),
                summary: "No web results found".to_string(),
            })
        })
    }
}

struct TestWebFetchBackend;

impl WebFetchBackend for TestWebFetchBackend {
    fn backend_id(&self) -> &str {
        DIRECT_HTTP_PROVIDER_ID
    }

    fn fetch<'a>(
        &'a self,
        _request: &'a FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, noema_capabilities::web::fetch::FetchResponse, WebFetchError> {
        Box::pin(async { Err(WebFetchError::Http) })
    }
}
