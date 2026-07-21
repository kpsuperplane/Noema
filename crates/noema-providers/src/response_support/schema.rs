//! Strict Noema response-envelope schema for Responses-compatible APIs.

use serde_json::Value;

use super::strict_schema::recursive_json_schema;

/// Build the strict JSON schema used for Noema's structured response envelope.
#[must_use]
pub fn noema_response_text_format() -> Value {
    response_text_format(true, true)
}

/// Build the shared response format with an explicit provider strictness bit.
pub(crate) fn noema_response_text_format_with_strict(
    include_tool_calls: bool,
    strict: bool,
) -> Value {
    response_text_format(include_tool_calls, strict)
}

fn response_text_format(include_tool_calls: bool, strict: bool) -> Value {
    let mut format = serde_json::json!({
        "format": {
            "type": "json_schema",
            "name": "noema_response",
            "strict": strict,
            "schema": {
                "type": "object",
                "properties": {
                    "response_status": {
                        "type": "string",
                        "enum": ["needs_tools", "final"]
                    },
                    "responses": {
                        "type": "array",
                        "items": {
                            "anyOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["text"]
                                        },
                                        "phase": {
                                            "type": "string",
                                            "enum": ["commentary", "final_answer"]
                                        },
                                        "text": {
                                            "type": "string"
                                        }
                                    },
                                    "required": ["kind", "phase", "text"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["multiple_choice"]
                                        },
                                        "phase": {
                                            "type": "string",
                                            "enum": ["commentary", "final_answer"]
                                        },
                                        "prompt": {
                                            "type": "string"
                                        },
                                        "selection_mode": {
                                            "type": "string",
                                            "enum": ["pick_one", "pick_many"]
                                        },
                                        "options": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "id": {"type": "string"},
                                                    "label": {"type": "string"}
                                                },
                                                "required": ["id", "label"],
                                                "additionalProperties": false
                                            }
                                        }
                                    },
                                    "required": ["kind", "phase", "prompt", "selection_mode", "options"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["structured"]
                                        },
                                        "schema": {
                                            "type": "string"
                                        },
                                        "payload": {
                                            "$ref": "#/$defs/node"
                                        }
                                    },
                                    "required": ["kind", "schema", "payload"],
                                    "additionalProperties": false
                                }
                            ]
                        }
                    },
                    "tool_calls": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": ["string", "null"]},
                                "name": {"type": "string"},
                                "payload": {"type": "object"}
                            },
                            "required": ["id", "name", "payload"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["response_status", "responses", "tool_calls"],
                "additionalProperties": false,
                "$defs": {
                    "node": recursive_json_schema()["$defs"]["node"]
                }
            }
        }
    });

    if !include_tool_calls {
        let schema = &mut format["format"]["schema"];
        if let Some(properties) = schema["properties"].as_object_mut() {
            properties.remove("tool_calls");
        }
        if let Some(required) = schema["required"].as_array_mut() {
            required.retain(|field| field.as_str() != Some("tool_calls"));
        }
    }

    format
}
