//! Provider-owned web backend contracts.

mod fetch;
mod search;

use std::{future::Future, pin::Pin};

pub use fetch::{
    DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS, WebFetchBackend, WebFetchBackendHandle,
    WebFetchContext, WebFetchError,
};
pub use search::{
    BEST_EFFORT_PUBLIC_CONTRACT, DUCKDUCKGO_PUBLIC_PROVIDER_ID, WebSearchBackend,
    WebSearchBackendHandle, WebSearchError,
};

/// Boxed future returned by provider-owned web backends.
pub type WebOperationFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;
