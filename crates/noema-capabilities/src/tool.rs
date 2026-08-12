//! Serializable provider-visible tool contracts.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Validated canonical Noema tool name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ToolName(String);

/// Namespace used by reviewed tools that enable one exact disabled capability.
pub const TOOL_ENABLEMENT_PREFIX: &str = "enable.";

impl ToolName {
    /// Validate and construct a canonical tool name.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError::InvalidToolName`] for an invalid name.
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

    /// Return the validated name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Return the reviewed enablement-tool name for one disabled capability.
///
/// # Errors
///
/// Returns [`ToolContractError::InvalidToolName`] if the composed name is invalid.
pub fn tool_enablement_name(capability: &ToolName) -> Result<ToolName, ToolContractError> {
    ToolName::new(format!("{TOOL_ENABLEMENT_PREFIX}{}", capability.as_str()))
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

impl<'de> Deserialize<'de> for ToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated JSON object schema.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolSchema {
    value: Value,
}

impl ToolSchema {
    /// Validate an input schema.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError::InvalidSchema`] for a non-object root.
    pub fn new_input(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        if value.get("type").and_then(Value::as_str) != Some("object") {
            return Err(ToolContractError::InvalidSchema(format!(
                "tool {tool_name} input schema root must be an object"
            )));
        }
        Ok(Self { value })
    }

    /// Return the raw JSON schema.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

impl<'de> Deserialize<'de> for ToolSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::new_input("schema", value).map_err(serde::de::Error::custom)
    }
}

/// Canonical provider-visible tool specification.
///
/// Execution authority is intentionally absent and cannot be serialized into
/// a provider request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Canonical Noema operation name.
    pub name: ToolName,
    /// Human and model readable description.
    pub description: String,
    /// JSON object schema for arguments.
    pub input_schema: ToolSchema,
}

impl ToolSpec {
    /// Validate and construct a tool specification.
    ///
    /// # Errors
    ///
    /// Returns [`ToolContractError`] for an invalid name, description, or schema.
    pub fn new(
        name: impl AsRef<str>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Result<Self, ToolContractError> {
        let name = ToolName::new(name.as_ref())?;
        let input_schema = ToolSchema::new_input(name.as_str(), input_schema)?;
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
        })
    }
}

/// Validation error for tool contracts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolContractError {
    /// Tool name validation failed.
    #[error("{0}")]
    InvalidToolName(String),
    /// Description validation failed.
    #[error("{0}")]
    InvalidDescription(String),
    /// Schema validation failed.
    #[error("{0}")]
    InvalidSchema(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialized_spec_contains_no_execution_authority() {
        let spec = ToolSpec::new(
            "web.search",
            "Search the public web.",
            json!({"type":"object"}),
        )
        .expect("spec");
        assert_eq!(
            serde_json::to_value(spec).expect("serialize"),
            json!({
                "name": "web.search",
                "description": "Search the public web.",
                "input_schema": {"type":"object"}
            })
        );
    }

    #[test]
    fn canonical_tool_spec_requires_object_input_schema() {
        let error = ToolSpec::new("web.search", "Search.", json!({"type": "string"}))
            .expect_err("non-object input schema rejected");
        assert!(matches!(error, ToolContractError::InvalidSchema(_)));
        let spec = ToolSpec::new(
            "web.search",
            "Search.",
            json!({"type": "object", "properties": {}}),
        )
        .expect("object input schema");
        assert_eq!(spec.input_schema.as_value()["type"], "object");
        let error = ToolSpec::new("web.search", "  ", json!({"type": "object"}))
            .expect_err("empty description rejected");
        assert!(matches!(error, ToolContractError::InvalidDescription(_)));
    }

    #[test]
    fn tool_name_accepts_canonical_noema_names() {
        for name in ["search_memory", "web.search", "mcp.mcp:docs.read"] {
            assert_eq!(ToolName::new(name).expect("canonical name").as_str(), name);
        }
        for name in ["", " web.search", "web/search", "web..search", "web search"] {
            assert!(
                ToolName::new(name).is_err(),
                "expected invalid name: {name:?}"
            );
        }
        assert!(serde_json::from_value::<ToolName>(json!("web/search")).is_err());
        assert_eq!(
            serde_json::from_value::<ToolName>(json!("web.search"))
                .expect("valid name")
                .as_str(),
            "web.search"
        );
    }

    #[test]
    fn tool_schema_serializes_as_raw_json_schema() {
        let schema = ToolSchema::new_input("web.search", json!({"type": "object", "required": []}))
            .expect("schema");
        assert_eq!(
            serde_json::to_value(schema).expect("serialize"),
            json!({"type": "object", "required": []})
        );
        assert!(serde_json::from_value::<ToolSchema>(json!({"type": "array"})).is_err());
        assert!(serde_json::from_value::<ToolSchema>(json!({"type": "object"})).is_ok());
    }
}
