//! Provider-neutral interactive browser facade.

use noema_capabilities::web::browse::{BrowseCommand, BrowseResponse};
use std::{fmt, sync::Arc};

#[cfg(feature = "adapters")]
use crate::adapters::web::browse::ObscuraBrowseBackend;

/// Stable id for Noema's embedded Obscura browser provider.
pub const OBSCURA_BROWSER_PROVIDER_ID: &str = "obscura";

/// Opaque execution authority for one browser session.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WebBrowseOwner(String);

impl WebBrowseOwner {
    /// Construct an owner from an already-authorized runtime identity.
    #[must_use]
    pub fn new(identity: impl Into<String>) -> Self {
        Self(identity.into())
    }

    #[cfg(feature = "adapters")]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Safe failures returned by the interactive browser backend.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebBrowseError {
    /// The URL is malformed or uses an unsupported scheme.
    #[error("invalid public web URL")]
    InvalidUrl,
    /// The URL targets a private, local, or otherwise blocked address.
    #[error("blocked private or local target")]
    BlockedTarget,
    /// This execution already owns an active browser session.
    #[error("this execution already has an active browser session")]
    SessionAlreadyOpen,
    /// This execution has no active browser session.
    #[error("this execution has no active browser session")]
    SessionNotFound,
    /// The process-wide browser session limit has been reached.
    #[error("browser session capacity reached")]
    Capacity,
    /// The request refers to an older page snapshot.
    #[error("browser snapshot is stale; take a new snapshot")]
    StaleSnapshot,
    /// The requested element is no longer available.
    #[error("browser element reference is unavailable")]
    ElementNotFound,
    /// The bounded browser operation timed out.
    #[error("browser operation timed out")]
    Timeout,
    /// The requested history entry does not exist.
    #[error("browser history entry is unavailable")]
    HistoryUnavailable,
    /// The embedded browser worker failed before dispatch.
    #[error("browser worker unavailable")]
    Unavailable,
    /// The worker failed after a potentially mutating operation was dispatched.
    #[error("browser action outcome is uncertain")]
    OutcomeUncertain,
}

/// Clonable provider-neutral browser backend.
#[derive(Clone)]
pub struct WebBrowseBackendHandle(Arc<WebBrowseBackend>);

impl WebBrowseBackendHandle {
    /// Construct the embedded Obscura backend.
    #[cfg(feature = "adapters")]
    #[must_use]
    pub fn obscura() -> Self {
        Self(Arc::new(WebBrowseBackend {
            implementation: WebBrowseBackendKind::Obscura(ObscuraBrowseBackend::new()),
        }))
    }

    /// Execute a browser operation for one authorized execution owner.
    ///
    /// # Errors
    ///
    /// Returns a safe provider-neutral browser failure.
    pub async fn execute(
        &self,
        owner: &WebBrowseOwner,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        self.0.execute(owner, command).await
    }

    /// Return whether this owner currently has a live browser session.
    pub async fn has_session(&self, owner: &WebBrowseOwner) -> bool {
        self.0.has_session(owner).await
    }

    /// Stable provider id selected by this backend.
    #[must_use]
    pub fn backend_id(&self) -> &'static str {
        OBSCURA_BROWSER_PROVIDER_ID
    }
}

impl fmt::Debug for WebBrowseBackendHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WebBrowseBackendHandle")
            .field(&"[CONFIGURED]")
            .finish()
    }
}

/// Browser facade whose concrete implementation remains private.
pub struct WebBrowseBackend {
    implementation: WebBrowseBackendKind,
}

enum WebBrowseBackendKind {
    #[cfg(feature = "adapters")]
    Obscura(ObscuraBrowseBackend),
    #[cfg(not(feature = "adapters"))]
    #[allow(dead_code, reason = "transport-free builds retain the public facade")]
    Unavailable,
}

impl WebBrowseBackend {
    async fn execute(
        &self,
        _owner: &WebBrowseOwner,
        _command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        match &self.implementation {
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Obscura(backend) => backend.execute(_owner, _command).await,
            #[cfg(not(feature = "adapters"))]
            WebBrowseBackendKind::Unavailable => Err(WebBrowseError::Unavailable),
        }
    }

    async fn has_session(&self, _owner: &WebBrowseOwner) -> bool {
        match &self.implementation {
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Obscura(backend) => backend.has_session(_owner).await,
            #[cfg(not(feature = "adapters"))]
            WebBrowseBackendKind::Unavailable => false,
        }
    }
}
