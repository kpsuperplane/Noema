use std::collections::HashMap;

use serde_json::Value;

use crate::{ProviderError, ProviderSchemaRequest, ProviderTool};

use super::convert_schema_fully;

#[derive(Debug, Clone)]
pub(crate) struct OpenAiToolDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Value,
    #[cfg(feature = "adapters")]
    pub(crate) strict: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct OpenAiToolNameMap {
    pub(crate) definitions: Vec<OpenAiToolDefinition>,
    provider_to_canonical: HashMap<String, String>,
    #[cfg(feature = "adapters")]
    canonical_to_provider: HashMap<String, String>,
    return_rules: HashMap<String, ToolReturnRules>,
}

#[derive(Debug, Clone)]
struct ToolReturnRules {
    source_schema: Value,
    full_conversion: bool,
}

impl OpenAiToolNameMap {
    pub(crate) fn from_tools_with_request(
        tools: &[ProviderTool],
        request_mode: ProviderSchemaRequest,
    ) -> Result<Self, ProviderError> {
        let mut definitions = Vec::with_capacity(tools.len());
        let mut provider_to_canonical = HashMap::with_capacity(tools.len());
        #[cfg(feature = "adapters")]
        let mut canonical_to_provider = HashMap::with_capacity(tools.len());
        let mut return_rules = HashMap::with_capacity(tools.len());

        for tool in tools {
            let provider_safe = tool.exposed_name();
            let canonical = tool.canonical_spec().name.as_str();
            if let Some(existing) = provider_to_canonical.get(provider_safe) {
                let message = if existing == canonical {
                    format!("duplicate tool name {canonical}")
                } else {
                    format!(
                        "provider-safe tool name collision: {existing} and {canonical} both map to {provider_safe}"
                    )
                };
                return Err(ProviderError::InvalidRequest { message });
            }

            provider_to_canonical.insert(provider_safe.to_string(), canonical.to_string());
            #[cfg(feature = "adapters")]
            canonical_to_provider.insert(canonical.to_string(), provider_safe.to_string());
            let mut parameters = tool.input_schema.as_value().clone();
            let strict = if request_mode == ProviderSchemaRequest::RequestStrictWhenPossible {
                let mut converted = parameters.clone();
                match convert_schema_fully(&mut converted) {
                    Ok(()) => {
                        parameters = converted;
                        Some(true)
                    }
                    Err(_) => {
                        normalize_openai_schema(&mut parameters);
                        Some(false)
                    }
                }
            } else {
                normalize_openai_schema(&mut parameters);
                (request_mode == ProviderSchemaRequest::Send).then_some(false)
            };
            return_rules.insert(
                provider_safe.to_string(),
                ToolReturnRules {
                    source_schema: tool.input_schema.as_value().clone(),
                    full_conversion: strict == Some(true),
                },
            );
            definitions.push(OpenAiToolDefinition {
                name: provider_safe.to_string(),
                description: tool.description.clone(),
                parameters,
                #[cfg(feature = "adapters")]
                strict,
            });
        }

        Ok(Self {
            definitions,
            provider_to_canonical,
            #[cfg(feature = "adapters")]
            canonical_to_provider,
            return_rules,
        })
    }

    pub(crate) fn canonical_name(&self, provider_name: &str) -> Option<&str> {
        self.provider_to_canonical
            .get(provider_name)
            .map(String::as_str)
    }

    #[cfg(feature = "adapters")]
    pub(crate) fn provider_name(&self, canonical_name: &str) -> Option<&str> {
        self.canonical_to_provider
            .get(canonical_name)
            .map(String::as_str)
    }

    pub(crate) fn source_form_arguments(&self, provider_name: &str, mut value: Value) -> Value {
        let Some(rules) = self
            .return_rules
            .get(provider_name)
            .filter(|rules| rules.full_conversion)
        else {
            return value;
        };
        restore_optional_nulls(&mut value, &rules.source_schema);
        value
    }
}

fn restore_optional_nulls(value: &mut Value, schema: &Value) {
    let Some(instance) = value.as_object_mut() else {
        if let Some(items) = schema.get("items")
            && let Some(values) = value.as_array_mut()
        {
            for value in values {
                restore_optional_nulls(value, items);
            }
        }
        return;
    };
    let rules = matching_object_rules(schema, instance);
    let names = instance.keys().cloned().collect::<Vec<_>>();
    for name in names {
        let property_rules = rules
            .iter()
            .filter_map(|rule| rule.get("properties")?.get(&name))
            .collect::<Vec<_>>();
        if property_rules.is_empty() {
            continue;
        }
        let required = rules.iter().any(|rule| {
            rule.get("required")
                .and_then(Value::as_array)
                .is_some_and(|names| names.iter().any(|required| required == &name))
        });
        let remove = instance.get(&name).is_some_and(Value::is_null)
            && !required
            && property_rules.iter().all(|rule| !schema_accepts_null(rule));
        if remove {
            instance.remove(&name);
        } else if let Some(child) = instance.get_mut(&name) {
            for property_rule in property_rules {
                restore_optional_nulls(child, property_rule);
            }
        }
    }
}

fn matching_object_rules<'a>(
    schema: &'a Value,
    instance: &serde_json::Map<String, Value>,
) -> Vec<&'a serde_json::Map<String, Value>> {
    let mut rules = Vec::new();
    let Some(object) = schema.as_object() else {
        return rules;
    };
    if object.contains_key("properties") {
        rules.push(object);
    }
    if let Some(branches) = object.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            rules.extend(matching_object_rules(branch, instance));
        }
    }
    for keyword in ["oneOf", "anyOf"] {
        let Some(branches) = object.get(keyword).and_then(Value::as_array) else {
            continue;
        };
        let matches = branches
            .iter()
            .filter(|branch| object_shape_matches(branch, instance))
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            rules.extend(matching_object_rules(matches[0], instance));
        }
    }
    rules
}

fn object_shape_matches(schema: &Value, instance: &serde_json::Map<String, Value>) -> bool {
    let Some(object) = schema.as_object() else {
        return false;
    };
    if object
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| {
            required
                .iter()
                .filter_map(Value::as_str)
                .any(|name| !instance.contains_key(name))
        })
    {
        return false;
    }
    object
        .get("properties")
        .and_then(Value::as_object)
        .is_none_or(|properties| {
            properties.iter().all(|(name, rule)| {
                instance
                    .get(name)
                    .filter(|value| !value.is_null())
                    .is_none_or(|value| {
                        rule.get("const").is_none_or(|expected| expected == value)
                            && rule
                                .get("enum")
                                .and_then(Value::as_array)
                                .is_none_or(|allowed| allowed.contains(value))
                    })
            })
        })
}

fn schema_accepts_null(schema: &Value) -> bool {
    match schema {
        Value::Bool(allowed) => *allowed,
        Value::Object(object) => {
            object.get("const").is_some_and(Value::is_null)
                || object
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(Value::is_null))
                || match object.get("type") {
                    Some(Value::String(kind)) => kind == "null",
                    Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "null"),
                    _ => false,
                }
                || object
                    .get("oneOf")
                    .or_else(|| object.get("anyOf"))
                    .and_then(Value::as_array)
                    .is_some_and(|schemas| schemas.iter().any(schema_accepts_null))
        }
        _ => false,
    }
}

fn normalize_openai_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let has_lookaround = object
                .get("pattern")
                .and_then(Value::as_str)
                .is_some_and(regex_contains_lookaround);
            if has_lookaround {
                object.remove("pattern");
            }
            for child in object.values_mut() {
                normalize_openai_schema(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize_openai_schema(child);
            }
        }
        _ => {}
    }
}

fn regex_contains_lookaround(pattern: &str) -> bool {
    ["(?=", "(?!", "(?<=", "(?<!"]
        .iter()
        .any(|lookaround| pattern.contains(lookaround))
}
