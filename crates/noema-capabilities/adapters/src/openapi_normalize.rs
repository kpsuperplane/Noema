//! Source-shape checks and OpenAPI-to-v1 argument normalization.

use crate::{
    ArgumentDefinition, ArgumentLocation, ArgumentSource, ArgumentType,
    json_limits::parse_without_duplicate_keys,
    openapi::{
        MAX_OPENAPI_DEPTH, MAX_SOURCE_REFERENCE_BYTES, MAX_TEXT_BYTES, OpenApiDiagnostic,
        OpenApiDiagnosticSeverity, OpenApiImportError, OpenApiSourceFormat,
    },
    openapi_schema::{SchemaFailure, SchemaResolver, component_name},
};
use openapiv3::{
    Components, OpenAPI, Parameter, ParameterSchemaOrContent, PathStyle, QueryStyle, ReferenceOr,
    RequestBody,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

pub(crate) fn parse_source(
    bytes: &[u8],
    format: OpenApiSourceFormat,
) -> Result<Value, OpenApiImportError> {
    match format {
        OpenApiSourceFormat::Json => {
            parse_without_duplicate_keys(bytes).map_err(|_| OpenApiImportError::InvalidSource)
        }
        OpenApiSourceFormat::Yaml => {
            let value = serde_yaml::from_slice::<serde_yaml::Value>(bytes)
                .map_err(|_| OpenApiImportError::InvalidSource)?;
            serde_json::to_value(value).map_err(|_| OpenApiImportError::InvalidSource)
        }
    }
}

pub(crate) fn lower_parameters(
    path_parameters: &[ReferenceOr<Parameter>],
    operation_parameters: &[ReferenceOr<Parameter>],
    components: Option<&Components>,
    resolver: &mut SchemaResolver<'_>,
) -> Result<Vec<ArgumentDefinition>, SchemaFailure> {
    #[derive(Debug)]
    struct LoweredParameter {
        name: String,
        location: ArgumentLocation,
        argument_type: ArgumentType,
        enum_values: Vec<String>,
        required: bool,
    }

    let mut values = BTreeMap::<(String, &'static str), LoweredParameter>::new();
    let mut add = |parameters: &[ReferenceOr<Parameter>], allow_override: bool| {
        let mut local = BTreeSet::new();
        for parameter in parameters {
            let parameter = resolve_parameter(parameter, components)?;
            let (data, kind, location) = match &parameter {
                Parameter::Query {
                    parameter_data,
                    style,
                    allow_reserved,
                    ..
                } => {
                    if *style != QueryStyle::Form
                        || *allow_reserved
                        || parameter_data.explode.is_some()
                    {
                        return Err(SchemaFailure("parameter_style_unsupported"));
                    }
                    (parameter_data, "query", ArgumentLocation::Query)
                }
                Parameter::Path {
                    parameter_data,
                    style,
                    ..
                } => {
                    if *style != PathStyle::Simple
                        || parameter_data.explode.is_some()
                        || !parameter_data.required
                    {
                        return Err(SchemaFailure("parameter_style_unsupported"));
                    }
                    (parameter_data, "path", ArgumentLocation::Path)
                }
                Parameter::Header { .. } => {
                    return Err(SchemaFailure("header_parameter_unsupported"));
                }
                Parameter::Cookie { .. } => {
                    return Err(SchemaFailure("cookie_parameter_unsupported"));
                }
            };
            let key = (data.name.clone(), kind);
            if !local.insert(key.clone()) {
                return Err(SchemaFailure("duplicate_parameter"));
            }
            if !allow_override && values.contains_key(&key) {
                return Err(SchemaFailure("duplicate_parameter"));
            }
            let ParameterSchemaOrContent::Schema(schema) = &data.format else {
                return Err(SchemaFailure("parameter_content_unsupported"));
            };
            let (argument_type, enum_values) = resolver.primitive(schema)?;
            if kind == "path" && argument_type == ArgumentType::StringArray {
                return Err(SchemaFailure("path_array_unsupported"));
            }
            if !valid_id(&data.name) {
                return Err(SchemaFailure("parameter_name_invalid"));
            }
            values.insert(
                key,
                LoweredParameter {
                    name: data.name.clone(),
                    location,
                    argument_type,
                    enum_values,
                    required: data.required || location == ArgumentLocation::Path,
                },
            );
        }
        Ok(())
    };
    add(path_parameters, false)?;
    add(operation_parameters, true)?;

    let mut arguments = Vec::with_capacity(values.len());
    for (_, parameter) in values {
        arguments.push(ArgumentDefinition {
            name: parameter.name,
            source: ArgumentSource::ModelInput,
            location: parameter.location,
            argument_type: parameter.argument_type,
            required: parameter.required,
            enum_values: parameter.enum_values,
        });
    }
    Ok(arguments)
}

pub(crate) fn lower_request_body(
    request_body: &ReferenceOr<RequestBody>,
    components: Option<&Components>,
    resolver: &mut SchemaResolver<'_>,
) -> Result<Vec<ArgumentDefinition>, SchemaFailure> {
    let request_body = resolve_request_body(request_body, components)?;
    if request_body.required
        && request_body
            .content
            .get("application/json")
            .and_then(|media| media.schema.as_ref())
            .is_none()
    {
        return Err(SchemaFailure("required_body_unsupported"));
    }
    if request_body.content.len() != 1 {
        return Err(SchemaFailure("request_media_type_unsupported"));
    }
    let Some(media) = request_body.content.get("application/json") else {
        return Err(SchemaFailure("request_media_type_unsupported"));
    };
    let Some(schema) = &media.schema else {
        return Err(SchemaFailure("request_schema_missing"));
    };
    let properties = resolver.object_properties(schema)?;
    if properties.is_empty() {
        return Err(SchemaFailure("request_body_properties_empty"));
    }
    if request_body.required && !properties.iter().any(|(_, _, _, required)| *required) {
        return Err(SchemaFailure("required_body_unsupported"));
    }
    properties
        .into_iter()
        .map(|(name, argument_type, enum_values, required)| {
            if !valid_id(&name) {
                return Err(SchemaFailure("body_property_name_invalid"));
            }
            Ok(ArgumentDefinition {
                name,
                source: ArgumentSource::ModelInput,
                location: ArgumentLocation::JsonBody,
                argument_type,
                required,
                enum_values,
            })
        })
        .collect()
}

fn resolve_parameter(
    parameter: &ReferenceOr<Parameter>,
    components: Option<&Components>,
) -> Result<Parameter, SchemaFailure> {
    let mut current = parameter;
    let mut seen = BTreeSet::new();
    loop {
        match current {
            ReferenceOr::Item(parameter) => return Ok(parameter.clone()),
            ReferenceOr::Reference { reference } => {
                let name = component_name(reference, "parameters")?;
                if !seen.insert(name.to_string()) {
                    return Err(SchemaFailure("recursive_parameter"));
                }
                current = components
                    .and_then(|components| components.parameters.get(name))
                    .ok_or(SchemaFailure("unresolved_local_ref"))?;
            }
        }
    }
}

fn resolve_request_body(
    request_body: &ReferenceOr<RequestBody>,
    components: Option<&Components>,
) -> Result<RequestBody, SchemaFailure> {
    let mut current = request_body;
    let mut seen = BTreeSet::new();
    loop {
        match current {
            ReferenceOr::Item(request_body) => return Ok(request_body.clone()),
            ReferenceOr::Reference { reference } => {
                let name = component_name(reference, "requestBodies")?;
                if !seen.insert(name.to_string()) {
                    return Err(SchemaFailure("recursive_request_body"));
                }
                current = components
                    .and_then(|components| components.request_bodies.get(name))
                    .ok_or(SchemaFailure("unresolved_local_ref"))?;
            }
        }
    }
}

pub(crate) fn normalize_servers(
    document: &OpenAPI,
    diagnostics: &mut Vec<OpenApiDiagnostic>,
) -> Vec<String> {
    if document.servers.len() > 1 {
        diagnostic(
            diagnostics,
            OpenApiDiagnosticSeverity::Error,
            "multiple_servers",
            "/servers",
        );
    }
    document
        .servers
        .iter()
        .filter_map(|server| {
            if server
                .variables
                .as_ref()
                .is_some_and(|values| !values.is_empty())
                || server.url.contains('{')
            {
                diagnostic(
                    diagnostics,
                    OpenApiDiagnosticSeverity::Error,
                    "server_variables_unsupported",
                    "/servers",
                );
                return None;
            }
            let Ok(parsed) = Url::parse(&server.url) else {
                diagnostic(
                    diagnostics,
                    OpenApiDiagnosticSeverity::Error,
                    "server_url_invalid",
                    "/servers",
                );
                return None;
            };
            if parsed.scheme() != "https"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.port() == Some(0)
            {
                diagnostic(
                    diagnostics,
                    OpenApiDiagnosticSeverity::Error,
                    "server_url_invalid",
                    "/servers",
                );
                return None;
            }
            sanitize_text(&server.url)
        })
        .collect()
}

pub(crate) fn inspect_raw(value: &Value, diagnostics: &mut Vec<OpenApiDiagnostic>, path: String) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref")
                && !reference
                    .as_str()
                    .is_some_and(|reference| reference.starts_with("#/"))
            {
                diagnostic(
                    diagnostics,
                    OpenApiDiagnosticSeverity::Error,
                    "external_ref_unsupported",
                    &path,
                );
            }
            if object.contains_key("webhooks") {
                diagnostic(
                    diagnostics,
                    OpenApiDiagnosticSeverity::Warning,
                    "webhooks_unsupported",
                    &path,
                );
            }
            for (key, child) in object {
                inspect_raw(child, diagnostics, format!("{path}/{}", safe_segment(key)));
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                inspect_raw(child, diagnostics, format!("{path}/{index}"));
            }
        }
        _ => {}
    }
}

pub(crate) fn supported_method(method: &str) -> Option<crate::HttpMethod> {
    match method {
        "get" => Some(crate::HttpMethod::Get),
        "post" => Some(crate::HttpMethod::Post),
        "put" => Some(crate::HttpMethod::Put),
        "patch" => Some(crate::HttpMethod::Patch),
        "delete" => Some(crate::HttpMethod::Delete),
        _ => None,
    }
}

pub(crate) fn normalized_operation_id(
    source: Option<&str>,
    method: &str,
    path: &str,
) -> Option<String> {
    let source = source.filter(|source| !source.is_empty());
    let raw = source
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{method}_{path}"));
    let mut normalized = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    normalized = normalized
        .chars()
        .take(96)
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    valid_id(&normalized).then_some(normalized)
}

pub(crate) fn validate_source_reference(value: &str) -> Result<(), OpenApiImportError> {
    if value.is_empty()
        || value.len() > MAX_SOURCE_REFERENCE_BYTES
        || value.trim() != value
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(OpenApiImportError::InvalidSource);
    }
    Ok(())
}

pub(crate) fn sanitize_text(value: &str) -> Option<String> {
    let mut result = value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}'
                )
        })
        .collect::<String>();
    if result.trim().is_empty() {
        return None;
    }
    result = result.trim().to_string();
    if result.len() > MAX_TEXT_BYTES {
        let mut end = MAX_TEXT_BYTES;
        while !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
    }
    Some(result)
}

fn safe_segment(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        .take(64)
        .collect::<String>()
}

pub(crate) fn location(path: &str, method: Option<&str>) -> String {
    let path = safe_segment(path);
    method.map_or_else(
        || format!("/paths/{path}"),
        |method| format!("/paths/{path}/{method}"),
    )
}

pub(crate) fn diagnostic(
    diagnostics: &mut Vec<OpenApiDiagnostic>,
    severity: OpenApiDiagnosticSeverity,
    code: &str,
    location: &str,
) {
    diagnostics.push(OpenApiDiagnostic {
        severity,
        code: code.to_string(),
        location: location.to_string(),
    });
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

pub(crate) fn path_arguments_match(path: &str, arguments: &[ArgumentDefinition]) -> bool {
    let mut placeholders = BTreeSet::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return false;
        };
        let name = &after[..end];
        if !valid_id(name) || !placeholders.insert(name.to_string()) {
            return false;
        }
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return false;
    }
    let path_arguments = arguments
        .iter()
        .filter(|argument| argument.location == ArgumentLocation::Path)
        .map(|argument| argument.name.as_str())
        .collect::<BTreeSet<_>>();
    placeholders
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        == path_arguments
}

pub(crate) fn is_openapi_3_0(version: &str) -> bool {
    let mut parts = version.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some("3"), Some("0"), Some(patch), None)
            if !patch.is_empty() && patch.bytes().all(|byte| byte.is_ascii_digit())
    )
}

pub(crate) fn within_depth(value: &Value, depth: usize) -> bool {
    if depth > MAX_OPENAPI_DEPTH {
        return false;
    }
    match value {
        Value::Array(values) => values.iter().all(|value| within_depth(value, depth + 1)),
        Value::Object(values) => values.values().all(|value| within_depth(value, depth + 1)),
        _ => true,
    }
}
