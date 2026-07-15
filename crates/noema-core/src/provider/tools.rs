//! Provider-neutral tool contracts.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Validated canonical Noema tool name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolName(String);

impl ToolName {
    /// Validate and construct a tool name.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the name is empty, has leading or
    /// trailing whitespace, or contains characters outside the canonical Noema
    /// tool-name grammar. Provider adapters may lower these names for stricter
    /// provider-specific grammars.
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
        if !trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
        }) {
            return Err(ToolContractError::InvalidToolName(format!(
                "tool name cannot contain spaces, slashes, control characters, or other unsupported characters: {value:?}"
            )));
        }
        if trimmed.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .any(|character| character.is_ascii_alphanumeric())
        }) {
            return Err(ToolContractError::InvalidToolName(format!(
                "tool name must contain non-empty dot-separated segments with at least one ASCII letter or digit: {value:?}"
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

impl Serialize for ToolName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Canonical provider-neutral model-visible tool specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolSpec {
    /// Canonical Noema tool name.
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
}

impl NoemaToolSpec {
    /// Validate and construct a canonical tool specification.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the name, description, or input schema
    /// violates the canonical provider-neutral tool contract.
    pub fn new(
        name: impl AsRef<str>,
        description: impl Into<String>,
        input_schema: Value,
        execution: NoemaToolExecution,
    ) -> Result<Self, ToolContractError> {
        let name = ToolName::new(name.as_ref())?;
        let input_schema = NoemaToolSchema::new_input(name.as_str(), input_schema)?;
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
        })
    }

    /// Attach an optional output schema to this tool specification.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the output schema root is not a JSON
    /// object schema.
    pub fn with_output_schema(mut self, schema: Value) -> Result<Self, ToolContractError> {
        self.output_schema = Some(NoemaToolSchema::new_output(self.name.as_str(), schema)?);
        Ok(self)
    }
}

/// Validated JSON object schema for a Noema tool contract.
#[derive(Debug, Clone, PartialEq)]
pub struct NoemaToolSchema {
    value: Value,
}

impl NoemaToolSchema {
    /// Validate and construct a tool schema.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the schema root is not a JSON object
    /// schema.
    pub fn new(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        Self::new_input(tool_name, value)
    }

    /// Validate and construct an input schema.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the schema root is not a JSON object
    /// schema.
    pub fn new_input(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        Self::new_with_kind(tool_name, "input", value)
    }

    /// Validate and construct an output schema.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] when the schema root is not a JSON object
    /// schema.
    pub fn new_output(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        Self::new_with_kind(tool_name, "output", value)
    }

    fn new_with_kind(
        tool_name: &str,
        schema_kind: &str,
        value: Value,
    ) -> Result<Self, ToolContractError> {
        if value.get("type").and_then(Value::as_str) != Some("object") {
            return Err(ToolContractError::InvalidSchema(format!(
                "tool {tool_name} {schema_kind} schema root must be an object"
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

impl Serialize for NoemaToolSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NoemaToolSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::new_input("schema", value).map_err(serde::de::Error::custom)
    }
}

/// Execution target for a canonical Noema tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolExecution {
    /// Builtin tool executed by the local Noema runtime.
    LocalBuiltin,
    /// First-party public web search executed by the Noema runtime.
    WebSearch,
    /// First-party public web fetch executed by the Noema runtime.
    WebFetch,
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

/// Provider transport used to expose the canonical Noema tool catalog.
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
    /// Provider call id this result answers when supplied by the native tool channel.
    pub provider_call_id: Option<String>,
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
    fn canonical_tool_spec_accepts_web_search_execution_kind() {
        let spec = NoemaToolSpec::new(
            "web.search",
            "Search the public web using Noema's configured search provider.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            NoemaToolExecution::WebSearch,
        )
        .expect("web search tool spec");

        assert_eq!(spec.name.as_str(), "web.search");
        assert!(matches!(spec.execution, NoemaToolExecution::WebSearch));
    }

    #[test]
    fn canonical_tool_spec_rejects_empty_description() {
        let error = NoemaToolSpec::new(
            "search_memory",
            "   ",
            json!({"type": "object"}),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect_err("empty description rejected");

        assert_eq!(
            error.to_string(),
            "tool search_memory description cannot be empty"
        );
    }

    #[test]
    fn tool_name_accepts_canonical_noema_names() {
        for name in [
            "search_memory",
            "update_own_name",
            "mcp.docs.read",
            "mcp.mcp:docs.read",
        ] {
            assert_eq!(ToolName::new(name).expect("valid name").as_str(), name);
        }
    }

    #[test]
    fn tool_name_rejects_invalid_provider_visible_names() {
        for name in [
            "",
            " search_memory",
            "search_memory ",
            "search memory",
            "mcp/docs/read",
            ".mcp.docs.read",
            "mcp..docs.read",
            "mcp.docs.read.",
            "search\tmemory",
            "search\nmemory",
        ] {
            assert!(
                ToolName::new(name).is_err(),
                "expected invalid tool name: {name:?}"
            );
        }
    }

    #[test]
    fn tool_name_deserialization_uses_validation() {
        let valid: ToolName = serde_json::from_value(json!("mcp.docs.read")).expect("valid name");
        assert_eq!(valid.as_str(), "mcp.docs.read");

        let error = serde_json::from_value::<ToolName>(json!("mcp/docs/read"))
            .expect_err("invalid name rejected");
        assert!(error.to_string().contains("tool name cannot contain"));
    }

    #[test]
    fn tool_schema_serializes_as_raw_json_schema() {
        let schema = NoemaToolSchema::new(
            "search_memory",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
        .expect("schema");

        assert_eq!(
            serde_json::to_value(&schema).expect("serialize schema"),
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn tool_schema_deserialization_validates_object_root() {
        let schema: NoemaToolSchema =
            serde_json::from_value(json!({"type": "object"})).expect("object schema");
        assert_eq!(schema.as_value(), &json!({"type": "object"}));

        let error = serde_json::from_value::<NoemaToolSchema>(json!({"type": "string"}))
            .expect_err("non-object schema rejected");
        assert_eq!(
            error.to_string(),
            "tool schema input schema root must be an object"
        );
    }

    #[test]
    fn output_schema_error_names_output_schema() {
        let error = NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            json!({"type": "object"}),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("spec")
        .with_output_schema(json!({"type": "string"}))
        .expect_err("non-object output schema rejected");

        assert_eq!(
            error.to_string(),
            "tool search_memory output schema root must be an object"
        );
    }

    #[test]
    fn tool_result_preserves_provider_call_id() {
        let result = NoemaToolResult {
            call_id: Some("call_1".to_string()),
            name: ToolName::new("search_memory").expect("name"),
            provider_call_id: Some("fc_1".to_string()),
            success: true,
            output: json!({"ok": true}),
        };

        assert_eq!(result.provider_call_id.as_deref(), Some("fc_1"));
    }

    #[test]
    fn default_tool_capabilities_do_not_expose_tools() {
        let capabilities = ProviderToolCapabilities::default();

        assert_eq!(capabilities.tool_transport, ProviderToolTransport::None);
        assert!(!capabilities.parallel_tool_calls);
        assert!(!capabilities.allowed_tools);
        assert!(!capabilities.prompt_cache_options);
        assert!(!capabilities.prompt_cache_breakpoints);
    }
}
