use ring::digest::{SHA256, digest};
use serde_json::Value;

use crate::McpDiscoveredTool;

/// Compute the versioned authorization fingerprint for discovered tool
/// metadata.
#[must_use]
pub fn discovered_tool_fingerprint(tool: &McpDiscoveredTool) -> String {
    let payload = serde_json::json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.input_schema,
        "output_schema": tool.output_schema,
        "annotations": tool.annotations,
    });
    let canonical_payload = canonicalize_json(&payload);
    let json_string =
        serde_json::to_string(&canonical_payload).expect("MCP metadata is serializable");
    let fingerprint = digest(&SHA256, json_string.as_bytes());
    format!("mcp-tool-metadata:v2:{}", hex_bytes(fingerprint.as_ref()))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_json).collect()),
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(key, _)| *key);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonicalize_json(value)))
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> McpDiscoveredTool {
        McpDiscoveredTool {
            name: "read".to_string(),
            description: Some("Read a document".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "limit": {"type": "integer"}
                }
            }),
            output_schema: Some(serde_json::json!({"type": "object"})),
            annotations: serde_json::json!({"readOnlyHint": true}),
            metadata_fingerprint: String::new(),
        }
    }

    #[test]
    fn fingerprint_is_versioned_sha256_and_canonicalizes_object_order() {
        let first = tool();
        let mut reordered = first.clone();
        reordered.input_schema = serde_json::from_str(
            r#"{"properties":{"limit":{"type":"integer"},"id":{"type":"string"}},"type":"object"}"#,
        )
        .expect("schema");

        let fingerprint = discovered_tool_fingerprint(&first);
        assert_eq!(fingerprint, discovered_tool_fingerprint(&reordered));
        let digest = fingerprint
            .strip_prefix("mcp-tool-metadata:v2:")
            .expect("version prefix");
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_covers_every_authorization_relevant_field() {
        let base = tool();
        let fingerprint = discovered_tool_fingerprint(&base);
        let mut variants = Vec::new();
        let mut changed = base.clone();
        changed.name = "write".to_string();
        variants.push(changed);
        let mut changed = base.clone();
        changed.description = None;
        variants.push(changed);
        let mut changed = base.clone();
        changed.input_schema = serde_json::json!({"type": "string"});
        variants.push(changed);
        let mut changed = base.clone();
        changed.output_schema = None;
        variants.push(changed);
        let mut changed = base;
        changed.annotations = serde_json::json!({"readOnlyHint": false});
        variants.push(changed);

        assert!(
            variants
                .iter()
                .all(|variant| { discovered_tool_fingerprint(variant) != fingerprint })
        );
    }
}
