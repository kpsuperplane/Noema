//! Interactive browser adapters.

use noema_capabilities::web::url_policy::validate_public_url;

use crate::{WebBrowseError, WebFetchError};

mod kernel;
mod obscura;

pub(crate) use kernel::KernelBrowseBackend;
pub(crate) use obscura::ObscuraBrowseBackend;

pub(crate) use obscura::process::run_if_requested as run_worker_if_requested;

fn map_url_error(error: WebFetchError) -> WebBrowseError {
    match error {
        WebFetchError::BlockedTarget | WebFetchError::RedirectBlocked => {
            WebBrowseError::BlockedTarget
        }
        WebFetchError::Timeout => WebBrowseError::Timeout,
        WebFetchError::UnsupportedScheme | WebFetchError::MalformedUrl => {
            WebBrowseError::InvalidUrl
        }
        _ => WebBrowseError::Unavailable,
    }
}

fn map_resulting_url_error(error: WebFetchError) -> WebBrowseError {
    match map_url_error(error) {
        WebBrowseError::InvalidUrl | WebBrowseError::BlockedTarget => WebBrowseError::BlockedTarget,
        error => error,
    }
}

fn public_display_url(raw: String) -> Option<String> {
    validate_public_url(&raw).ok().map(|url| url.to_string())
}

fn truncate_chars(value: String, limit: usize) -> (String, bool) {
    let mut characters = value.char_indices();
    let Some((byte_index, _)) = characters.nth(limit) else {
        return (value, false);
    };
    (value[..byte_index].to_string(), true)
}

/// Build the default interactive browser backend.
#[must_use]
pub fn default_web_browse_backend(
    max_sessions: usize,
    max_old_space_mb: usize,
) -> crate::WebBrowseBackendHandle {
    crate::WebBrowseBackendHandle::obscura(max_sessions, max_old_space_mb)
}
