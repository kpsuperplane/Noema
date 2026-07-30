use std::collections::HashMap;

use serde_json::Value;

use crate::{ProviderError, ProviderTool, SchemaEnforcement};

use super::lower_strict_schema;

#[derive(Debug, Clone)]
pub(crate) struct OpenAiToolDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Value,
    pub(crate) strict: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct OpenAiToolNameMap {
    pub(crate) definitions: Vec<OpenAiToolDefinition>,
    pub(crate) strict_fallbacks: Vec<(String, String)>,
    provider_to_canonical: HashMap<String, String>,
    canonical_to_provider: HashMap<String, String>,
}

impl OpenAiToolNameMap {
    pub(crate) fn from_tools_with_enforcement(
        tools: &[ProviderTool],
        enforcement: SchemaEnforcement,
    ) -> Result<Self, ProviderError> {
        let mut definitions = Vec::with_capacity(tools.len());
        let mut provider_to_canonical = HashMap::with_capacity(tools.len());
        let mut canonical_to_provider = HashMap::with_capacity(tools.len());
        let mut strict_fallbacks = Vec::new();

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
            canonical_to_provider.insert(canonical.to_string(), provider_safe.to_string());
            let mut parameters = tool.input_schema.as_value().clone();
            let strict = if enforcement == SchemaEnforcement::Strict {
                match lower_strict_schema(&mut parameters) {
                    Ok(()) => Some(true),
                    Err(error) => {
                        normalize_openai_schema(&mut parameters);
                        strict_fallbacks.push((canonical.to_string(), error));
                        Some(false)
                    }
                }
            } else {
                normalize_openai_schema(&mut parameters);
                (enforcement == SchemaEnforcement::BestEffort).then_some(false)
            };
            definitions.push(OpenAiToolDefinition {
                name: provider_safe.to_string(),
                description: tool.description.clone(),
                parameters,
                strict,
            });
        }

        Ok(Self {
            definitions,
            strict_fallbacks,
            provider_to_canonical,
            canonical_to_provider,
        })
    }

    pub(crate) fn canonical_name(&self, provider_name: &str) -> Option<&str> {
        self.provider_to_canonical
            .get(provider_name)
            .map(String::as_str)
    }

    pub(crate) fn provider_name(&self, canonical_name: &str) -> Option<&str> {
        self.canonical_to_provider
            .get(canonical_name)
            .map(String::as_str)
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
