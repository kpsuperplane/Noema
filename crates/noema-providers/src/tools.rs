//! Provider tool transport and selection policy.

use noema_capabilities::ToolName;
use serde::{Deserialize, Serialize};

/// The strength of schema enforcement a provider can apply to one request
/// surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaEnforcement {
    /// The provider cannot constrain this surface with a schema.
    Unsupported,
    /// The provider accepts a schema, but does not promise strict decoding.
    BestEffort,
    /// The provider guarantees strict decoding for the supported schema
    /// subset.
    Strict,
}

/// Independent schema capabilities for native tools and structured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSchemaCapabilities {
    /// Enforcement for provider-native tool arguments.
    pub native_tool_arguments: SchemaEnforcement,
    /// Enforcement for structured assistant output without tools.
    pub structured_output: SchemaEnforcement,
    /// Enforcement for structured assistant output in a tool-calling turn.
    pub structured_output_with_tools: SchemaEnforcement,
}

impl ProviderSchemaCapabilities {
    /// Capabilities for a provider that only accepts unconstrained output.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            native_tool_arguments: SchemaEnforcement::Unsupported,
            structured_output: SchemaEnforcement::Unsupported,
            structured_output_with_tools: SchemaEnforcement::Unsupported,
        }
    }

    /// Capabilities for a provider that can enforce every shared surface.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            native_tool_arguments: SchemaEnforcement::Strict,
            structured_output: SchemaEnforcement::Strict,
            structured_output_with_tools: SchemaEnforcement::Strict,
        }
    }
}

impl Default for ProviderSchemaCapabilities {
    fn default() -> Self {
        Self::unsupported()
    }
}

/// Provider transport used to expose the canonical tool catalog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolTransport {
    /// This provider does not expose model-visible tools.
    #[default]
    None,
    /// Tools are exposed through the provider's native tool channel.
    Native,
    /// Tools are exposed through Noema's structured response envelope.
    NoemaEnvelope,
}

/// Provider-native tool schema dialect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolSchemaDialect {
    /// No native schema dialect.
    #[default]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

impl ProviderToolCapabilities {
    /// Derive conservative schema capabilities from the legacy transport
    /// fields when a provider has not supplied a model-specific override.
    #[must_use]
    pub fn schema_capabilities(self) -> ProviderSchemaCapabilities {
        let native = if self.tool_transport == ProviderToolTransport::Native {
            if self.strict_schema {
                SchemaEnforcement::Strict
            } else {
                SchemaEnforcement::BestEffort
            }
        } else {
            SchemaEnforcement::Unsupported
        };
        let structured = if self.tool_transport != ProviderToolTransport::None {
            if self.strict_schema {
                SchemaEnforcement::Strict
            } else {
                SchemaEnforcement::BestEffort
            }
        } else {
            SchemaEnforcement::Unsupported
        };
        ProviderSchemaCapabilities {
            native_tool_arguments: native,
            structured_output: structured,
            structured_output_with_tools: structured,
        }
    }
}
