//! Bounded model-argument encoding for reviewed JSON REST plans.

use crate::{
    AdapterCredentialMaterial, ArgumentDefinition, ArgumentLocation, ArgumentType,
    AuthenticationSchemeV4, CompiledAdapterDefinition, CompiledOperation,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use url::Url;

const MAX_ARGUMENT_BYTES: usize = 256 * 1024;
const MAX_REQUEST_URL_BYTES: usize = 8 * 1024;

#[derive(Clone, PartialEq)]
pub(crate) struct EncodedAdapterRequest {
    pub(crate) url: Url,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) sensitive_headers: BTreeMap<String, String>,
    pub(crate) sensitive_query_names: BTreeSet<String>,
    pub(crate) body: Option<Value>,
}

impl std::fmt::Debug for EncodedAdapterRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EncodedAdapterRequest")
            .field("url", &"[REDACTED]")
            .field("headers", &self.headers)
            .field("sensitive_headers", &"[REDACTED]")
            .field("sensitive_query_names", &self.sensitive_query_names)
            .field("body", &self.body)
            .finish()
    }
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
    let mut query = operation
        .fixed_query
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<Vec<_>>();
    let mut body = Map::new();
    for argument in &operation.arguments {
        let Some(value) = values.get(&argument.name) else {
            if argument.required {
                return Err(AdapterRequestError);
            }
            continue;
        };
        if value.is_null() && !argument.required {
            continue;
        }
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
                if operation.json_body_template.is_none() {
                    body.insert(argument.name.clone(), value.clone());
                }
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
    if let crate::PaginationPolicy::ResponseToken {
        page_size: Some(page_size),
        ..
    } = &operation.pagination
    {
        append_runtime_query(
            &mut url,
            &page_size.request_argument,
            &page_size.value.to_string(),
        )?;
    }
    let reviewed_origin = Url::parse(&definition.origin).map_err(|_| AdapterRequestError)?;
    if url.as_str().len() > MAX_REQUEST_URL_BYTES
        || url.origin() != reviewed_origin.origin()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AdapterRequestError);
    }
    let body = operation
        .json_body_template
        .as_ref()
        .map(|template| render_json_body(template, values))
        .transpose()?
        .flatten()
        .or_else(|| (!body.is_empty()).then_some(Value::Object(body)));
    if body.as_ref().is_some_and(|body| {
        serde_json::to_vec(body).map_or(true, |bytes| bytes.len() > MAX_ARGUMENT_BYTES)
    }) {
        return Err(AdapterRequestError);
    }
    Ok(EncodedAdapterRequest {
        url,
        headers: operation.fixed_headers.clone(),
        sensitive_headers: BTreeMap::new(),
        sensitive_query_names: BTreeSet::new(),
        body,
    })
}

pub(crate) fn inject_pagination_token(
    request: &mut EncodedAdapterRequest,
    operation: &CompiledOperation,
    token: &str,
) -> Result<(), AdapterRequestError> {
    let crate::PaginationPolicy::ResponseToken {
        request_argument, ..
    } = &operation.pagination
    else {
        return Err(AdapterRequestError);
    };
    if token.is_empty()
        || token.len() > 4 * 1024
        || token.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(AdapterRequestError);
    }
    append_runtime_query(&mut request.url, request_argument, token)
}

fn append_runtime_query(url: &mut Url, name: &str, value: &str) -> Result<(), AdapterRequestError> {
    if name.is_empty()
        || name.len() > 128
        || name.bytes().any(|byte| byte.is_ascii_control())
        || url.query_pairs().any(|(existing, _)| existing == name)
    {
        return Err(AdapterRequestError);
    }
    url.query_pairs_mut().append_pair(name, value);
    (url.as_str().len() <= MAX_REQUEST_URL_BYTES)
        .then_some(())
        .ok_or(AdapterRequestError)
}

fn render_json_body(
    template: &Value,
    arguments: &Map<String, Value>,
) -> Result<Option<Value>, AdapterRequestError> {
    match template {
        Value::Object(object) if object.len() == 1 && object.contains_key("$argument") => {
            let name = object["$argument"].as_str().ok_or(AdapterRequestError)?;
            Ok(arguments
                .get(name)
                .filter(|value| !value.is_null())
                .cloned())
        }
        Value::Object(object) => {
            let rendered = object
                .iter()
                .filter_map(|(name, value)| {
                    render_json_body(value, arguments)
                        .transpose()
                        .map(|result| result.map(|value| (name.clone(), value)))
                })
                .collect::<Result<Map<_, _>, _>>()?;
            Ok(Some(Value::Object(rendered)))
        }
        Value::Array(values) => {
            let mut rendered = Vec::with_capacity(values.len());
            for value in values {
                if let Some(item) = render_json_body(value, arguments)?
                    && !(value.as_object().is_some_and(|object| !object.is_empty())
                        && item.as_object().is_some_and(Map::is_empty))
                {
                    rendered.push(item);
                }
            }
            Ok(Some(Value::Array(rendered)))
        }
        value => Ok(Some(value.clone())),
    }
}

/// Apply one reviewed credential transform without changing request authority.
pub(crate) fn apply_credential_auth(
    definition: &CompiledAdapterDefinition,
    operation: &CompiledOperation,
    request: &mut EncodedAdapterRequest,
    credential: &AdapterCredentialMaterial,
) -> Result<(), AdapterRequestError> {
    let AuthenticationSchemeV4::Credential(config) = &definition.authentication else {
        return Err(AdapterRequestError);
    };
    let AdapterCredentialMaterial::Credential { fields } = credential else {
        return Err(AdapterRequestError);
    };
    let input = json!({
        "credentials": fields,
        "request": {
            "operation_id": operation.operation_id,
            "method": format!("{:?}", operation.method).to_ascii_uppercase(),
            "path": request.url.path(),
            "header_names": request.headers.keys().collect::<Vec<_>>(),
            "query_names": request.url.query_pairs().map(|(name, _)| name.into_owned()).collect::<Vec<_>>(),
        }
    });
    let output = crate::luau::decorate_request(config.request_auth.source(), &input)
        .map_err(|_| AdapterRequestError)?;
    let Value::Object(mut output) = output else {
        return Err(AdapterRequestError);
    };
    if output
        .keys()
        .any(|key| !matches!(key.as_str(), "headers" | "query"))
    {
        return Err(AdapterRequestError);
    }
    let headers = output
        .remove("headers")
        .map(string_map)
        .transpose()?
        .unwrap_or_default();
    let query = output
        .remove("query")
        .map(string_map)
        .transpose()?
        .unwrap_or_default();
    if headers.len() > 16 || query.len() > 16 {
        return Err(AdapterRequestError);
    }
    let existing_headers = request
        .headers
        .keys()
        .map(|name| name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut normalized = BTreeSet::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if !valid_auth_header_name(&name)
            || !normalized.insert(lower.clone())
            || existing_headers.contains(&lower)
            || value.is_empty()
            || value.len() > 16 * 1024
            || value.contains(['\r', '\n'])
        {
            return Err(AdapterRequestError);
        }
        request.sensitive_headers.insert(name, value);
    }
    let existing_query = request
        .url
        .query_pairs()
        .map(|(name, _)| name.into_owned())
        .collect::<BTreeSet<_>>();
    for (name, value) in query {
        if name.is_empty()
            || name.len() > 128
            || name.bytes().any(|byte| byte.is_ascii_control())
            || existing_query.contains(&name)
            || value.is_empty()
            || value.len() > 16 * 1024
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(AdapterRequestError);
        }
        request
            .sensitive_query_names
            .insert(name.to_ascii_lowercase());
        request.url.query_pairs_mut().append_pair(&name, &value);
    }
    if request.url.as_str().len() > MAX_REQUEST_URL_BYTES {
        return Err(AdapterRequestError);
    }
    Ok(())
}

fn string_map(value: Value) -> Result<BTreeMap<String, String>, AdapterRequestError> {
    let Value::Object(values) = value else {
        return Err(AdapterRequestError);
    };
    values
        .into_iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key, value.to_string()))
                .ok_or(AdapterRequestError)
        })
        .collect()
}

fn valid_auth_header_name(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "host"
            | "content-length"
            | "content-type"
            | "content-encoding"
            | "content-range"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "upgrade"
            | "http2-settings"
            | "expect"
            | "via"
            | "cookie"
            | "set-cookie"
            | "accept-encoding"
            | "forwarded"
    ) && !lower.starts_with("x-forwarded-")
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
