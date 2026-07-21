//! Strict Noema response-envelope schema for Responses-compatible APIs.

use serde_json::Value;

/// Build the strict JSON schema used for Noema's structured response envelope.
#[must_use]
pub fn noema_response_text_format() -> Value {
    response_text_format(true)
}

/// Build the structured response schema used alongside provider-native tools.
///
/// Native tool calls arrive as provider output items, so the assistant text
/// envelope must not advertise a second, legacy `tool_calls` channel.
#[must_use]
pub fn noema_native_response_text_format() -> Value {
    response_text_format(false)
}

fn response_text_format(include_tool_calls: bool) -> Value {
    let mut format = serde_json::json!({
        "format": {
            "type": "json_schema",
            "name": "noema_response",
            "strict": false,
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
                            "oneOf": [
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
                                            "type": "object"
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
                            "required": ["name", "payload"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["response_status", "responses", "tool_calls"],
                "additionalProperties": false
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
