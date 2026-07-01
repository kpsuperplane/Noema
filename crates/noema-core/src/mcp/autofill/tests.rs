use serde_json::json;

use super::{build_autofill_prompt, deterministic_owner_extractors, parse_autofill_response};
use crate::{
    McpToolRecord, McpTrustClassification, OwnerExtractor, OwnerExtractorSource,
    TrustedIdentitySelectorKind,
};

#[test]
fn parses_valid_autofill_response_for_known_tools() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "tool": "read_doc",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none",
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
        "tool": "read_doc",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none"
      }]
    }"#;

    let suggestions = parse_autofill_response(response, &tools).expect("suggestions");

    assert_eq!(suggestions[0].disabled, None);
}

#[test]
fn rejects_unknown_tool_name_without_partial_suggestions() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "tool": "missing_doc",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none",
        "disabled": false
      }]
    }"#;

    let error = parse_autofill_response(response, &tools).expect_err("unknown tool rejected");

    assert!(error.to_string().contains("unknown MCP tool name"));
}

#[test]
fn rejects_duplicate_tool_name_without_partial_suggestions() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "tool": "read_doc",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none"
      }, {
        "tool": "read_doc",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none"
      }]
    }"#;

    let error = parse_autofill_response(response, &tools).expect_err("duplicate tool rejected");

    assert!(error.to_string().contains("duplicate MCP tool name"));
}

#[test]
fn rejects_invalid_enum() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "tool": "read_doc",
        "read_classification": "Mixed",
        "write_classification": "none",
        "export_classification": "none",
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
    assert!(prompt.contains("read_doc: Read a document"));
    assert!(prompt.contains("Inputs: owner_email"));
    assert!(prompt.contains("Annotations: readOnlyHint=true"));
    assert!(!prompt.contains("mcp_tool:docs:read"));
}

#[test]
fn prompt_limits_model_to_trust_classification_rubric() {
    let prompt = build_autofill_prompt("Docs", &[test_tool("mcp_tool:docs:read", "read_doc")]);

    assert!(prompt.contains("do not return extractor"));
    assert!(!prompt.contains("owner_extractors"));
    assert!(prompt.contains(r#""tool":"...""#));
    assert!(!prompt.contains(r#""mcp_tool_id":"...""#));
    assert!(prompt.contains("trusted:"));
    assert!(prompt.contains("untrusted:"));
    assert!(prompt.contains("public web"));
    assert!(prompt.contains("authenticated user's own"));
}

#[test]
fn prompt_omits_annotations_when_empty() {
    let mut tool = test_tool("mcp_tool:docs:read", "read_doc");
    tool.annotations = json!({});

    let prompt = build_autofill_prompt("Docs", &[tool]);

    assert!(!prompt.contains("Annotations:"));
}

#[test]
fn deterministic_extractors_use_shallow_scalar_owner_identity_fields() {
    let mut tool = test_tool("mcp_tool:docs:read", "read_doc");
    tool.input_schema = json!({
        "type": "object",
        "properties": {
            "owner_email": { "type": "string", "format": "email" },
            "workspace": {
                "type": "object",
                "properties": {
                    "domain": { "type": "string" }
                }
            },
            "created_by": {
                "type": "object",
                "properties": {
                    "phone": { "type": "string" }
                }
            },
            "page_id": { "type": "string" },
            "owners": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "email": { "type": "string", "format": "email" }
                    }
                }
            },
            "result": {
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "object",
                        "properties": {
                            "email": { "type": "string", "format": "email" }
                        }
                    }
                }
            }
        }
    });

    let extractors = deterministic_owner_extractors(&tool);

    assert_eq!(
        extractors,
        vec![
            owner_extractor(
                OwnerExtractorSource::Arguments,
                TrustedIdentitySelectorKind::Email,
                "/owner_email"
            ),
            owner_extractor(
                OwnerExtractorSource::Arguments,
                TrustedIdentitySelectorKind::Phone,
                "/created_by/phone"
            ),
            owner_extractor(
                OwnerExtractorSource::Arguments,
                TrustedIdentitySelectorKind::Domain,
                "/workspace/domain"
            ),
        ]
    );
}

#[test]
fn deterministic_extractors_read_structured_output_and_camel_case_fields() {
    let mut tool = test_tool("mcp_tool:docs:fetch", "fetch_doc");
    tool.input_schema = json!({
        "type": "object",
        "properties": {
            "doc_id": { "type": "string" }
        }
    });
    tool.output_schema = Some(json!({
        "type": "object",
        "properties": {
            "createdBy": {
                "type": "object",
                "properties": {
                    "emailAddress": { "type": "string" }
                }
            },
            "workspaceDomain": { "type": "string" },
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "ownerEmail": { "type": "string" }
                    }
                }
            }
        }
    }));

    let extractors = deterministic_owner_extractors(&tool);

    assert_eq!(
        extractors,
        vec![
            owner_extractor(
                OwnerExtractorSource::StructuredContent,
                TrustedIdentitySelectorKind::Domain,
                "/workspaceDomain"
            ),
            owner_extractor(
                OwnerExtractorSource::StructuredContent,
                TrustedIdentitySelectorKind::Email,
                "/createdBy/emailAddress"
            ),
        ]
    );
}

#[test]
fn parse_ignores_model_extractors_and_uses_deterministic_extractors() {
    let mut tool = test_tool("mcp_tool:notion:update", "notion-update-page");
    tool.input_schema = json!({
        "type": "object",
        "properties": {
            "owner": {
                "type": "object",
                "properties": {
                    "email": { "type": "string", "format": "email" }
                }
            },
            "page_id": { "type": "string" }
        }
    });
    let response = r#"{
      "suggestions": [{
        "tool": "notion-update-page",
        "read_classification": "none",
        "write_classification": "mixed",
        "export_classification": "none",
        "owner_extractors": [{
          "source": "arguments",
          "selector_kind": "domain",
          "path": "/page_id"
        }],
        "disabled": false
      }]
    }"#;

    let suggestions = parse_autofill_response(response, &[tool]).expect("suggestions");

    assert_eq!(
        suggestions[0].owner_extractors,
        vec![owner_extractor(
            OwnerExtractorSource::Arguments,
            TrustedIdentitySelectorKind::Email,
            "/owner/email"
        )]
    );
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

    assert!(prompt.len() < 2_000, "prompt was {} chars", prompt.len());
    assert!(prompt.contains("notion-update-page"));
    assert!(prompt.contains("Update a Notion page."));
    assert!(prompt.contains("Inputs: page_id, recipient_email"));
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

fn owner_extractor(
    source: OwnerExtractorSource,
    selector_kind: TrustedIdentitySelectorKind,
    path: &str,
) -> OwnerExtractor {
    OwnerExtractor {
        source,
        selector_kind,
        path: path.to_string(),
    }
}
