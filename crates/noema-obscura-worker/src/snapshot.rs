use crate::url_policy::validate_public_url;
use crate::{WebBrowseError, public_url::WebFetchError};
use serde::Deserialize;
pub(crate) fn map_url_error(error: WebFetchError) -> WebBrowseError {
    match error {
        WebFetchError::BlockedTarget => WebBrowseError::BlockedTarget,
        WebFetchError::UnsupportedScheme | WebFetchError::MalformedUrl => {
            WebBrowseError::InvalidUrl
        }
        _ => WebBrowseError::Unavailable,
    }
}

pub(crate) fn map_resulting_url_error(error: WebFetchError) -> WebBrowseError {
    match map_url_error(error) {
        WebBrowseError::InvalidUrl | WebBrowseError::BlockedTarget => WebBrowseError::BlockedTarget,
        error => error,
    }
}

pub(crate) fn public_display_url(raw: String) -> Option<String> {
    validate_public_url(&raw).ok().map(|url| url.to_string())
}

#[derive(Deserialize)]
pub(crate) struct RawSubmissionContext {
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

pub(crate) fn submission_context(
    raw: Option<RawSubmissionContext>,
) -> Option<crate::protocol::BrowseSubmissionContext> {
    let raw = raw?;
    let destination = public_display_url(raw.destination)?;
    let field_count = raw.fields.len();
    Some(crate::protocol::BrowseSubmissionContext {
        destination,
        method: truncate_chars(raw.method.to_uppercase(), 16).0,
        fields: raw
            .fields
            .into_iter()
            .take(64)
            .map(|field| crate::protocol::BrowseSubmissionField {
                name: truncate_chars(field.name, 256).0,
                value: truncate_chars(field.value, 1_000).0,
            })
            .collect(),
        omitted_control_count: raw.omitted_control_count,
        truncated: raw.truncated || field_count > 64,
    })
}

pub(crate) fn truncate_chars(value: String, limit: usize) -> (String, bool) {
    let mut characters = value.char_indices();
    let Some((byte_index, _)) = characters.nth(limit) else {
        return (value, false);
    };
    (value[..byte_index].to_string(), true)
}
