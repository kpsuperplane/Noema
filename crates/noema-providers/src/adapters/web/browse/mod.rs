//! Embedded Obscura browser adapter.

mod obscura;

pub(crate) use obscura::ObscuraBrowseBackend;

/// Build the default interactive browser backend.
#[must_use]
pub fn default_web_browse_backend() -> crate::WebBrowseBackendHandle {
    crate::WebBrowseBackendHandle::obscura()
}
