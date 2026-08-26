//! Provider-neutral interactive browser facade.

use noema_capabilities::web::browse::{BrowseCommand, BrowseResponse};
use std::{fmt, sync::Arc};

#[cfg(feature = "adapters")]
use crate::adapters::web::browse::{KernelBrowseBackend, ObscuraBrowseBackend};

/// Stable id for Noema's Obscura browser provider.
pub const OBSCURA_BROWSER_PROVIDER_ID: &str = "obscura";
/// Stable id for Kernel's hosted browser provider.
pub const KERNEL_BROWSER_PROVIDER_ID: &str = "kernel";

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
    /// Browser arguments violate the source contract.
    #[error("invalid browser arguments: {detail}")]
    InvalidArguments {
        /// Safe bounded parser detail.
        detail: String,
    },
    /// The URL is malformed or uses an unsupported scheme.
    #[error("invalid public web URL")]
    InvalidUrl,
    /// The URL targets a private, local, or otherwise blocked address.
    #[error("blocked private or local target")]
    BlockedTarget,
    /// This conversation or task has no active browser session.
    #[error("this conversation or task has no active browser session")]
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
    /// The worker remained available but could not navigate to the resource.
    #[error("browser navigation failed; use file.download for downloadable files")]
    NavigationFailed,
    /// The browser worker failed before dispatch.
    #[error("browser worker unavailable")]
    Unavailable,
    /// The browser provider account is not authenticated.
    #[error("browser provider account unauthenticated")]
    Unauthenticated,
    /// The configured browser provider route cannot be resolved.
    #[error("browser provider route unavailable")]
    RouteUnavailable,
    /// The current provider is the final provider in the configured route.
    #[error("no later browser provider is configured")]
    NoLaterProvider,
    /// The worker failed after a potentially mutating operation was dispatched.
    #[error("browser action outcome is uncertain")]
    OutcomeUncertain,
    /// A backend supplied a safe, bounded cause for a provider-neutral failure.
    #[error("{kind}; provider={provider}; stage={stage}; detail={detail}")]
    ProviderFailure {
        /// Provider-neutral failure category.
        kind: Box<WebBrowseError>,
        /// Stable backend identifier.
        provider: String,
        /// Stable operation stage.
        stage: String,
        /// Safe bounded provider detail.
        detail: String,
    },
}

impl WebBrowseError {
    /// Attach safe, bounded provider detail to this provider-neutral failure.
    #[must_use]
    pub fn with_provider_detail(
        self,
        provider: &str,
        stage: &str,
        detail: impl Into<String>,
    ) -> Self {
        Self::ProviderFailure {
            kind: Box::new(self),
            provider: provider.to_string(),
            stage: stage.to_string(),
            detail: detail.into(),
        }
    }
}

/// Clonable provider-neutral browser backend.
#[derive(Clone)]
pub struct WebBrowseBackendHandle(Arc<WebBrowseBackend>);

impl WebBrowseBackendHandle {
    /// Construct the process-isolated Obscura backend.
    #[cfg(feature = "adapters")]
    #[must_use]
    pub fn obscura(max_sessions: usize, max_old_space_mb: usize) -> Self {
        Self(Arc::new(WebBrowseBackend {
            implementation: WebBrowseBackendKind::Obscura(ObscuraBrowseBackend::new(
                max_sessions,
                max_old_space_mb,
            )),
        }))
    }

    /// Construct the hosted Kernel browser backend.
    #[cfg(feature = "adapters")]
    #[must_use]
    pub fn kernel(api_key: String, max_sessions: usize) -> Self {
        Self(Arc::new(WebBrowseBackend {
            implementation: WebBrowseBackendKind::Kernel(KernelBrowseBackend::new(
                api_key,
                max_sessions,
            )),
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
        match &self.0.implementation {
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Obscura(_) => OBSCURA_BROWSER_PROVIDER_ID,
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Kernel(_) => KERNEL_BROWSER_PROVIDER_ID,
            #[cfg(not(feature = "adapters"))]
            WebBrowseBackendKind::Unavailable => OBSCURA_BROWSER_PROVIDER_ID,
        }
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
struct WebBrowseBackend {
    implementation: WebBrowseBackendKind,
}

enum WebBrowseBackendKind {
    #[cfg(feature = "adapters")]
    Obscura(ObscuraBrowseBackend),
    #[cfg(feature = "adapters")]
    Kernel(KernelBrowseBackend),
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
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Kernel(backend) => backend.execute(_owner, _command).await,
            #[cfg(not(feature = "adapters"))]
            WebBrowseBackendKind::Unavailable => Err(WebBrowseError::Unavailable),
        }
    }

    async fn has_session(&self, _owner: &WebBrowseOwner) -> bool {
        match &self.implementation {
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Obscura(backend) => backend.has_session(_owner).await,
            #[cfg(feature = "adapters")]
            WebBrowseBackendKind::Kernel(backend) => backend.has_session(_owner).await,
            #[cfg(not(feature = "adapters"))]
            WebBrowseBackendKind::Unavailable => false,
        }
    }
}
