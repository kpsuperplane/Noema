//! One bounded response-decoding authority shared by invocation and setup probes.

use crate::{
    ResponseContract,
    network::{AdapterHttpError, AdapterHttpResponse},
};
use serde_json::Value;

pub(crate) async fn success(
    response: &AdapterHttpResponse,
    contract: &ResponseContract,
) -> Result<Value, AdapterHttpError> {
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
    tokio::task::spawn_blocking(move || {
        let value = match transform {
            Some(transform) => crate::luau::transform(&transform, &response)
                .map_err(|_| AdapterHttpError::InvalidResponse)?,
            None if response.status == 204 => Value::Null,
            None => json(&response)?,
        };
        if !crate::output_schema::matches(&schema, &value)
            || serde_json::to_vec(&value)
                .map_err(|_| AdapterHttpError::InvalidResponse)?
                .len()
                > crate::output_schema::MAX_MODEL_RESULT_BYTES
        {
            return Err(AdapterHttpError::InvalidResponse);
        }
        Ok(value)
    })
    .await
    .map_err(|_| AdapterHttpError::InvalidResponse)?
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

pub(crate) fn extract_pagination_token(
    response: &AdapterHttpResponse,
    pointer: &str,
) -> Result<(Option<String>, AdapterHttpResponse), AdapterHttpError> {
    let mut value = json(response)?;
    let token = match value.pointer(pointer) {
        None | Some(Value::Null) => None,
        Some(Value::String(token)) if !token.is_empty() && token.len() <= 4 * 1024 => {
            Some(token.clone())
        }
        _ => return Err(AdapterHttpError::InvalidResponse),
    };
    if token.is_some() && !remove_json_pointer(&mut value, pointer) {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let mut sanitized = response.clone();
    sanitized.body = serde_json::to_vec(&value).map_err(|_| AdapterHttpError::InvalidResponse)?;
    Ok((token, sanitized))
}

pub(crate) fn inject_continuation(
    mut payload: Value,
    continuation: Option<&str>,
) -> Result<Value, AdapterHttpError> {
    let Value::Object(object) = &mut payload else {
        return Err(AdapterHttpError::InvalidResponse);
    };
    if let Some(continuation) = continuation {
        if object.contains_key("continuation") || continuation.len() > 128 {
            return Err(AdapterHttpError::InvalidResponse);
        }
        object.insert(
            "continuation".to_string(),
            Value::String(continuation.to_string()),
        );
    }
    if serde_json::to_vec(&payload)
        .map_err(|_| AdapterHttpError::InvalidResponse)?
        .len()
        > crate::output_schema::MAX_MODEL_RESULT_BYTES
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    Ok(payload)
}

fn remove_json_pointer(value: &mut Value, pointer: &str) -> bool {
    let mut parts = pointer
        .split('/')
        .skip(1)
        .map(|part| part.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let Some(last) = parts.pop() else {
        return false;
    };
    let mut parent = value;
    for part in parts {
        parent = match parent {
            Value::Object(object) => match object.get_mut(&part) {
                Some(value) => value,
                None => return false,
            },
            Value::Array(array) => match part
                .parse::<usize>()
                .ok()
                .and_then(|index| array.get_mut(index))
            {
                Some(value) => value,
                None => return false,
            },
            _ => return false,
        };
    }
    match parent {
        Value::Object(object) => object.remove(&last).is_some(),
        Value::Array(array) => last
            .parse::<usize>()
            .ok()
            .filter(|index| *index < array.len())
            .map(|index| array.remove(index))
            .is_some(),
        _ => false,
    }
}
