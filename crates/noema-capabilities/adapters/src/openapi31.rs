//! Bounded OpenAPI 3.1 admission into the existing 3.0 proposal authority.
//!
//! This is deliberately a narrow bridge: it accepts the JSON Schema
//! 2020-12 vocabulary only where the existing closed argument lowerer can
//! prove the same primitive/path/query/body contract. Unsupported schema
//! computation is rejected before the 3.0 parser sees the document.

use crate::{
    SourceDigest,
    json_limits::validate_json_shape,
    openapi::{
        MAX_OPENAPI_DEPTH, OpenApiCandidate, OpenApiImportError, OpenApiImporter,
        OpenApiSourceFormat,
    },
    openapi_normalize::within_depth,
};
use serde_json::Value;

const JSON_SCHEMA_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
const OPENAPI_31_BASE_DIALECT: &str = "https://spec.openapis.org/oas/3.1/dialect/base";

pub(crate) fn import(
    source_reference: String,
    bytes: &[u8],
    source_format: OpenApiSourceFormat,
    raw: Value,
    version: &str,
) -> Result<OpenApiCandidate, OpenApiImportError> {
    validate_31(&raw)?;
    let mut normalized = raw;
    let root = normalized
        .as_object_mut()
        .ok_or(OpenApiImportError::InvalidDocument)?;
    root.insert("openapi".to_string(), Value::String("3.0.3".to_string()));
    root.remove("jsonSchemaDialect");
    let normalized_bytes =
        serde_json::to_vec(&normalized).map_err(|_| OpenApiImportError::InvalidDocument)?;
    let mut candidate = OpenApiImporter::import(
        source_reference,
        &normalized_bytes,
        OpenApiSourceFormat::Json,
    )?;
    candidate.source_digest = SourceDigest::compute(bytes);
    candidate.source_format = source_format;
    candidate.version = version.to_string();
    Ok(candidate)
}

fn validate_31(raw: &Value) -> Result<(), OpenApiImportError> {
    let Some(root) = raw.as_object() else {
        return Err(OpenApiImportError::InvalidDocument);
    };
    if root.contains_key("webhooks") {
        return Err(OpenApiImportError::InvalidDocument);
    }
    if let Some(dialect) = root.get("jsonSchemaDialect")
        && !matches!(
            dialect.as_str(),
            Some(JSON_SCHEMA_2020_12) | Some(OPENAPI_31_BASE_DIALECT)
        )
    {
        return Err(OpenApiImportError::InvalidDocument);
    }
    if !validate_json_shape(raw) || !within_depth(raw, 0) {
        return Err(OpenApiImportError::InvalidShape);
    }
    walk_schema_keywords(raw, 0)
}

fn walk_schema_keywords(value: &Value, depth: usize) -> Result<(), OpenApiImportError> {
    if depth > MAX_OPENAPI_DEPTH {
        return Err(OpenApiImportError::InvalidShape);
    }
    let Value::Object(object) = value else {
        if let Value::Array(values) = value {
            for value in values {
                walk_schema_keywords(value, depth + 1)?;
            }
        }
        return Ok(());
    };
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "$dynamicRef"
                | "$dynamicAnchor"
                | "$recursiveRef"
                | "$recursiveAnchor"
                | "$defs"
                | "$id"
                | "$anchor"
                | "unevaluatedProperties"
                | "unevaluatedItems"
                | "dependentSchemas"
                | "dependentRequired"
                | "prefixItems"
                | "contains"
                | "minContains"
                | "maxContains"
                | "propertyNames"
                | "patternProperties"
                | "if"
                | "then"
                | "else"
                | "const"
                | "oneOf"
                | "anyOf"
                | "allOf"
                | "not"
                | "nullable"
        ) {
            return Err(OpenApiImportError::InvalidDocument);
        }
        if key == "$ref"
            && value
                .as_str()
                .is_none_or(|reference| !reference.starts_with("#/components/"))
        {
            return Err(OpenApiImportError::InvalidDocument);
        }
        if key == "type" && value.is_array() {
            return Err(OpenApiImportError::InvalidDocument);
        }
        walk_schema_keywords(value, depth + 1)?;
    }
    Ok(())
}
