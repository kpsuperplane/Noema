use std::{fmt, ops::Deref, sync::Arc};

use noema_capabilities::web::search::{SearchRequest, SearchResponse};

use super::WebOperationFuture;

/// Stable id for Noema's default public DuckDuckGo provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ID: &str = "duckduckgo_public";
/// Stable account id for Noema's built-in public search provider.
pub const DUCKDUCKGO_PUBLIC_PROVIDER_ACCOUNT_ID: &str = "provider_account:duckduckgo_public:system";
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
    /// Stable provider backend identifier.
    fn backend_id(&self) -> &str;

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
}

impl Deref for WebSearchBackendHandle {
    type Target = dyn WebSearchBackend;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
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
