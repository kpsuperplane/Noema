//! Provider-neutral helpers for lowering JSON Schema into strict provider
//! dialects and carrying arbitrary structured payloads over a closed schema.

use serde_json::{Map, Value, json};

/// Lower a canonical object schema into the strict subset used by OpenAI-style
/// constrained decoding.
///
/// Strict schemas make every property required and represent optional values
/// as nullable. Unsupported composition or map constructs return an error so
/// callers can deliberately fall back to best effort for that one tool.
pub fn lower_strict_schema(value: &mut Value) -> Result<(), String> {
    if value.get("type").and_then(Value::as_str) != Some("object") {
        return Err("schema root must be an object".to_string());
    }
    lower_node(value, "$")
}

fn lower_node(value: &mut Value, path: &str) -> Result<(), String> {
    let Value::Object(object) = value else {
        if let Value::Array(values) = value {
            for (index, child) in values.iter_mut().enumerate() {
                lower_node(child, &format!("{path}[{index}]"))?;
            }
        }
        return Ok(());
    };

    for keyword in [
        "allOf",
        "not",
        "if",
        "then",
        "else",
        "dependentRequired",
        "dependentSchemas",
    ] {
        if object.contains_key(keyword) {
            return Err(format!("unsupported `{keyword}` at {path}"));
        }
    }
    if object.contains_key("uniqueItems") {
        return Err(format!("unsupported `uniqueItems` at {path}"));
    }
    if let Some(additional) = object.get("additionalProperties")
        && !additional.is_boolean()
    {
        return Err(format!(
            "schema maps through `additionalProperties` at {path}"
        ));
    }
    if object
        .get("pattern")
        .and_then(Value::as_str)
        .is_some_and(regex_contains_lookaround)
    {
        return Err(format!("unsupported regex lookaround at {path}"));
    }

    if let Some(one_of) = object.remove("oneOf") {
        object.insert("anyOf".to_string(), one_of);
    }

    let original_required = object
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<std::collections::HashSet<_>>();
    if let Some(properties) = object.get_mut("properties") {
        let Value::Object(properties) = properties else {
            return Err(format!("`properties` must be an object at {path}"));
        };
        let mut required = original_required
            .iter()
            .map(|name| Value::String((*name).to_string()))
            .collect::<Vec<_>>();
        for (name, child) in properties.iter_mut() {
            lower_node(child, &format!("{path}.properties.{name}"))?;
            if !original_required.contains(name) {
                make_nullable(child);
                required.push(Value::String(name.clone()));
            }
        }
        object.insert("required".to_string(), Value::Array(required));
        object.insert("additionalProperties".to_string(), Value::Bool(false));
    } else if object.contains_key("anyOf") {
        // A composition wrapper is a schema union, not an object instance of
        // its own. Hoist the object constraint to the closed variants instead
        // of adding `additionalProperties: false` here, which would reject
        // every property admitted by those variants.
        object.remove("type");
        object.remove("additionalProperties");
        object.remove("required");
    } else if object.get("type").and_then(Value::as_str) == Some("object") {
        object
            .entry("required")
            .or_insert_with(|| Value::Array(Vec::new()));
        object.insert("additionalProperties".to_string(), Value::Bool(false));
    }

    for (key, child) in object.iter_mut() {
        if key == "properties" || key == "required" || key == "additionalProperties" {
            continue;
        }
        lower_node(child, &format!("{path}.{key}"))?;
    }
    Ok(())
}

fn make_nullable(value: &mut Value) {
    let Value::Object(object) = value else {
        return;
    };
    match object.get_mut("type") {
        Some(Value::String(type_name)) => {
            let type_name = std::mem::take(type_name);
            object.insert(
                "type".to_string(),
                Value::Array(vec![
                    Value::String(type_name),
                    Value::String("null".to_string()),
                ]),
            );
        }
        Some(Value::Array(types)) if !types.iter().any(|kind| kind.as_str() == Some("null")) => {
            types.push(Value::String("null".to_string()));
        }
        Some(_) => {}
        None => {
            if let Some(Value::Array(variants)) = object.get_mut("anyOf") {
                variants.push(json!({"type": "null"}));
                return;
            }
            let original = Value::Object(std::mem::take(object));
            *value = json!({"anyOf": [original, {"type": "null"}]});
        }
    }
}

fn regex_contains_lookaround(pattern: &str) -> bool {
    ["(?=", "(?!", "(?<=", "(?<!"]
        .iter()
        .any(|lookaround| pattern.contains(lookaround))
}

/// Encode ordinary JSON as a recursive, tagged payload accepted by strict
/// structured-output schemas.
#[must_use]
pub fn encode_recursive_json(value: &Value) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => json!({
            "kind": "scalar",
            "value": value,
            "items": null,
            "entries": null
        }),
        Value::Array(values) => json!({
            "kind": "array",
            "value": null,
            "items": values.iter().map(encode_recursive_json).collect::<Vec<_>>(),
            "entries": null
        }),
        Value::Object(entries) => json!({
            "kind": "object",
            "value": null,
            "items": null,
            "entries": entries
                .iter()
                .map(|(key, value)| json!({"key": key, "value": encode_recursive_json(value)}))
                .collect::<Vec<_>>()
        }),
    }
}

/// Decode a recursive tagged payload back into ordinary JSON.
///
/// # Errors
///
/// Returns an error when the payload is not a closed recursive node, contains
/// an unknown field, or repeats an object key.
pub fn decode_recursive_json(value: &Value) -> Result<Value, String> {
    let Value::Object(object) = value else {
        return Err("recursive JSON payload must be an object".to_string());
    };
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "kind" | "value" | "items" | "entries"))
    {
        return Err("recursive JSON payload contains an unknown field".to_string());
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "recursive JSON payload is missing kind".to_string())?;
    // Some guided runtimes cannot represent JSON null in a dynamic schema and
    // emit empty/typed padding for the inactive sibling fields. The tag is the
    // authority, so decode only the branch selected by `kind`.
    match kind {
        "scalar" => {
            let value = object
                .get("value")
                .ok_or_else(|| "scalar recursive JSON payload is missing value".to_string())?;
            if value.is_object() || value.is_array() {
                return Err("scalar recursive JSON payload value must be primitive".to_string());
            }
            Ok(value.clone())
        }
        "array" => {
            let items = object
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| "array recursive JSON payload is missing items".to_string())?;
            items
                .iter()
                .map(decode_recursive_json)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        "object" => {
            let entries = object
                .get("entries")
                .and_then(Value::as_array)
                .ok_or_else(|| "object recursive JSON payload is missing entries".to_string())?;
            let mut output = Map::new();
            for entry in entries {
                let entry = entry
                    .as_object()
                    .ok_or_else(|| "object recursive JSON entry must be an object".to_string())?;
                if entry
                    .keys()
                    .any(|key| !matches!(key.as_str(), "key" | "value"))
                {
                    return Err("recursive JSON entry contains an unknown field".to_string());
                }
                let key = entry
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "object recursive JSON entry is missing key".to_string())?;
                if output.contains_key(key) {
                    return Err(format!("duplicate recursive JSON object key {key:?}"));
                }
                let child = entry
                    .get("value")
                    .ok_or_else(|| "object recursive JSON entry is missing value".to_string())?;
                output.insert(key.to_string(), decode_recursive_json(child)?);
            }
            Ok(Value::Object(output))
        }
        other => Err(format!("unknown recursive JSON payload kind {other:?}")),
    }
}

/// Return the closed recursive JSON schema used for arbitrary structured
/// payloads. Every branch is represented by one tagged object so it remains
/// compatible with strict providers without constraining the eventual UI.
#[must_use]
pub fn recursive_json_schema() -> Value {
    let node_ref = json!({"$ref": "#/$defs/node"});
    json!({
        "$defs": {
            "node": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["scalar", "array", "object"]},
                    "value": {"type": ["string", "number", "boolean", "null"]},
                    "items": {"type": ["array", "null"], "items": node_ref},
                    "entries": {
                        "type": ["array", "null"],
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": {"type": "string"},
                                "value": node_ref
                            },
                            "required": ["key", "value"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["kind", "value", "items", "entries"],
                "additionalProperties": false
            }
        },
        "$ref": "#/$defs/node"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_lowering_closes_objects_and_nullable_optionals() {
        let mut schema = json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": []
        });
        lower_strict_schema(&mut schema).expect("strict schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["query"]));
        assert_eq!(
            schema["properties"]["query"]["type"],
            json!(["string", "null"])
        );
    }

    #[test]
    fn strict_lowering_rejects_map_and_uniqueness_semantics() {
        for mut schema in [
            json!({"type":"object","additionalProperties":{"type":"string"}}),
            json!({"type":"object","uniqueItems":true}),
        ] {
            assert!(lower_strict_schema(&mut schema).is_err());
        }
    }

    #[test]
    fn strict_lowering_preserves_closed_composition_variants() {
        let mut schema = json!({
            "type": "object",
            "oneOf": [
                {"type":"object","properties":{"kind":{"type":"string","enum":["a"]}},"required":["kind"]},
                {"type":"object","properties":{"kind":{"type":"string","enum":["b"]}},"required":["kind"]}
            ]
        });
        lower_strict_schema(&mut schema).expect("strict composition");
        assert!(schema.get("type").is_none());
        assert!(schema.get("additionalProperties").is_none());
        assert_eq!(schema["anyOf"].as_array().map(Vec::len), Some(2));
        assert!(
            schema["anyOf"]
                .as_array()
                .expect("variants")
                .iter()
                .all(|variant| variant["additionalProperties"] == false)
        );
    }

    #[test]
    fn recursive_payload_round_trips_and_rejects_duplicate_keys() {
        let value = json!({"a": [true, null], "b": "text"});
        let encoded = encode_recursive_json(&value);
        assert_eq!(decode_recursive_json(&encoded).expect("decode"), value);
        let duplicate = json!({
            "kind":"object", "value":null, "items":null,
            "entries":[
                {"key":"a","value":encode_recursive_json(&json!(1))},
                {"key":"a","value":encode_recursive_json(&json!(2))}
            ]
        });
        assert!(decode_recursive_json(&duplicate).is_err());
        let extra = json!({
            "kind":"scalar", "value":1, "items":null, "entries":null, "extra":true
        });
        assert!(decode_recursive_json(&extra).is_err());
        let padded = json!({
            "kind":"scalar", "value":"text", "items":[], "entries":[]
        });
        assert_eq!(
            decode_recursive_json(&padded).expect("padded decode"),
            json!("text")
        );
    }
}
