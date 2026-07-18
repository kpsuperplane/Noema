//! Pure MCP calibration-autofill prompt construction and response validation.

#[cfg(any(feature = "transport", test))]
use std::collections::{BTreeMap, BTreeSet};

#[cfg(any(feature = "transport", test))]
use serde::Deserialize;
#[cfg(any(feature = "transport", test))]
use serde_json::Value;
#[cfg(any(feature = "transport", test))]
use thiserror::Error;

use crate::McpTrustClassification;
#[cfg(any(feature = "transport", test))]
use crate::{
    McpToolRecord,
    eligibility::{prompt_safe_mcp_tool_description, sanitize_prompt_line},
};

#[cfg(any(feature = "transport", test))]
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
    /// Optional disabled-state suggestion; omitted output preserves the current draft.
    pub disabled: Option<bool>,
}

/// Errors returned while parsing or validating MCP autofill model output.
#[derive(Debug, Error)]
#[cfg(any(feature = "transport", test))]
pub(crate) enum McpAutofillError {
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
#[cfg(any(feature = "transport", test))]
#[serde(deny_unknown_fields)]
struct AutofillResponse {
    suggestions: Vec<RawSuggestion>,
}

#[derive(Debug, Deserialize)]
#[cfg(any(feature = "transport", test))]
#[serde(deny_unknown_fields)]
struct RawSuggestion {
    tool: String,
    #[serde(rename = "read", alias = "r", alias = "read_classification")]
    read_classification: String,
    #[serde(rename = "write", alias = "w", alias = "write_classification")]
    write_classification: String,
    #[serde(rename = "export", alias = "e", alias = "export_classification")]
    export_classification: String,
    #[serde(rename = "d", alias = "disabled")]
    disabled: Option<bool>,
}

/// Build the metadata-only prompt used to request MCP calibration suggestions.
#[must_use]
#[cfg(any(feature = "transport", test))]
pub(crate) fn build_autofill_prompt(server_name: &str, tools: &[McpToolRecord]) -> String {
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
Return d:false unless metadata says the tool itself is unsafe/deprecated. Writing is not a reason to disable.
tool	hint	in	out	ann
{tool_text}"#
    )
}

#[cfg(any(feature = "transport", test))]
fn format_tool_prompt_row(tool: &McpToolRecord) -> String {
    let hint = prompt_safe_mcp_tool_description(
        tool.description.as_deref(),
        MAX_TOOL_DESCRIPTION_HINT_CHARS,
    );
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

#[cfg(any(feature = "transport", test))]
fn schema_field_names(schema: &Value) -> Vec<String> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    properties
        .keys()
        .map(|name| sanitize_prompt_line(name))
        .collect()
}

#[cfg(any(feature = "transport", test))]
fn field_list_or_dash(fields: Vec<String>) -> String {
    if fields.is_empty() {
        "-".to_string()
    } else {
        fields.join(",")
    }
}

#[cfg(any(feature = "transport", test))]
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

#[cfg(any(feature = "transport", test))]
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

/// Parse and validate model-produced MCP calibration suggestions.
///
/// # Errors
///
/// Returns an error when the model output is not strict JSON, references an
/// unknown or duplicate tool name, or uses invalid enum strings.
#[cfg(any(feature = "transport", test))]
pub(crate) fn parse_autofill_response(
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
            disabled: suggestion.disabled,
        });
    }

    Ok(suggestions)
}

#[cfg(any(feature = "transport", test))]
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
mod tests {
    use serde_json::json;

    use super::{build_autofill_prompt, parse_autofill_response};
    use crate::{McpToolRecord, McpTrustClassification};

    #[test]
    fn parses_valid_autofill_response_for_known_tools() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let response = json!({"suggestions": [{
            "tool": "read_doc", "read": "m", "write": "n", "export": "n", "d": false
        }]});
        let suggestions =
            parse_autofill_response(&response.to_string(), &tools).expect("suggestions");

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].mcp_tool_id, "mcp_tool:docs:read");
        assert_eq!(
            suggestions[0].read_classification,
            McpTrustClassification::Mixed
        );
        assert_eq!(suggestions[0].disabled, Some(false));
        let without_disabled = json!({"suggestions": [{
            "tool": "read_doc", "read": "m", "write": "n", "export": "n"
        }]});
        assert_eq!(
            parse_autofill_response(&without_disabled.to_string(), &tools).expect("suggestions")[0]
                .disabled,
            None
        );
    }

    #[test]
    fn rejects_unknown_tool_name_without_partial_suggestions() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let valid = json!({"tool": "read_doc", "read": "m", "write": "n", "export": "n"});
        let cases = [
            (
                json!({"suggestions": [{
                    "tool": "missing_doc", "read": "m", "write": "n", "export": "n"
                }]}),
                "unknown MCP tool name",
            ),
            (
                json!({"suggestions": [valid.clone(), valid.clone()]}),
                "duplicate MCP tool name",
            ),
            (
                json!({"suggestions": [{
                    "tool": "read_doc", "read": "Mixed", "write": "n", "export": "n"
                }]}),
                "invalid read_classification",
            ),
        ];

        for (response, expected) in cases {
            let error = parse_autofill_response(&response.to_string(), &tools)
                .expect_err("invalid output rejected");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn prompt_compacts_long_tool_and_field_descriptions() {
        let mut tool = test_tool("mcp_tool:notion:update-page", "notion-update-page");
        tool.description = Some(format!(
            "Update a Notion page. {}\n<example>{}</example>",
            "Long operational guidance. ".repeat(80),
            "Example payload noise. ".repeat(80)
        ));
        tool.input_schema = json!({
            "type": "object",
            "properties": {
                "page_id": {
                    "type": "string",
                    "description": "The ID of the page to update, with or without dashes."
                },
                "recipient_email": {
                    "type": "string",
                    "description": format!(
                        "Email address for the recipient owner. {}",
                        "Verbose examples that should not ride along. ".repeat(40)
                    )
                }
            }
        });

        let prompt = build_autofill_prompt("Notion", &[tool]);

        assert!(prompt.len() < 1_250, "prompt was {} chars", prompt.len());
        assert!(prompt.contains("tool\thint\tin\tout\tann"));
        assert!(prompt.contains("notion-update-page"));
        assert!(prompt.contains("Update a Notion page."));
        assert!(prompt.contains("page_id,recipient_email"));
        assert!(!prompt.contains("Example payload noise"));
        assert!(!prompt.contains("Verbose examples that should not ride along"));
    }

    fn test_tool(mcp_tool_id: &str, name: &str) -> McpToolRecord {
        McpToolRecord {
            mcp_tool_id: mcp_tool_id.to_string(),
            mcp_server_id: "mcp_server:docs".to_string(),
            name: name.to_string(),
            description: Some("Read a document".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "owner_email": { "type": "string" }
                }
            }),
            output_schema: None,
            annotations: json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint_1".to_string(),
            discovered_at: "2026-07-01T00:00:00Z".to_string(),
        }
    }
}
