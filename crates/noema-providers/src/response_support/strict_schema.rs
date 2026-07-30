//! Provider-neutral helpers for lowering native tool JSON Schema into strict
//! provider dialects.

use serde_json::{Value, json};

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
        required.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_lowering_closes_objects_and_nullable_optionals() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "context": {"type": "string"}
            },
            "required": ["query"]
        });
        lower_strict_schema(&mut schema).expect("strict schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["context", "query"]));
        assert_eq!(
            schema["properties"]["context"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(schema["properties"]["query"]["type"], "string");
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
}
