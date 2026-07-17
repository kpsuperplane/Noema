//! Transitional aliases for provider-owned web fetch contracts.

#[cfg(test)]
pub use noema_providers::EXTRACTION_READABILITYRS;
pub use noema_providers::{
    DIRECT_HTTP_PROVIDER_ID, WebFetchBackendHandle as WebFetchRuntimeProvider,
    WebFetchContext as FetchRuntimeContext, WebFetchError as FetchError,
};
