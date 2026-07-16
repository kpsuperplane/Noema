//! Strict Noema response-envelope schema for Responses-compatible APIs.

use serde_json::Value;

/// Build the strict JSON schema used for Noema's structured response envelope.
#[must_use]
pub fn noema_response_text_format() -> Value {
    serde_json::json!({
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
    })
}
