//! Interactive browser adapters.

use noema_capabilities::web::url_policy::validate_public_url;
use serde::Deserialize;

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

#[derive(Deserialize)]
struct RawSubmissionContext {
    destination: String,
    method: String,
    fields: Vec<RawSubmissionField>,
    omitted_control_count: usize,
    truncated: bool,
}

#[derive(Deserialize)]
struct RawSubmissionField {
    name: String,
    value: String,
}

fn submission_context(
    raw: Option<RawSubmissionContext>,
) -> Option<noema_capabilities::web::browse::BrowseSubmissionContext> {
    let raw = raw?;
    let destination = public_display_url(raw.destination)?;
    let field_count = raw.fields.len();
    Some(noema_capabilities::web::browse::BrowseSubmissionContext {
        destination,
        method: truncate_chars(raw.method.to_uppercase(), 16).0,
        fields: raw
            .fields
            .into_iter()
            .take(64)
            .map(
                |field| noema_capabilities::web::browse::BrowseSubmissionField {
                    name: truncate_chars(field.name, 256).0,
                    value: truncate_chars(field.value, 1_000).0,
                },
            )
            .collect(),
        omitted_control_count: raw.omitted_control_count,
        truncated: raw.truncated || field_count > 64,
    })
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
