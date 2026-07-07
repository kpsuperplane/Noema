//! Model-visible `web.search` tool contract and runtime executor.

use crate::{
    provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError},
    search::types::{SearchError, SearchRequest, SearchResponse, SearchRuntimeProvider},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) const WEB_SEARCH_TOOL: &str = "web.search";
pub(crate) const DEFAULT_RESULTS: usize = 5;
pub(crate) const MAX_RESULTS: usize = 10;
pub(crate) const MAX_QUERY_CHARS: usize = 500;
const MAX_REASON_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebSearchArguments {
    query: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebSearchToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

pub(crate) fn is_web_search_tool(name: &str) -> bool {
    name == WEB_SEARCH_TOOL
}

pub(crate) fn web_search_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
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
        NoemaToolExecution::WebSearch,
    )
}

pub(crate) async fn execute_web_search(
    provider: &SearchRuntimeProvider,
    call_id: Option<String>,
    payload: &Value,
) -> WebSearchToolResult {
    match execute_web_search_inner(provider, payload).await {
        Ok(response) => WebSearchToolResult {
            call_id,
            name: WEB_SEARCH_TOOL.to_string(),
            success: true,
            payload: serde_json::to_value(response)
                .unwrap_or_else(|_| json!({"error": "search response serialization failed"})),
        },
        Err(error) => WebSearchToolResult {
            call_id,
            name: WEB_SEARCH_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_web_search_inner(
    provider: &SearchRuntimeProvider,
    payload: &Value,
) -> Result<SearchResponse, SearchError> {
    let request = parse_web_search_arguments(payload)?;
    provider.search(&request).await
}

pub(crate) fn parse_web_search_arguments(payload: &Value) -> Result<SearchRequest, SearchError> {
    let argument_value = if let Some(arguments) = payload.get("arguments") {
        reject_nested_outer_fields(payload)?;
        arguments.clone()
    } else {
        payload.clone()
    };
    let mut arguments: WebSearchArguments = serde_json::from_value(argument_value)
        .map_err(|error| SearchError::InvalidArguments(format!("invalid arguments: {error}")))?;

    arguments.query = arguments.query.trim().to_string();
    if arguments.query.is_empty() {
        return Err(SearchError::InvalidArguments(
            "query is required".to_string(),
        ));
    }
    if arguments.query.chars().count() > MAX_QUERY_CHARS {
        return Err(SearchError::InvalidArguments(format!(
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
        return Err(SearchError::InvalidArguments(format!(
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

fn reject_nested_outer_fields(payload: &Value) -> Result<(), SearchError> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    if object.keys().all(|key| key == "arguments") {
        Ok(())
    } else {
        Err(SearchError::InvalidArguments(
            "nested arguments payload cannot include outer fields".to_string(),
        ))
    }
}

pub(crate) fn safe_error_message(error: &SearchError) -> String {
    match error {
        SearchError::InvalidArguments(message) => message.clone(),
        SearchError::Timeout => "search request timed out".to_string(),
        SearchError::RateLimited => {
            "search provider rate limited or blocked the request".to_string()
        }
        SearchError::AuthFailed => "provider account unauthenticated".to_string(),
        SearchError::Http => "search provider request failed".to_string(),
        SearchError::Parse => "search provider response could not be parsed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn web_search_tool_spec_matches_runtime_arguments() {
        let spec = web_search_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), WEB_SEARCH_TOOL);
        assert!(matches!(spec.execution, NoemaToolExecution::WebSearch));
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["query"]["maxLength"],
            MAX_QUERY_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["max_results"]["maximum"],
            MAX_RESULTS
        );
    }

    #[test]
    fn parses_nested_arguments_and_trims_query() {
        let arguments = parse_web_search_arguments(&json!({
            "arguments": {
                "query": "  rust reqwest current version  ",
                "reason": "  answer a version question  ",
                "max_results": 7
            }
        }))
        .expect("arguments");

        assert_eq!(arguments.query, "rust reqwest current version");
        assert_eq!(
            arguments.reason.as_deref(),
            Some("answer a version question")
        );
        assert_eq!(arguments.max_results, 7);
    }

    #[test]
    fn defaults_and_clamps_result_count() {
        let arguments = parse_web_search_arguments(&json!({
            "query": "duckduckgo html endpoint",
            "max_results": 50
        }))
        .expect("arguments");

        assert_eq!(arguments.max_results, MAX_RESULTS);
    }

    #[test]
    fn rejects_empty_query() {
        let error =
            parse_web_search_arguments(&json!({"query": "   "})).expect_err("empty query rejected");

        assert_eq!(safe_error_message(&error), "query is required");
    }

    #[test]
    fn rejects_nested_outer_fields() {
        let error = parse_web_search_arguments(&json!({
            "arguments": {"query": "trains"},
            "provider": "brave_search"
        }))
        .expect_err("outer fields rejected");

        assert_eq!(
            safe_error_message(&error),
            "nested arguments payload cannot include outer fields"
        );
    }
}
