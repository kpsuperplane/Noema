/// Safe failures returned by the interactive browser backend.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebBrowseError {
    /// The URL is malformed or uses an unsupported scheme.
    #[error("invalid public web URL")]
    InvalidUrl,
    /// The URL targets a private, local, or otherwise blocked address.
    #[error("blocked private or local target")]
    BlockedTarget,
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
    /// The worker failed after a potentially mutating operation.
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
