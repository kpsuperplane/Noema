//! Transitional aliases for provider-owned web fetch contracts.

pub use noema_providers::{
    DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS,
    WebFetchBackendHandle as WebFetchRuntimeProvider, WebFetchContext as FetchRuntimeContext,
    WebFetchError as FetchError,
};
