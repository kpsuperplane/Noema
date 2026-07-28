//! One bounded response-decoding authority shared by invocation and setup probes.

use crate::network::{AdapterHttpError, AdapterHttpResponse};
use serde_json::Value;

pub(crate) fn json(response: &AdapterHttpResponse) -> Result<Value, AdapterHttpError> {
    let content_type = response.content_type.as_deref().unwrap_or_default();
    if content_type != "application/json" && !content_type.ends_with("+json") {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let value = crate::json_limits::parse_without_duplicate_keys(&response.body)
        .map_err(|_| AdapterHttpError::InvalidResponse)?;
    crate::json_limits::validate_json_shape(&value)
        .then_some(value)
        .ok_or(AdapterHttpError::InvalidResponse)
}
