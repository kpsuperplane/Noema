//! Provider-safe tool lowering for Responses-compatible APIs.

use serde::Serialize;
use serde_json::Value;

use crate::{NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderError};
#[cfg(test)]
use crate::{ProviderSchemaRequest, ProviderTool};

use crate::response_support::tool_names::{OpenAiToolDefinition, OpenAiToolNameMap};
pub(crate) use crate::tools::provider_safe_tool_name;

/// Native Responses API tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_web_access: Option<bool>,
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
            name: Some(name.into()),
            description: Some(description.into()),
            parameters: Some(parameters),
            strict,
            external_web_access: None,
        }
    }

    /// Build the provider-hosted live web-search tool definition.
    #[must_use]
    pub fn web_search() -> Self {
        Self {
            kind: "web_search",
            name: None,
            description: None,
            parameters: None,
            strict: None,
            external_web_access: Some(true),
        }
    }
}

/// Request-local provider-safe tool names for OpenAI-compatible adapters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponsesToolNameMap {
    pub(crate) tools: Vec<ResponsesTool>,
    pub(crate) conversion_fallbacks: Vec<(String, String)>,
    names: OpenAiToolNameMap,
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
        Self::from_tools_with_request(&tools, ProviderSchemaRequest::Send)
    }

    pub(crate) fn from_tools_with_request(
        tools: &[crate::ProviderTool],
        request_mode: crate::ProviderSchemaRequest,
    ) -> Result<Self, ProviderError> {
        let names = OpenAiToolNameMap::from_tools_with_request(tools, request_mode)?;
        let responses_tools = names
            .definitions
            .iter()
            .map(responses_tool_from_definition)
            .collect();

        Ok(Self {
            tools: responses_tools,
            conversion_fallbacks: names.conversion_fallbacks.clone(),
            names,
        })
    }

    pub(super) fn canonical_name(&self, provider_name: &str) -> Option<&str> {
        self.names.canonical_name(provider_name)
    }

    pub(super) fn source_form_arguments(&self, provider_name: &str, value: Value) -> Value {
        self.names.source_form_arguments(provider_name, value)
    }

    fn provider_name(&self, canonical_name: &str) -> Option<&str> {
        self.names.provider_name(canonical_name)
    }
}

fn responses_tool_from_definition(definition: &OpenAiToolDefinition) -> ResponsesTool {
    ResponsesTool::function(
        definition.name.clone(),
        definition.description.clone(),
        definition.parameters.clone(),
        definition.strict,
    )
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
