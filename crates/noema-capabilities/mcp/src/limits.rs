//! Shared bounds for untrusted MCP metadata, schemas, results, and diagnostics.

use serde_json::{Map, Value};

#[cfg(feature = "transport")]
pub(crate) const MAX_DISCOVERED_TOOLS: usize = 128;
#[cfg(feature = "transport")]
pub(crate) const MAX_TOOL_NAME_BYTES: usize = 256;
#[cfg(feature = "transport")]
pub(crate) const MAX_TOOL_DESCRIPTION_BYTES: usize = 8 * 1024;
#[cfg(feature = "transport")]
pub(crate) const MAX_PAGINATION_CURSOR_BYTES: usize = 8 * 1024;
#[cfg(any(feature = "transport", test))]
pub(crate) const MAX_SCHEMA_BYTES: usize = 64 * 1024;
#[cfg(feature = "transport")]
pub(crate) const MAX_ANNOTATIONS_BYTES: usize = 16 * 1024;
#[cfg(feature = "transport")]
pub(crate) const MAX_TOOL_RESULT_BYTES: usize = 1024 * 1024;
#[cfg(feature = "transport")]
pub(crate) const MAX_WIRE_FRAME_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_DIAGNOSTIC_TEXT_BYTES: usize = 2 * 1024;
pub(crate) const MAX_DIAGNOSTIC_RAW_BYTES: usize = 16 * 1024;

const MAX_JSON_DEPTH: usize = 24;
const MAX_JSON_NODES: usize = 4_096;
const MAX_JSON_COLLECTION_ITEMS: usize = 256;
const MAX_JSON_STRING_BYTES: usize = 1024 * 1024;

pub(crate) fn json_within_limits(value: &Value, max_bytes: usize) -> bool {
    let mut budget = JsonBudget {
        bytes_left: max_bytes,
        nodes_left: MAX_JSON_NODES,
    };
    visit_json(value, 0, &mut budget)
}

#[cfg(feature = "transport")]
pub(crate) fn json_object_within_limits(value: &Map<String, Value>, max_bytes: usize) -> bool {
    let mut budget = JsonBudget {
        bytes_left: max_bytes,
        nodes_left: MAX_JSON_NODES,
    };
    visit_object(value, 0, &mut budget)
}

pub(crate) fn bounded_diagnostic_text(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    if value.len() <= MAX_DIAGNOSTIC_TEXT_BYTES {
        return value.to_string();
    }
    let mut end = MAX_DIAGNOSTIC_TEXT_BYTES.saturating_sub(3);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = value[..end].to_string();
    bounded.push_str("...");
    bounded
}

#[cfg(any(feature = "transport", test))]
pub(crate) fn bounded_provider_schema(schema: &Value) -> Option<Value> {
    json_within_limits(schema, MAX_SCHEMA_BYTES).then(|| schema.clone())
}

struct JsonBudget {
    bytes_left: usize,
    nodes_left: usize,
}

fn visit_json(value: &Value, depth: usize, budget: &mut JsonBudget) -> bool {
    if depth > MAX_JSON_DEPTH || budget.nodes_left == 0 {
        return false;
    }
    budget.nodes_left -= 1;
    match value {
        Value::Null => consume(budget, 4),
        Value::Bool(_) => consume(budget, 5),
        Value::Number(number) => consume(budget, number.to_string().len()),
        Value::String(value) => {
            value.len() <= MAX_JSON_STRING_BYTES
                && consume(budget, escaped_json_string_len(value).saturating_add(2))
        }
        Value::Array(values) => {
            values.len() <= MAX_JSON_COLLECTION_ITEMS
                && consume(budget, values.len().saturating_add(2))
                && values
                    .iter()
                    .all(|value| visit_json(value, depth + 1, budget))
        }
        Value::Object(values) => visit_object(values, depth, budget),
    }
}

fn visit_object(values: &Map<String, Value>, depth: usize, budget: &mut JsonBudget) -> bool {
    values.len() <= MAX_JSON_COLLECTION_ITEMS
        && consume(budget, values.len().saturating_add(2))
        && values.iter().all(|(key, value)| {
            key.len() <= MAX_JSON_STRING_BYTES
                && consume(budget, escaped_json_string_len(key).saturating_add(3))
                && visit_json(value, depth + 1, budget)
        })
}

fn consume(budget: &mut JsonBudget, bytes: usize) -> bool {
    let Some(remaining) = budget.bytes_left.checked_sub(bytes) else {
        return false;
    };
    budget.bytes_left = remaining;
    true
}

fn escaped_json_string_len(value: &str) -> usize {
    value.chars().fold(0_usize, |length, character| {
        length.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\u{000c}' | '\n' | '\r' | '\t' => 2,
            character if character <= '\u{001f}' => 6,
            character => character.len_utf8(),
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reviewed_provider_schema_preserves_exact_bounded_semantics() {
        let schema = json!({
            "type": "object",
            "$defs": {
                "documentId": {
                    "type": "string",
                    "description": "The durable document identifier"
                }
            },
            "properties": {
                "document_id": {"$ref": "#/$defs/documentId"}
            },
            "required": ["document_id"]
        });

        assert_eq!(bounded_provider_schema(&schema), Some(schema));
    }

    #[test]
    fn deeply_nested_or_oversized_json_is_rejected_without_serializing_a_copy() {
        let mut nested = json!({"type": "object"});
        for _ in 0..=MAX_JSON_DEPTH {
            nested = json!({"properties": {"next": nested}});
        }
        assert!(!json_within_limits(&nested, MAX_SCHEMA_BYTES));
        assert!(!json_within_limits(
            &Value::String("x".repeat(MAX_JSON_STRING_BYTES + 1)),
            MAX_SCHEMA_BYTES
        ));
    }
}
