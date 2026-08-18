//! Stable local-file parsing and public-file download contracts.

use crate::{ToolContractError, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Local file parsing operation name.
pub const FILE_PARSE_TOOL: &str = "file.parse";
/// Public file download operation name.
pub const FILE_DOWNLOAD_TOOL: &str = "file.download";
/// Default maximum parsed characters.
pub const DEFAULT_MAX_CHARS: usize = 20_000;
/// Hard maximum parsed characters.
pub const HARD_MAX_CHARS: usize = 20_000;
/// Maximum relative path length.
pub const MAX_PATH_CHARS: usize = 4096;
/// Maximum URL length.
pub const MAX_URL_CHARS: usize = 2048;
/// Maximum reason length.
pub const MAX_REASON_CHARS: usize = 500;

/// Normalized local parse request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileParseRequest {
    /// Relative source path.
    pub path: String,
    /// Maximum returned characters.
    pub max_chars: usize,
}

/// Outcome of one supported-file conversion attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileParseStatus {
    /// A parser returned bounded content.
    Converted,
    /// No parser supports the source format.
    Unsupported,
    /// A supported parser could not convert the source.
    Failed,
}

/// Bounded model-visible file parse result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileParseResponse {
    /// Relative source path supplied by the model.
    pub path: String,
    /// Complete source file size.
    pub source_bytes: u64,
    /// Conversion outcome.
    pub status: FileParseStatus,
    /// Parser implementation, when selected.
    pub parser: Option<String>,
    /// Detected source format, when known.
    pub format: Option<String>,
    /// Returned content format.
    pub content_format: Option<String>,
    /// Bounded parsed content.
    pub content: Option<String>,
    /// Returned character count.
    pub returned_chars: usize,
    /// Whether content was shortened.
    pub truncated: bool,
    /// Safe conversion failure code.
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseArguments {
    path: String,
    #[serde(default)]
    max_chars: Option<usize>,
}

/// Build the exact `file.parse` specification.
///
/// # Errors
///
/// Returns [`ToolContractError`] when the static source schema is invalid.
pub fn parse_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        FILE_PARSE_TOOL,
        "Parse a supported file inside the current working-directory boundary and return bounded text. The file is not modified.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_PATH_CHARS,
                    "description": "Path relative to the current working directory."
                },
                "max_chars": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": HARD_MAX_CHARS,
                    "description": "Maximum parsed characters to return."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
    )
}

/// Parse and normalize `file.parse` arguments.
///
/// # Errors
///
/// Returns a safe message when arguments do not match the contract.
pub fn parse_arguments(payload: &Value) -> Result<FileParseRequest, String> {
    let value = nested_arguments(payload)?;
    let arguments: ParseArguments = serde_json::from_value(value)
        .map_err(|_| "arguments do not match the file.parse schema".to_string())?;
    let path = arguments.path.trim().to_string();
    if path.is_empty() {
        return Err("path is required".to_string());
    }
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(format!("path must be {MAX_PATH_CHARS} characters or fewer"));
    }
    Ok(FileParseRequest {
        path,
        max_chars: arguments
            .max_chars
            .unwrap_or(DEFAULT_MAX_CHARS)
            .clamp(1000, HARD_MAX_CHARS),
    })
}

/// Omit parsed content and sanitize URL credentials before persistence.
#[must_use]
pub fn sanitize_payload_for_storage(payload: &Value) -> Value {
    let mut sanitized = crate::web::fetch::sanitize_payload_for_storage(payload);
    remove_content(&mut sanitized);
    sanitized
}

fn nested_arguments(payload: &Value) -> Result<Value, String> {
    let valid = payload
        .as_object()
        .is_some_and(|object| object.keys().all(|key| key == "arguments"));
    match payload.get("arguments") {
        Some(arguments) if valid => Ok(arguments.clone()),
        Some(_) => Err("nested arguments payload cannot include outer fields".to_string()),
        None => Ok(payload.clone()),
    }
}

fn remove_content(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("content");
            for nested in object.values_mut() {
                remove_content(nested);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(remove_content),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_contract_bounds_input_and_omits_persisted_content() {
        let request = parse_arguments(&json!({"path":" data.csv ","max_chars":1000}))
            .expect("parse arguments");
        assert_eq!(request.path, "data.csv");
        assert_eq!(request.max_chars, 1000);
        assert!(parse_arguments(&json!({"path":"data.csv","extra":true})).is_err());

        let persisted = sanitize_payload_for_storage(&json!({
            "path":"data.csv",
            "content":"private source text",
            "parse":{"content":"nested text"}
        }));
        assert_eq!(persisted, json!({"path":"data.csv","parse":{}}));
    }
}
