//! Provider-safe tool lowering for Responses-compatible APIs.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::provider::{NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderError};

/// Native Responses API tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    pub(super) name: String,
    description: String,
    parameters: Value,
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
    ) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Request-local provider-safe tool names for OpenAI-compatible adapters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponsesToolNameMap {
    pub(crate) tools: Vec<ResponsesTool>,
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
    pub(crate) fn from_tools(
        tools: &[noema_capabilities::ToolSpec],
    ) -> Result<Self, ProviderError> {
        let mut responses_tools = Vec::with_capacity(tools.len());
        let mut provider_to_canonical = HashMap::with_capacity(tools.len());
        let mut canonical_to_provider = HashMap::with_capacity(tools.len());

        for tool in tools {
            let canonical = tool.name.as_str();
            let provider_safe = provider_safe_tool_name(canonical);
            if let Some(existing) = provider_to_canonical.get(&provider_safe) {
                let message = if existing == canonical {
                    format!("duplicate tool name {canonical}")
                } else {
                    format!(
                        "provider-safe tool name collision: {existing} and {canonical} both map to {provider_safe}"
                    )
                };
                return Err(ProviderError::InvalidRequest { message });
            }

            provider_to_canonical.insert(provider_safe.clone(), canonical.to_string());
            canonical_to_provider.insert(canonical.to_string(), provider_safe.clone());
            responses_tools.push(ResponsesTool::function(
                provider_safe,
                tool.description.clone(),
                tool.input_schema.as_value().clone(),
            ));
        }

        Ok(Self {
            tools: responses_tools,
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

pub(crate) fn provider_safe_tool_name(canonical: &str) -> String {
    let mut encoded = String::with_capacity(canonical.len());
    for byte in canonical.bytes() {
        let character = byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            encoded.push(character);
        } else {
            encoded.push_str(&format!("_x{byte:02x}_"));
        }
    }

    const OPENAI_FUNCTION_NAME_MAX: usize = 64;
    if encoded.len() <= OPENAI_FUNCTION_NAME_MAX {
        return encoded;
    }

    const HASH_SUFFIX_LEN: usize = 18;
    let mut prefix = encoded;
    prefix.truncate(OPENAI_FUNCTION_NAME_MAX - HASH_SUFFIX_LEN);
    format!("{prefix}_h{:016x}", fnv1a64(canonical.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
