//! Host-provided construction boundary for concrete web capability backends.

use std::{future::Future, pin::Pin, sync::Arc};

use noema_providers::{WebBrowseBackendHandle, WebFetchBackendHandle, WebSearchBackendHandle};
use thiserror::Error;

/// Exact provider account selected for a web capability invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebBackendRequest {
    /// Provider implementation kind.
    pub provider_kind: String,
    /// Stable provider account identifier.
    pub provider_account_id: String,
    /// Credential revision captured with the account selection.
    pub credential_revision: u64,
}

/// Failure to construct a selected web capability backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WebBackendResolverError {
    /// The selected account has no usable credential.
    #[error("provider account unauthenticated")]
    Unauthenticated,
    /// The host cannot construct the selected provider implementation.
    #[error("provider backend unavailable")]
    Unavailable,
}

/// Future returned by the host-provided web backend resolver.
pub type WebBackendFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, WebBackendResolverError>> + Send + 'a>>;

/// Concrete web backend construction supplied by the application host.
pub trait WebBackendResolver: std::fmt::Debug + Send + Sync {
    /// Resolve a web-search backend for an exact provider account.
    fn resolve_search(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebSearchBackendHandle>;

    /// Resolve a web-fetch backend for an exact provider account.
    fn resolve_fetch(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebFetchBackendHandle>;

    /// Resolve an interactive browser backend for an exact provider account.
    fn resolve_browse(
        &self,
        _request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebBrowseBackendHandle> {
        Box::pin(async { Err(WebBackendResolverError::Unavailable) })
    }

    /// Persist an authentication failure fenced by the observed credential revision.
    fn record_auth_failure(
        &self,
        provider_account_id: String,
        credential_revision: u64,
    ) -> WebBackendFuture<'_, ()>;
}

/// Shared host-provided web backend resolver.
pub type WebBackendResolverHandle = Arc<dyn WebBackendResolver>;
