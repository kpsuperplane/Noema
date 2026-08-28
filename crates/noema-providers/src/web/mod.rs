//! Provider-owned web backend contracts.

mod browse;
mod fetch;
#[cfg(feature = "public-http")]
pub mod public_url;
mod search;

use std::{future::Future, pin::Pin};

pub use fetch::{
    DIRECT_HTTP_PROVIDER_ACCOUNT_ID, DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS,
    FIRECRAWL_KEYLESS_PROVIDER_ACCOUNT_ID, WebFetchBackend, WebFetchBackendHandle, WebFetchContext,
    WebFetchError,
};
pub use search::{
    BEST_EFFORT_PUBLIC_CONTRACT, DUCKDUCKGO_PUBLIC_PROVIDER_ACCOUNT_ID,
    DUCKDUCKGO_PUBLIC_PROVIDER_ID, WebSearchBackend, WebSearchBackendHandle, WebSearchError,
};

/// Boxed future returned by provider-owned web backends.
pub type WebOperationFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;
pub use browse::{
    KERNEL_BROWSER_PROVIDER_ID, OBSCURA_BROWSER_PROVIDER_ACCOUNT_ID, OBSCURA_BROWSER_PROVIDER_ID,
    WebBrowseBackendHandle, WebBrowseError, WebBrowseOwner,
};
