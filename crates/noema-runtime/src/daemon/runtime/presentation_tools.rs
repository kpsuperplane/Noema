//! Native presentation tools owned by the primary conversation runtime.
//!
//! The A2UI tool deliberately carries its protocol messages as JSONL text.
//! The outer tool contract stays shallow so each provider only has to support
//! a string argument; protocol validation happens after the native call is
//! received by the interaction runtime.

use noema_capabilities::{ToolContractError, ToolSpec};
use noema_providers::{MultipleChoiceOption, MultipleChoiceSelectionMode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

/// Canonical native tool for asking the human to choose one or more options.
pub(crate) const PRESENT_MULTIPLE_CHOICE_TOOL: &str = "noema.present_multiple_choice";
/// Canonical native tool for presenting an A2UI v0.9.1 message batch.
pub(crate) const PRESENT_A2UI_TOOL: &str = "noema.present_a2ui";

/// Validated arguments for [`PRESENT_MULTIPLE_CHOICE_TOOL`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresentMultipleChoiceArguments {
    /// Question or instruction shown above the options.
    pub(crate) prompt: String,
    /// Whether the human may choose one option or several.
    pub(crate) selection_mode: MultipleChoiceSelectionMode,
    /// Ordered options with stable semantic IDs.
    pub(crate) options: Vec<MultipleChoiceOption>,
}

/// Validated outer arguments for [`PRESENT_A2UI_TOOL`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresentA2UIArguments {
    /// One or more newline-delimited A2UI v0.9.1 messages.
    pub(crate) jsonl: String,
}

/// Build the canonical multiple-choice presentation tool specification.
pub(crate) fn present_multiple_choice_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        PRESENT_MULTIPLE_CHOICE_TOOL,
        "Present a multiple-choice question with stable option IDs and labels to the human.",
        json!({
            "type": "object",
            "properties": {
                "prompt": {"type": "string", "minLength": 1},
                "selection_mode": {"type": "string", "enum": ["pick_one", "pick_many"]},
                "options": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "minLength": 1},
                            "label": {"type": "string", "minLength": 1}
                        },
                        "required": ["id", "label"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["prompt", "selection_mode", "options"],
            "additionalProperties": false
        }),
    )
}

/// Build the shallow canonical A2UI presentation tool specification.
pub(crate) fn present_a2ui_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        PRESENT_A2UI_TOOL,
        "Present one or more A2UI v0.9.1 messages encoded as JSONL to the human.",
        json!({
            "type": "object",
            "properties": {
                "jsonl": {"type": "string", "minLength": 1}
            },
            "required": ["jsonl"],
            "additionalProperties": false
        }),
    )
}

/// Strictly decode and validate one multiple-choice tool payload.
pub(crate) fn parse_multiple_choice_payload(
    payload: &Value,
) -> Result<PresentMultipleChoiceArguments, String> {
    let raw: RawPresentMultipleChoiceArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid {PRESENT_MULTIPLE_CHOICE_TOOL} payload: {error}"))?;
    let prompt = normalize_required(raw.prompt, "prompt")?;
    if raw.options.is_empty() {
        return Err(format!(
            "{PRESENT_MULTIPLE_CHOICE_TOOL} options must include at least one item"
        ));
    }

    let mut ids = HashSet::with_capacity(raw.options.len());
    let mut options = Vec::with_capacity(raw.options.len());
    for option in raw.options {
        let id = normalize_required(option.id, "options.id")?;
        if !ids.insert(id.clone()) {
            return Err(format!(
                "{PRESENT_MULTIPLE_CHOICE_TOOL} option IDs must be unique: {id}"
            ));
        }
        options.push(MultipleChoiceOption {
            id,
            label: normalize_required(option.label, "options.label")?,
        });
    }

    Ok(PresentMultipleChoiceArguments {
        prompt,
        selection_mode: raw.selection_mode,
        options,
    })
}

/// Strictly decode the shallow outer A2UI tool payload.
pub(crate) fn parse_a2ui_payload(payload: &Value) -> Result<PresentA2UIArguments, String> {
    let raw: RawPresentA2UIArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid {PRESENT_A2UI_TOOL} payload: {error}"))?;
    let jsonl = normalize_required(raw.jsonl, "jsonl")?;
    Ok(PresentA2UIArguments { jsonl })
}

fn normalize_required(value: String, field: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPresentMultipleChoiceArguments {
    prompt: String,
    selection_mode: MultipleChoiceSelectionMode,
    options: Vec<RawMultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMultipleChoiceOption {
    id: String,
    label: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPresentA2UIArguments {
    jsonl: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn multiple_choice_payload_is_strict_and_normalized() {
        let payload = json!({
            "prompt": "  Pick a direction  ",
            "selection_mode": "pick_one",
            "options": [
                {"id": "ship", "label": " Ship it "},
                {"id": "polish", "label": "Polish first"}
            ]
        });
        assert_eq!(
            parse_multiple_choice_payload(&payload).expect("valid payload"),
            PresentMultipleChoiceArguments {
                prompt: "Pick a direction".to_string(),
                selection_mode: MultipleChoiceSelectionMode::PickOne,
                options: vec![
                    MultipleChoiceOption {
                        id: "ship".to_string(),
                        label: "Ship it".to_string(),
                    },
                    MultipleChoiceOption {
                        id: "polish".to_string(),
                        label: "Polish first".to_string(),
                    },
                ],
            }
        );
        assert!(
            parse_multiple_choice_payload(&json!({
                "prompt": "Pick",
                "selection_mode": "pick_one",
                "options": [{"id": "same", "label": "One"}, {"id": "same", "label": "Two"}]
            }))
            .is_err()
        );
        assert!(
            parse_multiple_choice_payload(&json!({
                "prompt": "Pick",
                "selection_mode": "pick_one",
                "options": [{"id": "one", "label": "One", "extra": true}]
            }))
            .is_err()
        );
    }

    #[test]
    fn a2ui_payload_is_shallow_but_strict() {
        let parsed = parse_a2ui_payload(&json!({
            "jsonl": "{\"beginRendering\":{}}\n{\"surfaceUpdate\":{}}"
        }))
        .expect("valid A2UI JSONL outer payload");
        assert_eq!(
            parsed.jsonl,
            "{\"beginRendering\":{}}\n{\"surfaceUpdate\":{}}"
        );
        assert!(parse_a2ui_payload(&json!({"jsonl": "  "})).is_err());
        assert!(parse_a2ui_payload(&json!({"jsonl": "{}", "extra": true})).is_err());
    }
}
