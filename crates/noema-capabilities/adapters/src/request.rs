//! Bounded model-argument encoding for reviewed JSON REST plans.

use crate::{
    ArgumentDefinition, ArgumentLocation, ArgumentType, CompiledAdapterDefinition,
    CompiledOperation,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use url::Url;

const MAX_ARGUMENT_BYTES: usize = 256 * 1024;
const MAX_REQUEST_URL_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EncodedAdapterRequest {
    pub(crate) url: Url,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) body: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("adapter request arguments are invalid")]
pub(crate) struct AdapterRequestError;

pub(crate) fn encode_request(
    definition: &CompiledAdapterDefinition,
    operation: &CompiledOperation,
    arguments: &Value,
) -> Result<EncodedAdapterRequest, AdapterRequestError> {
    if serde_json::to_vec(arguments)
        .map_err(|_| AdapterRequestError)?
        .len()
        > MAX_ARGUMENT_BYTES
    {
        return Err(AdapterRequestError);
    }
    let empty = Map::new();
    let values = match arguments {
        Value::Object(values) => values,
        Value::Null if operation.arguments.is_empty() => &empty,
        _ => return Err(AdapterRequestError),
    };
    let expected = operation
        .arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect::<BTreeSet<_>>();
    if values.keys().any(|name| !expected.contains(name.as_str())) {
        return Err(AdapterRequestError);
    }

    let mut rendered_path = operation.path.clone();
    let mut query = Vec::new();
    let mut body = Map::new();
    for argument in &operation.arguments {
        let Some(value) = values.get(&argument.name) else {
            if argument.required {
                return Err(AdapterRequestError);
            }
            continue;
        };
        validate_value(argument, value)?;
        match argument.location {
            ArgumentLocation::Path => {
                let scalar = scalar_text(value).ok_or(AdapterRequestError)?;
                if matches!(scalar.as_str(), "." | "..") {
                    return Err(AdapterRequestError);
                }
                rendered_path = rendered_path.replace(
                    &format!("{{{}}}", argument.name),
                    &encode_path_segment(&scalar),
                );
            }
            ArgumentLocation::Query => append_query(&mut query, &argument.name, value)?,
            ArgumentLocation::JsonBody => {
                body.insert(argument.name.clone(), value.clone());
            }
        }
    }
    if rendered_path.contains(['{', '}']) {
        return Err(AdapterRequestError);
    }

    let mut url = Url::parse(&format!(
        "{}{}",
        definition.origin.trim_end_matches('/'),
        rendered_path
    ))
    .map_err(|_| AdapterRequestError)?;
    if !query.is_empty() {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in query {
            serializer.append_pair(&name, &value);
        }
        url.set_query(Some(&serializer.finish()));
    }
    let reviewed_origin = Url::parse(&definition.origin).map_err(|_| AdapterRequestError)?;
    if url.as_str().len() > MAX_REQUEST_URL_BYTES
        || url.origin() != reviewed_origin.origin()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AdapterRequestError);
    }
    Ok(EncodedAdapterRequest {
        url,
        headers: operation.fixed_headers.clone(),
        body: (!body.is_empty()).then_some(Value::Object(body)),
    })
}

fn validate_value(argument: &ArgumentDefinition, value: &Value) -> Result<(), AdapterRequestError> {
    let valid_type = match argument.argument_type {
        ArgumentType::String => value.is_string(),
        ArgumentType::Integer => value
            .as_number()
            .is_some_and(|number| number.is_i64() || number.is_u64()),
        ArgumentType::Number => value.is_number(),
        ArgumentType::Boolean => value.is_boolean(),
        ArgumentType::StringArray => value
            .as_array()
            .is_some_and(|values| values.iter().all(Value::is_string)),
    };
    if !valid_type
        || (!argument.enum_values.is_empty()
            && !value
                .as_str()
                .is_some_and(|value| argument.enum_values.iter().any(|item| item == value)))
    {
        return Err(AdapterRequestError);
    }
    Ok(())
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn append_query(
    query: &mut Vec<(String, String)>,
    name: &str,
    value: &Value,
) -> Result<(), AdapterRequestError> {
    if let Some(values) = value.as_array() {
        for value in values {
            query.push((
                name.to_string(),
                value.as_str().ok_or(AdapterRequestError)?.to_string(),
            ));
        }
    } else {
        query.push((
            name.to_string(),
            scalar_text(value).ok_or(AdapterRequestError)?,
        ));
    }
    Ok(())
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

#[cfg(test)]
mod tests;
