//! One bounded response-decoding authority shared by invocation and setup probes.

use crate::{
    ResponseContract,
    network::{AdapterHttpError, AdapterHttpResponse},
};
use serde_json::Value;

pub(crate) async fn success(
    response: &AdapterHttpResponse,
    contract: Option<&ResponseContract>,
) -> Result<Value, AdapterHttpError> {
    let Some(contract) = contract else {
        return if response.status == 204 {
            Ok(Value::Null)
        } else {
            json(response)
        };
    };
    if response.status != 204
        && !response
            .content_type
            .as_ref()
            .is_some_and(|value| contract.accepted_content_types.contains(value))
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let mut response = response.clone();
    if response.status == 204 {
        response.content_type = None;
        response.body.clear();
    }
    let transform = contract.transform.clone();
    let schema = contract.output_schema.clone();
    tokio::task::spawn_blocking(move || crate::luau::transform(&transform, &schema, &response))
        .await
        .map_err(|_| AdapterHttpError::InvalidResponse)?
        .map_err(|_| AdapterHttpError::InvalidResponse)
}

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
