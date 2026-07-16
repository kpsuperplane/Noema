//! Provider tool transport and selection policy.

use noema_capabilities::ToolName;
use serde::{Deserialize, Serialize};

/// Provider transport used to expose the canonical tool catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolTransport {
    /// This provider does not expose model-visible tools.
    None,
    /// Tools are exposed through the provider's native tool channel.
    Native,
    /// Tools are exposed through Noema's structured response envelope.
    NoemaEnvelope,
}

/// Provider-native tool schema dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolSchemaDialect {
    /// No native schema dialect.
    None,
    /// OpenAI Responses API function tool schema dialect.
    OpenAiResponses,
    /// Anthropic tool schema dialect.
    Anthropic,
    /// Apple Foundation Models local schema dialect.
    FoundationLocal,
}

/// Tool selection policy requested by Noema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolChoice {
    /// Let the provider choose whether to call a tool.
    #[default]
    Auto,
    /// Prevent tool calls for this request.
    None,
    /// Require at least one tool call.
    Required,
    /// Restrict the callable set without changing the provider tool catalog.
    Allowed(NoemaAllowedTools),
}

/// Whether a provider may decline to call one of the allowed tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaAllowedToolsMode {
    /// Let the provider choose whether to call an allowed tool.
    #[default]
    Auto,
    /// Require at least one call to an allowed tool.
    Required,
}

/// Provider-neutral restriction to a subset of the declared tool catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoemaAllowedTools {
    /// Whether a tool call is optional or required.
    pub mode: NoemaAllowedToolsMode,
    /// Canonical Noema tool names that remain callable.
    pub tools: Vec<ToolName>,
}

/// Provider/model tool-calling capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderToolCapabilities {
    /// Transport used to expose the request's canonical tool catalog.
    pub tool_transport: ProviderToolTransport,
    /// Whether independent parallel tool calls are supported.
    pub parallel_tool_calls: bool,
    /// Whether explicit tool choice is supported.
    pub tool_choice: bool,
    /// Whether the provider can restrict calls to a subset of a stable catalog.
    pub allowed_tools: bool,
    /// Provider schema dialect for native tools.
    pub schema_dialect: ProviderToolSchemaDialect,
    /// Whether strict JSON schema enforcement is supported.
    pub strict_schema: bool,
    /// Whether provider custom tools are supported.
    pub custom_tools: bool,
    /// Whether native tool-result messages are supported.
    pub native_tool_results: bool,
    /// Whether provider prompt-cache retention requests are supported.
    pub prompt_cache_retention: bool,
    /// Whether provider requests support a stable prompt cache key.
    pub prompt_cache_key: bool,
    /// Whether request-wide prompt-cache options are supported.
    pub prompt_cache_options: bool,
    /// Whether explicit prompt-cache breakpoints are supported on input content.
    pub prompt_cache_breakpoints: bool,
    /// Whether provider requests support encrypted reasoning include/replay.
    pub encrypted_reasoning: bool,
}

impl Default for ProviderToolCapabilities {
    fn default() -> Self {
        Self {
            tool_transport: ProviderToolTransport::None,
            parallel_tool_calls: false,
            tool_choice: false,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::None,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: false,
            prompt_cache_retention: false,
            prompt_cache_key: false,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tool_capabilities_do_not_expose_tools() {
        let capabilities = ProviderToolCapabilities::default();
        assert_eq!(capabilities.tool_transport, ProviderToolTransport::None);
        assert!(!capabilities.parallel_tool_calls);
        assert!(!capabilities.allowed_tools);
    }
}
