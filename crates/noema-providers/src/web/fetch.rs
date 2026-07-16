use std::{fmt, sync::Arc};

use noema_capabilities::web::fetch::{FetchRequest, FetchResponse};

use super::WebOperationFuture;
use crate::{GenerationPriority, ProviderRouteLease, ReasoningEffort};

/// Stable id for Noema's checked direct-HTTP provider.
pub const DIRECT_HTTP_PROVIDER_ID: &str = "direct_http";
/// Extraction label for the readability-based HTML backend.
pub const EXTRACTION_READABILITYRS: &str = "readability_rs";

/// Runtime context required by provider-owned fetch backends.
#[derive(Debug, Clone)]
pub struct WebFetchContext {
    /// Exact leased provider route used for summarization.
    pub summarizer_route: Arc<ProviderRouteLease>,
    /// Summarization model profile.
    pub summarizer_model: String,
    /// Optional summarization reasoning effort.
    pub summarizer_reasoning_effort: Option<ReasoningEffort>,
    /// Scheduling priority for summarization calls.
    pub generation_priority: GenerationPriority,
}

/// Safe failures returned by provider-owned web fetch backends.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebFetchError {
    /// Only public HTTP and HTTPS URLs are supported.
    #[error("unsupported URL scheme")]
    UnsupportedScheme,
    /// The URL could not be parsed.
    #[error("malformed URL")]
    MalformedUrl,
    /// The URL resolves to a private or local target.
    #[error("blocked private or local target")]
    BlockedTarget,
    /// DNS resolution failed.
    #[error("DNS lookup failed")]
    Dns,
    /// A redirect resolved to a blocked target.
    #[error("redirect target is blocked")]
    RedirectBlocked,
    /// The backend exceeded its redirect limit.
    #[error("too many redirects")]
    TooManyRedirects,
    /// The request timed out.
    #[error("fetch request timed out")]
    Timeout,
    /// The provider transport failed.
    #[error("fetch request failed")]
    Http,
    /// The provider account is not authenticated.
    #[error("provider account unauthenticated")]
    AuthFailed,
    /// The response content type is unsupported.
    #[error("unsupported content type")]
    UnsupportedContentType,
    /// The response exceeded its byte or character limit.
    #[error("response too large")]
    ResponseTooLarge,
    /// Readable content could not be extracted.
    #[error("page content could not be extracted")]
    Extraction,
    /// Model-backed summarization failed.
    #[error("summarization failed")]
    Summarization,
    /// The page exceeded the responsible summarization limit.
    #[error("page too large to summarize")]
    PageTooLarge,
}

/// Object-safe provider backend for the first-party `web.fetch` capability.
pub trait WebFetchBackend: Send + Sync {
    /// Stable provider backend identifier.
    fn backend_id(&self) -> &str;

    /// Execute one validated fetch request.
    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError>;
}

/// Clonable, redacted handle to one web fetch backend.
#[derive(Clone)]
pub struct WebFetchBackendHandle(Arc<dyn WebFetchBackend>);

impl WebFetchBackendHandle {
    /// Erase one concrete web fetch backend behind a shared handle.
    #[must_use]
    pub fn new(backend: impl WebFetchBackend + 'static) -> Self {
        Self(Arc::new(backend))
    }

    /// Erase an existing shared web fetch backend.
    #[must_use]
    pub fn from_arc(backend: Arc<dyn WebFetchBackend>) -> Self {
        Self(backend)
    }

    /// Return the stable provider backend identifier.
    #[must_use]
    pub fn backend_id(&self) -> &str {
        self.0.backend_id()
    }

    /// Execute one validated fetch request.
    ///
    /// # Errors
    ///
    /// Returns a typed provider backend failure.
    pub async fn fetch(
        &self,
        request: &FetchRequest,
        context: &WebFetchContext,
    ) -> Result<FetchResponse, WebFetchError> {
        self.0.fetch(request, context).await
    }
}

impl fmt::Debug for WebFetchBackendHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WebFetchBackendHandle")
            .field(&"[CONFIGURED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingFetch;

    impl WebFetchBackend for FailingFetch {
        fn backend_id(&self) -> &str {
            "failing"
        }

        fn fetch<'a>(
            &'a self,
            _request: &'a FetchRequest,
            _context: &'a WebFetchContext,
        ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
            Box::pin(async { Err(WebFetchError::Http) })
        }
    }

    #[test]
    fn fetch_handle_is_object_safe_and_redacted() {
        let handle = WebFetchBackendHandle::new(FailingFetch);

        assert_eq!(
            format!("{handle:?}"),
            "WebFetchBackendHandle(\"[CONFIGURED]\")",
        );
        assert_eq!(handle.backend_id(), "failing");
    }
}
