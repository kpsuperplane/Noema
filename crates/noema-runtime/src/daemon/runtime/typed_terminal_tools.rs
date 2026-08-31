//! Native terminal contracts for runtime-owned model operations.
//!
//! These calls are deliberately separate from capability dispatch: the model
//! is classifying or proposing data for trusted runtime validation, not asking
//! the runtime to execute an external capability.

use noema_capabilities::{ToolContractError, ToolSpec};
use noema_providers::{
    GenerateResponse, NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderTool,
    ProviderToolCapabilities, ProviderToolTransport, expose_provider_tools,
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub(crate) const SUBMIT_PROGRESS_AUDIT_TOOL: &str = "noema.submit_progress_audit";
pub(crate) const SUBMIT_ACTION_REVIEW_TOOL: &str = "noema.submit_action_review";
pub(crate) const SUBMIT_MEMORY_CHANGES_TOOL: &str = "noema.submit_memory_changes";

/// Build a request-local tool catalog containing one required terminal tool.
///
/// Runtime-owned model operations must use a provider's native tool channel;
/// there is no text-envelope fallback. The returned choice uses an allowed
/// subset where the provider supports it, preserving the canonical name even
/// when the adapter lowers it to a provider-safe alias.
pub(crate) fn required_native_tool(
    spec: ToolSpec,
    capabilities: ProviderToolCapabilities,
) -> Result<(Vec<ProviderTool>, NoemaToolChoice), String> {
    if capabilities.tool_transport != ProviderToolTransport::Native {
        return Err("provider does not support native terminal tools".to_string());
    }
    let canonical_name = spec.name.clone();
    let tools = expose_provider_tools(
        vec![spec],
        capabilities.tool_transport,
        capabilities.schema_dialect,
    );
    let tool_choice = if capabilities.allowed_tools {
        NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Required,
            tools: vec![canonical_name],
        })
    } else {
        NoemaToolChoice::Required
    };
    Ok((tools, tool_choice))
}

/// Validate and decode the one native terminal call required by a model
/// operation. Ordinary assistant text is intentionally ignored rather than
/// interpreted as a fallback response.
pub(crate) fn required_tool_payload<T: DeserializeOwned>(
    response: &GenerateResponse,
    expected_name: &str,
) -> Result<T, String> {
    if response.tool_calls.len() != 1 {
        return Err(format!(
            "expected exactly one native terminal call {expected_name}, received {}",
            response.tool_calls.len()
        ));
    }
    let call = &response.tool_calls[0];
    if call.name != expected_name {
        return Err(format!(
            "expected native terminal call {expected_name}, received {}",
            call.name
        ));
    }
    serde_json::from_value(call.payload.clone()).map_err(|error| {
        format!("native terminal payload for {expected_name} was invalid: {error}")
    })
}

pub(crate) fn progress_audit_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        SUBMIT_PROGRESS_AUDIT_TOOL,
        "Submit one typed progress audit classification for the current continuation loop.",
        json!({
            "type": "object",
            "properties": {
                "decision": {"type": "string", "enum": ["continue", "finalize", "ask_human", "pause"]},
                "user_summary": {"type": "string", "minLength": 1, "maxLength": 4000},
                "next_goal": {"type": ["string", "null"], "maxLength": 4000}
            },
            "required": ["decision", "user_summary", "next_goal"],
            "additionalProperties": false
        }),
    )
}

pub(crate) fn action_review_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        SUBMIT_ACTION_REVIEW_TOOL,
        "Submit one typed authorization and risk classification for the proposed governed action.",
        json!({
            "type": "object",
            "properties": {
                "authorization": {"type": "string", "enum": ["explicit", "substantive", "weak", "absent"]},
                "risk": {"type": "string", "enum": ["low", "medium", "high", "critical"]},
                "reason_codes": {"type": "array", "items": {"type": "string", "enum": ["action_matches_request", "authorization_ambiguous", "authorization_absent", "destination_ambiguous", "payload_scope_ambiguous", "sensitive_data", "broad_scope", "destructive_or_irreversible", "novel_destination", "low_risk"]}},
                "explanation": {"type": "string", "minLength": 1, "maxLength": 4000}
            },
            "required": ["authorization", "risk", "reason_codes", "explanation"],
            "additionalProperties": false
        }),
    )
}

pub(crate) fn memory_changes_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        SUBMIT_MEMORY_CHANGES_TOOL,
        "Submit one complete evidence-backed native memory change set for runtime validation.",
        json!({
            "type": "object",
            "properties": {
                "upserts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": ["string", "null"]},
                            "expected_hash": {"type": ["string", "null"]},
                            "path": {"type": "string", "minLength": 1},
                            "title": {"type": "string", "minLength": 1},
                            "icon": {"type": "string", "minLength": 1},
                            "body": {"type": "string", "description": "Article Markdown without a title or footnote definitions. Use numeric markers [^1], [^2], and so on for ordered citation groups."},
                            "citations": {"type": "array", "description": "Ordered evidence groups. Every group must have its matching numeric marker in body.", "items": {"type": "object", "properties": {"sources": {"type": "array", "minItems": 1, "uniqueItems": true, "items": {"type": "string", "minLength": 1}}}, "required": ["sources"], "additionalProperties": false}}
                        },
                        "required": ["path", "title", "icon", "body", "citations"],
                        "additionalProperties": false
                    }
                },
                "metadata_updates": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "minLength": 1},
                            "icon": {"type": "string", "minLength": 1}
                        },
                        "required": ["path", "icon"],
                        "additionalProperties": false
                    }
                },
                "deletes": {"type": "array", "items": {"type": "string", "minLength": 1}}
            },
            "required": ["upserts", "metadata_updates", "deletes"],
            "additionalProperties": false
        }),
    )
}
