//! Embedded Obscura browser adapter.

mod obscura;

pub(crate) use obscura::ObscuraBrowseBackend;

pub(crate) use obscura::process::run_if_requested as run_worker_if_requested;

/// Build the default interactive browser backend.
#[must_use]
pub fn default_web_browse_backend(
    max_sessions: usize,
    max_old_space_mb: usize,
) -> crate::WebBrowseBackendHandle {
    crate::WebBrowseBackendHandle::obscura(max_sessions, max_old_space_mb)
}
