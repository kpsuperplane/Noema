//! Provider-neutral tool contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Validated provider-visible tool name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolName(String);

impl ToolName {
    /// Validate and construct a tool name.
    pub fn new(value: impl Into<String>) -> Result<Self, ToolContractError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ToolContractError::InvalidToolName(
                "tool name cannot be empty".to_string(),
            ));
        }
        if trimmed != value {
            return Err(ToolContractError::InvalidToolName(format!(
                "tool name cannot contain leading or trailing whitespace: {value:?}"
            )));
        }
        Ok(Self(value))
    }

    /// Return the validated tool name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ToolName {
    type Err = ToolContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Canonical provider-neutral model-visible tool specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolSpec {
    /// Provider-visible canonical tool name.
    pub name: ToolName,
    /// Human and model readable tool description.
    pub description: String,
    /// JSON object schema for tool input arguments.
    pub input_schema: NoemaToolSchema,
    /// Optional JSON object schema for successful tool output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<NoemaToolSchema>,
    /// Execution target that owns the tool call.
    pub execution: NoemaToolExecution,
    /// Policy for exposing this tool to provider requests.
    pub exposure: ToolExposurePolicy,
}

impl NoemaToolSpec {
    /// Validate and construct a canonical tool specification.
    pub fn new(
        name: impl AsRef<str>,
        description: impl Into<String>,
        input_schema: Value,
        execution: NoemaToolExecution,
    ) -> Result<Self, ToolContractError> {
        let name = ToolName::new(name.as_ref())?;
        let input_schema = NoemaToolSchema::new(name.as_str(), input_schema)?;
        let description = description.into().trim().to_string();
        if description.is_empty() {
            return Err(ToolContractError::InvalidDescription(format!(
                "tool {name} description cannot be empty"
            )));
        }
        Ok(Self {
            name,
            description,
            input_schema,
            output_schema: None,
            execution,
            exposure: ToolExposurePolicy::default(),
        })
    }

    /// Attach an optional output schema to this tool specification.
    #[must_use]
    pub fn with_output_schema(mut self, schema: Value) -> Result<Self, ToolContractError> {
        self.output_schema = Some(NoemaToolSchema::new(self.name.as_str(), schema)?);
        Ok(self)
    }
}

/// Validated JSON object schema for a Noema tool contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolSchema {
    value: Value,
}

impl NoemaToolSchema {
    /// Validate and construct a tool schema.
    pub fn new(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        if value.get("type").and_then(Value::as_str) != Some("object") {
            return Err(ToolContractError::InvalidSchema(format!(
                "tool {tool_name} input schema root must be an object"
            )));
        }
        Ok(Self { value })
    }

    /// Return the underlying JSON schema value.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

/// Execution target for a canonical Noema tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolExecution {
    /// Builtin tool executed by the local Noema runtime.
    LocalBuiltin,
    /// Tool executed through a calibrated MCP server.
    Mcp {
        /// Persisted MCP server id.
        server_id: String,
        /// Provider-visible MCP tool name.
        tool_name: String,
        /// Persisted MCP tool id.
        tool_id: String,
    },
}

/// Provider exposure policy for a canonical tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExposurePolicy {
    /// Fallback behavior when native provider tools are unavailable.
    pub fallback_mode: ProviderToolFallbackMode,
}

impl Default for ToolExposurePolicy {
    fn default() -> Self {
        Self {
            fallback_mode: ProviderToolFallbackMode::NativeRequired,
        }
    }
}

/// Provider fallback behavior for tool exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolFallbackMode {
    /// Do not expose tools.
    NoTools,
    /// Expose only local builtin tools through the legacy response envelope.
    BuiltinOnlyEnvelope,
    /// Expose tools through the legacy response envelope.
    LegacyEnvelope,
    /// Require provider-native tool support.
    NativeRequired,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolChoice {
    /// Let the provider choose whether to call a tool.
    Auto,
    /// Prevent tool calls for this request.
    None,
    /// Require at least one tool call.
    Required,
}

impl Default for NoemaToolChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// Provider/model native tool-calling capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderToolCapabilities {
    /// Whether native model-visible tools are supported.
    pub native_tools: bool,
    /// Whether independent parallel tool calls are supported.
    pub parallel_tool_calls: bool,
    /// Whether explicit tool choice is supported.
    pub tool_choice: bool,
    /// Provider schema dialect for native tools.
    pub schema_dialect: ProviderToolSchemaDialect,
    /// Whether strict JSON schema enforcement is supported.
    pub strict_schema: bool,
    /// Whether provider custom tools are supported.
    pub custom_tools: bool,
    /// Whether native tool-result messages are supported.
    pub native_tool_results: bool,
    /// Default fallback behavior for unavailable native tools.
    pub fallback_mode: ProviderToolFallbackMode,
}

impl Default for ProviderToolCapabilities {
    fn default() -> Self {
        Self {
            native_tools: false,
            parallel_tool_calls: false,
            tool_choice: false,
            schema_dialect: ProviderToolSchemaDialect::None,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: false,
            fallback_mode: ProviderToolFallbackMode::NoTools,
        }
    }
}

/// Provider-neutral tool call emitted by a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolCall {
    /// Noema call id when assigned.
    pub id: Option<String>,
    /// Canonical tool name.
    pub name: ToolName,
    /// Tool arguments emitted by the provider.
    pub arguments: Value,
    /// Provider call id when supplied by the native tool channel.
    pub provider_call_id: Option<String>,
}

/// Provider-neutral result for an executed tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolResult {
    /// Noema call id this result answers.
    pub call_id: Option<String>,
    /// Canonical tool name.
    pub name: ToolName,
    /// Whether execution succeeded.
    pub success: bool,
    /// Structured result payload.
    pub output: Value,
}

/// Validation error for canonical tool contracts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolContractError {
    /// Tool name validation failed.
    #[error("{0}")]
    InvalidToolName(String),
    /// Tool description validation failed.
    #[error("{0}")]
    InvalidDescription(String),
    /// Tool schema validation failed.
    #[error("{0}")]
    InvalidSchema(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_tool_spec_requires_object_input_schema() {
        let error = NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            json!({"type": "string"}),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect_err("non-object schema rejected");

        assert_eq!(
            error.to_string(),
            "tool search_memory input schema root must be an object"
        );
    }

    #[test]
    fn canonical_tool_spec_accepts_object_input_schema() {
        let spec = NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("object schema");

        assert_eq!(spec.name.as_str(), "search_memory");
        assert_eq!(spec.description, "Search governed Noema memory.");
        assert!(matches!(spec.execution, NoemaToolExecution::LocalBuiltin));
    }

    #[test]
    fn default_tool_capabilities_do_not_expose_native_tools() {
        let capabilities = ProviderToolCapabilities::default();

        assert!(!capabilities.native_tools);
        assert!(!capabilities.parallel_tool_calls);
        assert_eq!(
            capabilities.fallback_mode,
            ProviderToolFallbackMode::NoTools
        );
    }
}
