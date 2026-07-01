use std::collections::BTreeSet;

use serde::Deserialize;
use thiserror::Error;

use crate::{
    McpToolRecord, McpTrustClassification, OwnerExtractor, OwnerExtractorSource,
    TrustedIdentitySelectorKind,
};

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
    /// Model output referenced a tool id outside the persisted metadata set.
    #[error("unknown MCP tool id in autofill response: {0}")]
    UnknownToolId(String),
    /// Model output repeated a known tool id.
    #[error("duplicate MCP tool id in autofill response: {0}")]
    DuplicateToolId(String),
    /// Model output used an invalid trust classification value.
    #[error("invalid {field}: expected one of none, trusted, untrusted, mixed")]
    InvalidClassification {
        /// Name of the invalid classification field.
        field: &'static str,
    },
    /// Model output used an invalid owner extractor source.
    #[error("invalid owner extractor source: {0}")]
    InvalidExtractorSource(String),
    /// Model output used an invalid trusted identity selector kind.
    #[error("invalid owner extractor selector kind: {0}")]
    InvalidSelectorKind(String),
    /// Model output returned a blank owner extractor path.
    #[error("owner extractor path cannot be empty")]
    BlankExtractorPath,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutofillResponse {
    suggestions: Vec<RawSuggestion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSuggestion {
    mcp_tool_id: String,
    read_classification: String,
    write_classification: String,
    export_classification: String,
    #[serde(default)]
    owner_extractors: Vec<RawOwnerExtractor>,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOwnerExtractor {
    source: String,
    selector_kind: String,
    path: String,
}

/// Build the metadata-only prompt used to request MCP calibration suggestions.
#[must_use]
pub fn build_autofill_prompt(server_name: &str, tools: &[McpToolRecord]) -> String {
    let tool_payload = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "mcp_tool_id": tool.mcp_tool_id,
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
                "output_schema": tool.output_schema,
                "annotations": tool.annotations,
                "metadata_fingerprint": tool.metadata_fingerprint,
            })
        })
        .collect::<Vec<_>>();
    let tool_json = serde_json::to_string_pretty(&tool_payload)
        .expect("MCP tool metadata should serialize to JSON");

    format!(
        r#"You are Noema's MCP tool calibration assistant.
Return strict JSON only. Do not include Markdown, comments, code fences, or prose.
Classify from persisted metadata only for MCP server "{server_name}".

Definitions:
- read means the tool can bring data from the MCP destination into Noema.
- write means the tool can mutate state inside the MCP destination.
- export means the tool can share information beyond the MCP destination.
- Use none only when the axis clearly does not apply.
- Use mixed when trust or ownership depends on runtime contents.
- Use trusted or untrusted only when metadata makes the trust boundary clear without runtime data.
- Suggest owner_extractors only when a deterministic field exists in the metadata shape.
- If ownership cannot be resolved, return an empty owner_extractors array.
- Include disabled only when you intentionally suggest changing or preserving disabled state.

Return exactly:
{{"suggestions":[{{"mcp_tool_id":"...","read_classification":"none|trusted|untrusted|mixed","write_classification":"none|trusted|untrusted|mixed","export_classification":"none|trusted|untrusted|mixed","owner_extractors":[{{"source":"arguments|structured_content|metadata|resource_uri|built_in_adapter","selector_kind":"email|phone|domain","path":"..."}}],"disabled":false}}]}}

Tools:
{tool_json}"#
    )
}

/// Parse and validate model-produced MCP calibration suggestions.
///
/// # Errors
///
/// Returns an error when the model output is not strict JSON, references an
/// unknown or duplicate tool id, uses invalid enum strings, or includes invalid
/// owner extractors.
pub fn parse_autofill_response(
    text: &str,
    tools: &[McpToolRecord],
) -> Result<Vec<McpToolCalibrationSuggestion>, McpAutofillError> {
    let response: AutofillResponse = serde_json::from_str(text.trim())?;
    let known_tool_ids = tools
        .iter()
        .map(|tool| tool.mcp_tool_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_tool_ids = BTreeSet::new();
    let mut suggestions = Vec::with_capacity(response.suggestions.len());

    for suggestion in response.suggestions {
        if !known_tool_ids.contains(suggestion.mcp_tool_id.as_str()) {
            return Err(McpAutofillError::UnknownToolId(suggestion.mcp_tool_id));
        }
        if !seen_tool_ids.insert(suggestion.mcp_tool_id.clone()) {
            return Err(McpAutofillError::DuplicateToolId(suggestion.mcp_tool_id));
        }
        suggestions.push(McpToolCalibrationSuggestion {
            mcp_tool_id: suggestion.mcp_tool_id,
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
            owner_extractors: suggestion
                .owner_extractors
                .into_iter()
                .map(parse_owner_extractor)
                .collect::<Result<Vec<_>, _>>()?,
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
        "none" => Ok(McpTrustClassification::None),
        "trusted" => Ok(McpTrustClassification::Trusted),
        "untrusted" => Ok(McpTrustClassification::Untrusted),
        "mixed" => Ok(McpTrustClassification::Mixed),
        _ => Err(McpAutofillError::InvalidClassification { field }),
    }
}

fn parse_owner_extractor(raw: RawOwnerExtractor) -> Result<OwnerExtractor, McpAutofillError> {
    let path = raw.path.trim().to_string();
    if path.is_empty() {
        return Err(McpAutofillError::BlankExtractorPath);
    }

    Ok(OwnerExtractor {
        source: parse_extractor_source(&raw.source)?,
        selector_kind: parse_selector_kind(&raw.selector_kind)?,
        path,
    })
}

fn parse_extractor_source(value: &str) -> Result<OwnerExtractorSource, McpAutofillError> {
    match value {
        "arguments" => Ok(OwnerExtractorSource::Arguments),
        "structured_content" => Ok(OwnerExtractorSource::StructuredContent),
        "metadata" => Ok(OwnerExtractorSource::Metadata),
        "resource_uri" => Ok(OwnerExtractorSource::ResourceUri),
        "built_in_adapter" => Ok(OwnerExtractorSource::BuiltInAdapter),
        _ => Err(McpAutofillError::InvalidExtractorSource(value.to_string())),
    }
}

fn parse_selector_kind(value: &str) -> Result<TrustedIdentitySelectorKind, McpAutofillError> {
    match value {
        "email" => Ok(TrustedIdentitySelectorKind::Email),
        "phone" => Ok(TrustedIdentitySelectorKind::Phone),
        "domain" => Ok(TrustedIdentitySelectorKind::Domain),
        _ => Err(McpAutofillError::InvalidSelectorKind(value.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        McpToolRecord, McpTrustClassification,
        mcp::autofill::{build_autofill_prompt, parse_autofill_response},
    };

    #[test]
    fn parses_valid_autofill_response_for_known_tools() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let response = r#"{
          "suggestions": [{
            "mcp_tool_id": "mcp_tool:docs:read",
            "read_classification": "mixed",
            "write_classification": "none",
            "export_classification": "none",
            "owner_extractors": [{
              "source": "arguments",
              "selector_kind": "email",
              "path": "/owner_email"
            }],
            "disabled": false
          }]
        }"#;

        let suggestions = parse_autofill_response(response, &tools).expect("suggestions");

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].mcp_tool_id, "mcp_tool:docs:read");
        assert_eq!(
            suggestions[0].read_classification,
            McpTrustClassification::Mixed
        );
        assert_eq!(suggestions[0].owner_extractors[0].path, "/owner_email");
        assert_eq!(suggestions[0].disabled, Some(false));
    }

    #[test]
    fn parses_missing_disabled_as_no_disabled_suggestion() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let response = r#"{
          "suggestions": [{
            "mcp_tool_id": "mcp_tool:docs:read",
            "read_classification": "mixed",
            "write_classification": "none",
            "export_classification": "none",
            "owner_extractors": []
          }]
        }"#;

        let suggestions = parse_autofill_response(response, &tools).expect("suggestions");

        assert_eq!(suggestions[0].disabled, None);
    }

    #[test]
    fn rejects_unknown_tool_id_without_partial_suggestions() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let response = r#"{
          "suggestions": [{
            "mcp_tool_id": "mcp_tool:docs:missing",
            "read_classification": "mixed",
            "write_classification": "none",
            "export_classification": "none",
            "owner_extractors": [],
            "disabled": false
          }]
        }"#;

        let error = parse_autofill_response(response, &tools).expect_err("unknown tool rejected");

        assert!(error.to_string().contains("unknown MCP tool id"));
    }

    #[test]
    fn rejects_invalid_enum_and_blank_extractor_path() {
        let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
        let response = r#"{
          "suggestions": [{
            "mcp_tool_id": "mcp_tool:docs:read",
            "read_classification": "Mixed",
            "write_classification": "none",
            "export_classification": "none",
            "owner_extractors": [{
              "source": "arguments",
              "selector_kind": "email",
              "path": ""
            }],
            "disabled": false
          }]
        }"#;

        let error = parse_autofill_response(response, &tools).expect_err("invalid output rejected");

        assert!(error.to_string().contains("invalid read_classification"));
    }

    #[test]
    fn prompt_names_trust_axes_and_demands_strict_json() {
        let prompt = build_autofill_prompt("Docs", &[test_tool("mcp_tool:docs:read", "read_doc")]);

        assert!(prompt.contains("Return strict JSON only"));
        assert!(prompt.contains("export means"));
        assert!(prompt.contains("metadata only"));
        assert!(prompt.contains("mcp_tool:docs:read"));
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
