//! Provider-safe tool lowering for Responses-compatible APIs.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::response_support::lower_strict_schema;
use crate::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderError, ProviderTool,
    SchemaEnforcement,
};

pub(crate) use crate::tools::provider_safe_tool_name;

/// Native Responses API tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    pub(super) name: String,
    description: String,
    parameters: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

/// Responses API tool selection policy.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResponsesToolChoice {
    /// Provider-native string mode.
    Mode(&'static str),
    /// Restrict calls to a stable subset of the declared catalog.
    Allowed(ResponsesAllowedTools),
}

/// Responses API allowed-tools object.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesAllowedTools {
    #[serde(rename = "type")]
    kind: &'static str,
    mode: NoemaAllowedToolsMode,
    tools: Vec<ResponsesAllowedTool>,
}

/// One function reference inside an allowed-tools choice.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesAllowedTool {
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
}

impl ResponsesTool {
    /// Build a native function tool definition.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        strict: Option<bool>,
    ) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
            strict,
        }
    }
}

/// Request-local provider-safe tool names for OpenAI-compatible adapters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponsesToolNameMap {
    pub(crate) tools: Vec<ResponsesTool>,
    pub(crate) strict_fallbacks: Vec<(String, String)>,
    provider_to_canonical: HashMap<String, String>,
    canonical_to_provider: HashMap<String, String>,
}

impl ResponsesToolNameMap {
    /// Lower canonical Noema tool names into provider-safe Responses tools.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when two canonical names map
    /// to the same provider-safe name.
    #[cfg(test)]
    pub(crate) fn from_tools(
        tools: &[noema_capabilities::ToolSpec],
    ) -> Result<Self, ProviderError> {
        let tools = tools
            .iter()
            .cloned()
            .map(ProviderTool::canonical)
            .collect::<Vec<_>>();
        Self::from_tools_with_enforcement(&tools, SchemaEnforcement::BestEffort)
    }

    pub(crate) fn from_tools_with_enforcement(
        tools: &[ProviderTool],
        enforcement: SchemaEnforcement,
    ) -> Result<Self, ProviderError> {
        let mut responses_tools = Vec::with_capacity(tools.len());
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
                        normalize_responses_schema(&mut parameters);
                        strict_fallbacks.push((canonical.to_string(), error));
                        Some(false)
                    }
                }
            } else {
                normalize_responses_schema(&mut parameters);
                (enforcement == SchemaEnforcement::BestEffort).then_some(false)
            };
            responses_tools.push(ResponsesTool::function(
                provider_safe,
                tool.description.clone(),
                parameters,
                strict,
            ));
        }

        Ok(Self {
            tools: responses_tools,
            strict_fallbacks,
            provider_to_canonical,
            canonical_to_provider,
        })
    }

    pub(super) fn canonical_name(&self, provider_name: &str) -> Option<&str> {
        self.provider_to_canonical
            .get(provider_name)
            .map(String::as_str)
    }

    fn provider_name(&self, canonical_name: &str) -> Option<&str> {
        self.canonical_to_provider
            .get(canonical_name)
            .map(String::as_str)
    }
}

fn normalize_responses_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let has_lookaround = object
                .get("pattern")
                .and_then(Value::as_str)
                .is_some_and(regex_contains_lookaround);
            if has_lookaround {
                // Responses cannot compile lookarounds into its constrained
                // decoder; execution retains the canonical tool schema.
                object.remove("pattern");
            }
            for child in object.values_mut() {
                normalize_responses_schema(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize_responses_schema(child);
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

pub(crate) fn responses_tool_choice(
    tool_choice: &NoemaToolChoice,
    tool_names: &ResponsesToolNameMap,
    allowed_tools_supported: bool,
) -> Result<Option<ResponsesToolChoice>, ProviderError> {
    if tool_names.tools.is_empty() {
        return match tool_choice {
            NoemaToolChoice::Allowed(_) => Err(ProviderError::InvalidRequest {
                message: "allowed tools require a non-empty tool catalog".to_string(),
            }),
            _ => Ok(None),
        };
    }

    match tool_choice {
        NoemaToolChoice::Auto => Ok(Some(ResponsesToolChoice::Mode("auto"))),
        NoemaToolChoice::None => Ok(Some(ResponsesToolChoice::Mode("none"))),
        NoemaToolChoice::Required => Ok(Some(ResponsesToolChoice::Mode("required"))),
        NoemaToolChoice::Allowed(allowed) => {
            responses_allowed_tools(allowed, tool_names, allowed_tools_supported)
                .map(ResponsesToolChoice::Allowed)
                .map(Some)
        }
    }
}

fn responses_allowed_tools(
    allowed: &NoemaAllowedTools,
    tool_names: &ResponsesToolNameMap,
    supported: bool,
) -> Result<ResponsesAllowedTools, ProviderError> {
    if !supported {
        return Err(ProviderError::InvalidRequest {
            message: "allowed tools are not supported by this provider request profile".to_string(),
        });
    }
    if allowed.tools.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "allowed tools cannot be empty".to_string(),
        });
    }

    let mut seen = std::collections::HashSet::with_capacity(allowed.tools.len());
    let mut tools = Vec::with_capacity(allowed.tools.len());
    for tool in &allowed.tools {
        if !seen.insert(tool.as_str()) {
            return Err(ProviderError::InvalidRequest {
                message: format!("allowed tool {} is duplicated", tool.as_str()),
            });
        }
        let Some(provider_name) = tool_names.provider_name(tool.as_str()) else {
            return Err(ProviderError::InvalidRequest {
                message: format!(
                    "allowed tool {} is not present in the request tool catalog",
                    tool.as_str()
                ),
            });
        };
        tools.push(ResponsesAllowedTool {
            kind: "function",
            name: provider_name.to_string(),
        });
    }

    Ok(ResponsesAllowedTools {
        kind: "allowed_tools",
        mode: allowed.mode,
        tools,
    })
}
