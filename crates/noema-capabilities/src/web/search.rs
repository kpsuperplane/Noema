//! Stable `web.search` request, result, schema, and parser contract.

use crate::{ToolContractError, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

/// Canonical web-search operation name.
pub const WEB_SEARCH_TOOL: &str = "web.search";
/// Default number of returned results.
pub const DEFAULT_RESULTS: usize = 5;
/// Hard result-count ceiling.
pub const MAX_RESULTS: usize = 10;
/// Hard query length ceiling.
pub const MAX_QUERY_CHARS: usize = 500;
/// Hard reason length ceiling.
pub const MAX_REASON_CHARS: usize = 500;

/// Normalized search request after argument validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Exact query sent to the configured backend.
    pub query: String,
    /// Optional model-supplied reason.
    pub reason: Option<String>,
    /// Maximum normalized results.
    pub max_results: usize,
}

/// Normalized successful search response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Backend provider identifier.
    pub provider: String,
    /// Reliability contract label.
    pub provider_contract: String,
    /// Effective search query.
    pub query: String,
    /// Ranked normalized results.
    pub results: Vec<SearchResult>,
    /// Compact result summary.
    pub summary: String,
}

/// One normalized search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// One-based rank.
    pub rank: usize,
    /// Result title.
    pub title: String,
    /// Public result URL.
    pub url: String,
    /// Normalized snippet.
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    query: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

/// Safe search argument failure.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct SearchArgumentError {
    message: String,
}

impl SearchArgumentError {
    /// Return the safe model-visible validation message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Build the exact canonical `web.search` specification.
///
/// # Errors
///
/// Returns [`ToolContractError`] if the static contract is invalid.
pub fn tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        WEB_SEARCH_TOOL,
        "Search the public web using Noema's configured search provider.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_QUERY_CHARS,
                    "description": "The exact internet search query to send to the configured search provider."
                },
                "reason": {
                    "type": "string",
                    "maxLength": MAX_REASON_CHARS,
                    "description": "Brief reason this search is useful for the current response."
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_RESULTS,
                    "description": "Maximum number of search results to return."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
    )
}

/// Parse and normalize model-supplied search arguments.
///
/// # Errors
///
/// Returns [`SearchArgumentError`] when arguments violate the contract.
pub fn parse_arguments(payload: &Value) -> Result<SearchRequest, SearchArgumentError> {
    let argument_value = nested_arguments(payload)?;
    let mut arguments: SearchArguments =
        serde_json::from_value(argument_value).map_err(|_| SearchArgumentError {
            message: "arguments do not match the web.search schema".to_string(),
        })?;
    arguments.query = arguments.query.trim().to_string();
    if arguments.query.is_empty() {
        return Err(argument_error("query is required"));
    }
    if arguments.query.chars().count() > MAX_QUERY_CHARS {
        return Err(argument_error(&format!(
            "query must be {MAX_QUERY_CHARS} characters or fewer"
        )));
    }
    let reason = arguments
        .reason
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    if reason
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_REASON_CHARS)
    {
        return Err(argument_error(&format!(
            "reason must be {MAX_REASON_CHARS} characters or fewer"
        )));
    }
    Ok(SearchRequest {
        query: arguments.query,
        reason,
        max_results: arguments
            .max_results
            .unwrap_or(DEFAULT_RESULTS)
            .clamp(1, MAX_RESULTS),
    })
}

fn nested_arguments(payload: &Value) -> Result<Value, SearchArgumentError> {
    if let Some(arguments) = payload.get("arguments") {
        let valid = payload
            .as_object()
            .is_some_and(|object| object.keys().all(|key| key == "arguments"));
        if !valid {
            return Err(argument_error(
                "nested arguments payload cannot include outer fields",
            ));
        }
        Ok(arguments.clone())
    } else {
        Ok(payload.clone())
    }
}

fn argument_error(message: &str) -> SearchArgumentError {
    SearchArgumentError {
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_tool_spec_snapshot_is_stable() {
        assert_eq!(
            serde_json::to_value(tool_spec().expect("spec")).expect("serialize"),
            json!({
                "name": "web.search",
                "description": "Search the public web using Noema's configured search provider.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string", "minLength": 1, "maxLength": 500,
                            "description": "The exact internet search query to send to the configured search provider."
                        },
                        "reason": {
                            "type": "string", "maxLength": 500,
                            "description": "Brief reason this search is useful for the current response."
                        },
                        "max_results": {
                            "type": "integer", "minimum": 1, "maximum": 10,
                            "description": "Maximum number of search results to return."
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            })
        );
    }

    #[test]
    fn parser_rejects_outer_authority_fields() {
        let error = parse_arguments(&json!({
            "arguments": {"query":"rust"},
            "invoker_key": "forged"
        }))
        .expect_err("outer field rejected");
        assert_eq!(
            error.message(),
            "nested arguments payload cannot include outer fields"
        );
    }

    #[test]
    fn parser_normalizes_nested_arguments_and_enforces_bounds() {
        let parsed = parse_arguments(&json!({
            "arguments": {"query": "  rust  ", "reason": "  docs  ", "max_results": 99}
        }))
        .expect("valid nested arguments");
        assert_eq!(
            parsed,
            SearchRequest {
                query: "rust".to_string(),
                reason: Some("docs".to_string()),
                max_results: MAX_RESULTS,
            }
        );
        assert_eq!(
            parse_arguments(&json!({"query": "  "}))
                .expect_err("empty query")
                .message(),
            "query is required"
        );
        assert_eq!(
            parse_arguments(&json!({"query": "rust"}))
                .expect("default result count")
                .max_results,
            DEFAULT_RESULTS
        );
    }

    #[test]
    fn parser_error_does_not_echo_secret_values() {
        let error = parse_arguments(&json!({
            "query":"rust",
            "max_results":"secret-value-that-must-not-leak"
        }))
        .expect_err("wrong type rejected");
        assert_eq!(
            error.message(),
            "arguments do not match the web.search schema"
        );
        assert!(!error.to_string().contains("secret-value"));
    }
}
