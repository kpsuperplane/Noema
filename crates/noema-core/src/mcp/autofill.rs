use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::{
    McpToolRecord, McpTrustClassification, OwnerExtractor, OwnerExtractorSource,
    TrustedIdentitySelectorKind,
};

const MAX_TOOL_DESCRIPTION_HINT_CHARS: usize = 96;

/// Validated advisory calibration suggestion for a discovered MCP tool.
#[derive(Clone, Debug, PartialEq)]
pub struct McpToolCalibrationSuggestion {
    /// Durable MCP tool id this suggestion applies to.
    pub mcp_tool_id: String,
    /// Suggested read trust classification.
    pub read_classification: McpTrustClassification,
    /// Suggested write trust classification.
    pub write_classification: McpTrustClassification,
    /// Suggested export trust classification.
    pub export_classification: McpTrustClassification,
    /// Suggested deterministic ownership extractors.
    pub owner_extractors: Vec<OwnerExtractor>,
    /// Optional disabled-state suggestion; omitted output preserves the current draft.
    pub disabled: Option<bool>,
}

/// Errors returned while parsing or validating MCP autofill model output.
#[derive(Debug, Error)]
pub enum McpAutofillError {
    /// Model output was not valid strict JSON for the expected shape.
    #[error("invalid autofill JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Model output referenced a tool name outside the persisted metadata set.
    #[error("unknown MCP tool name in autofill response: {0}")]
    UnknownToolName(String),
    /// Model output repeated a known tool name.
    #[error("duplicate MCP tool name in autofill response: {0}")]
    DuplicateToolName(String),
    /// Model output used an invalid trust classification value.
    #[error("invalid {field}: expected one of none, trusted, untrusted, mixed")]
    InvalidClassification {
        /// Name of the invalid classification field.
        field: &'static str,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutofillResponse {
    suggestions: Vec<RawSuggestion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSuggestion {
    tool: String,
    #[serde(rename = "read", alias = "r", alias = "read_classification")]
    read_classification: String,
    #[serde(rename = "write", alias = "w", alias = "write_classification")]
    write_classification: String,
    #[serde(rename = "export", alias = "e", alias = "export_classification")]
    export_classification: String,
    #[serde(default, rename = "owner_extractors")]
    _owner_extractors: Vec<Value>,
    #[serde(rename = "d", alias = "disabled")]
    disabled: Option<bool>,
}

/// Build the metadata-only prompt used to request MCP calibration suggestions.
#[must_use]
pub fn build_autofill_prompt(server_name: &str, tools: &[McpToolRecord]) -> String {
    let tool_text = tools
        .iter()
        .map(format_tool_prompt_row)
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Classify MCP tools for server "{server_name}" from metadata only.
Return JSON only: {{"suggestions":[{{"tool":"name","read":"n|t|u|m","write":"n|t|u|m","export":"n|t|u|m","d":false}}]}}
read=MCP data into Noema; write=mutate MCP destination; export=share beyond MCP destination.
Writes/creates inside the same MCP destination are not export.
n=none; t=trusted own/private/local/same destination; u=untrusted public web/arbitrary URL/recipient/outside party; m=mixed/runtime-dependent.
Return d:false unless metadata says the tool itself is unsafe/deprecated. Writing is not a reason to disable. No owner extractors.
tool	hint	in	out	ann
{tool_text}"#
    )
}

fn format_tool_prompt_row(tool: &McpToolRecord) -> String {
    let hint =
        compact_description_hint(tool.description.as_deref(), MAX_TOOL_DESCRIPTION_HINT_CHARS);
    let output_fields = tool
        .output_schema
        .as_ref()
        .map(schema_field_names)
        .unwrap_or_default();
    [
        sanitize_prompt_line(&tool.name),
        hint.map(|hint| sanitize_prompt_line(&hint))
            .unwrap_or_else(|| "-".to_string()),
        field_list_or_dash(schema_field_names(&tool.input_schema)),
        field_list_or_dash(output_fields),
        format_annotations(&tool.annotations).unwrap_or_else(|| "-".to_string()),
    ]
    .join("\t")
}

fn schema_field_names(schema: &Value) -> Vec<String> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    properties
        .keys()
        .map(|name| sanitize_prompt_line(name))
        .collect()
}

fn field_list_or_dash(fields: Vec<String>) -> String {
    if fields.is_empty() {
        "-".to_string()
    } else {
        fields.join(",")
    }
}

fn format_annotations(annotations: &Value) -> Option<String> {
    match annotations {
        Value::Object(values) if values.is_empty() => None,
        Value::Object(values) => Some(
            values
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        sanitize_prompt_line(key),
                        sanitize_annotation_value(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Value::Null => None,
        value => Some(sanitize_annotation_value(value)),
    }
}

fn sanitize_annotation_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => sanitize_prompt_line(value),
        value => serde_json::to_string(value)
            .map(|value| sanitize_prompt_line(&value))
            .unwrap_or_else(|_| "unknown".to_string()),
    }
}

fn sanitize_prompt_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_description_hint(description: Option<&str>, max_chars: usize) -> Option<String> {
    let description = description?;
    let cleaned = description.trim();
    if cleaned.is_empty() {
        return None;
    }

    let without_examples = cleaned
        .split_once("<example")
        .map_or(cleaned, |(before_examples, _)| before_examples)
        .trim();
    let first_line = without_examples
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(without_examples);
    let first_sentence = first_line
        .split_once(". ")
        .map_or(first_line, |(sentence, _)| sentence);
    let mut hint = first_sentence.trim().to_string();
    if first_line.len() > hint.len() && !hint.ends_with('.') {
        hint.push('.');
    }

    Some(truncate_chars(&hint, max_chars))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

/// Discover conservative owner extractors from schema metadata.
///
/// The crawler only inspects scalar fields at `/field` and `/object/field`,
/// never traverses arrays, and never infers identities from opaque ids.
#[must_use]
pub fn deterministic_owner_extractors(tool: &McpToolRecord) -> Vec<OwnerExtractor> {
    let mut extractors = Vec::new();
    collect_schema_owner_extractors(
        OwnerExtractorSource::Arguments,
        &tool.input_schema,
        &mut extractors,
    );
    if let Some(output_schema) = &tool.output_schema {
        collect_schema_owner_extractors(
            OwnerExtractorSource::StructuredContent,
            output_schema,
            &mut extractors,
        );
    }
    extractors.sort_by(|left, right| {
        let left_source = extractor_source_sort_key(left.source);
        let right_source = extractor_source_sort_key(right.source);
        left_source
            .cmp(&right_source)
            .then_with(|| pointer_depth(&left.path).cmp(&pointer_depth(&right.path)))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| {
                left.selector_kind
                    .as_str()
                    .cmp(right.selector_kind.as_str())
            })
    });
    extractors.dedup();
    extractors
}

fn collect_schema_owner_extractors(
    source: OwnerExtractorSource,
    schema: &Value,
    extractors: &mut Vec<OwnerExtractor>,
) {
    collect_schema_owner_extractors_at_path(source, schema, &[], extractors);
}

fn collect_schema_owner_extractors_at_path(
    source: OwnerExtractorSource,
    schema: &Value,
    path_segments: &[&str],
    extractors: &mut Vec<OwnerExtractor>,
) {
    if schema_type_is(schema, "array") {
        return;
    }

    if !path_segments.is_empty()
        && path_segments.len() <= 2
        && is_scalar_schema(schema)
        && let Some(selector_kind) = selector_kind_for_field(path_segments, schema)
    {
        extractors.push(OwnerExtractor {
            source,
            selector_kind,
            path: json_pointer(path_segments),
        });
    }

    if !path_segments.is_empty() && !schema_is_object_like(schema) {
        return;
    }
    if path_segments.len() >= 2 {
        return;
    }

    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, property_schema) in properties {
        let mut child_path = Vec::with_capacity(path_segments.len() + 1);
        child_path.extend_from_slice(path_segments);
        child_path.push(name.as_str());
        collect_schema_owner_extractors_at_path(source, property_schema, &child_path, extractors);
    }
}

fn is_scalar_schema(schema: &Value) -> bool {
    !schema_type_is(schema, "array") && !schema_is_object_like(schema)
}

fn schema_is_object_like(schema: &Value) -> bool {
    schema_type_is(schema, "object")
        || schema
            .get("properties")
            .and_then(Value::as_object)
            .is_some()
}

fn selector_kind_for_field(
    path_segments: &[&str],
    schema: &Value,
) -> Option<TrustedIdentitySelectorKind> {
    let terminal = path_segments.last()?;
    let role_segments = &path_segments[..path_segments.len().saturating_sub(1)];
    let format = schema.get("format").and_then(Value::as_str).unwrap_or("");
    if format.eq_ignore_ascii_case("email") || field_name_has_token(terminal, "email") {
        return Some(TrustedIdentitySelectorKind::Email);
    }
    if format.eq_ignore_ascii_case("phone") || field_name_has_any_token(terminal, &["phone", "tel"])
    {
        return Some(TrustedIdentitySelectorKind::Phone);
    }
    if (field_name_has_token(terminal, "domain") || format.eq_ignore_ascii_case("hostname"))
        && path_has_owner_role(role_segments)
    {
        return Some(TrustedIdentitySelectorKind::Domain);
    }
    None
}

fn path_has_owner_role(path_segments: &[&str]) -> bool {
    path_segments.is_empty()
        || path_segments
            .iter()
            .any(|segment| field_name_has_any_token(segment, OWNER_ROLE_TOKENS))
}

const OWNER_ROLE_TOKENS: &[&str] = &[
    "owner",
    "creator",
    "author",
    "account",
    "user",
    "workspace",
    "organization",
    "organisation",
    "tenant",
    "created",
];

fn field_name_has_any_token(field_name: &str, tokens: &[&str]) -> bool {
    tokens
        .iter()
        .any(|token| field_name_has_token(field_name, token))
}

fn field_name_has_token(field_name: &str, token: &str) -> bool {
    field_name_tokens(field_name)
        .iter()
        .any(|part| part == &token.to_ascii_lowercase())
}

fn field_name_tokens(field_name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in field_name.chars() {
        if !character.is_ascii_alphanumeric() {
            push_field_name_token(&mut tokens, &mut current);
            continue;
        }
        if character.is_ascii_uppercase()
            && current
                .chars()
                .last()
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            push_field_name_token(&mut tokens, &mut current);
        }
        current.push(character.to_ascii_lowercase());
    }
    push_field_name_token(&mut tokens, &mut current);
    tokens
}

fn push_field_name_token(tokens: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    tokens.push(std::mem::take(current));
}

fn schema_type_is(schema: &Value, expected: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(value)) => value == expected,
        Some(Value::Array(values)) => values.iter().any(|value| value.as_str() == Some(expected)),
        _ => false,
    }
}

fn json_pointer(path_segments: &[&str]) -> String {
    let mut pointer = String::new();
    for segment in path_segments {
        pointer.push('/');
        pointer.push_str(&escape_json_pointer_segment(segment));
    }
    pointer
}

fn escape_json_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn pointer_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

const fn extractor_source_sort_key(source: OwnerExtractorSource) -> u8 {
    match source {
        OwnerExtractorSource::Arguments => 0,
        OwnerExtractorSource::StructuredContent => 1,
        OwnerExtractorSource::Metadata => 2,
        OwnerExtractorSource::ResourceUri => 3,
        OwnerExtractorSource::BuiltInAdapter => 4,
    }
}

/// Parse and validate model-produced MCP calibration suggestions.
///
/// # Errors
///
/// Returns an error when the model output is not strict JSON, references an
/// unknown or duplicate tool name, or uses invalid enum strings.
pub fn parse_autofill_response(
    text: &str,
    tools: &[McpToolRecord],
) -> Result<Vec<McpToolCalibrationSuggestion>, McpAutofillError> {
    let response: AutofillResponse = serde_json::from_str(text.trim())?;
    let tools_by_name = tools
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    let mut seen_tool_names = BTreeSet::new();
    let mut suggestions = Vec::with_capacity(response.suggestions.len());

    for suggestion in response.suggestions {
        if !seen_tool_names.insert(suggestion.tool.clone()) {
            return Err(McpAutofillError::DuplicateToolName(suggestion.tool));
        }
        let tool = tools_by_name
            .get(suggestion.tool.as_str())
            .ok_or_else(|| McpAutofillError::UnknownToolName(suggestion.tool.clone()))?;
        let owner_extractors = deterministic_owner_extractors(tool);
        suggestions.push(McpToolCalibrationSuggestion {
            mcp_tool_id: tool.mcp_tool_id.clone(),
            read_classification: parse_classification(
                &suggestion.read_classification,
                "read_classification",
            )?,
            write_classification: parse_classification(
                &suggestion.write_classification,
                "write_classification",
            )?,
            export_classification: parse_classification(
                &suggestion.export_classification,
                "export_classification",
            )?,
            owner_extractors,
            disabled: suggestion.disabled,
        });
    }

    Ok(suggestions)
}

fn parse_classification(
    value: &str,
    field: &'static str,
) -> Result<McpTrustClassification, McpAutofillError> {
    match value {
        "n" | "none" => Ok(McpTrustClassification::None),
        "t" | "trusted" => Ok(McpTrustClassification::Trusted),
        "u" | "untrusted" => Ok(McpTrustClassification::Untrusted),
        "m" | "mixed" => Ok(McpTrustClassification::Mixed),
        _ => Err(McpAutofillError::InvalidClassification { field }),
    }
}

#[cfg(test)]
mod tests;
