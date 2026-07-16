//! Model-visible `web.search` tool contract and runtime executor.

use crate::search::types::{SearchError, SearchRuntimeProvider};
use noema_capabilities::{
    ToolContractError, ToolSpec,
    web::search::{SearchRequest, SearchResponse, WEB_SEARCH_TOOL},
};
use serde_json::{Value, json};

#[derive(Debug)]
pub(crate) enum SearchExecutionError {
    InvalidArguments(String),
    Backend(SearchError),
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

pub(crate) fn web_search_tool_spec() -> Result<ToolSpec, ToolContractError> {
    noema_capabilities::web::search::tool_spec()
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
) -> Result<SearchResponse, SearchExecutionError> {
    let request = parse_web_search_arguments(payload)?;
    provider
        .search(&request)
        .await
        .map_err(SearchExecutionError::Backend)
}

pub(crate) fn parse_web_search_arguments(
    payload: &Value,
) -> Result<SearchRequest, SearchExecutionError> {
    noema_capabilities::web::search::parse_arguments(payload)
        .map_err(|error| SearchExecutionError::InvalidArguments(error.message().to_string()))
}

pub(crate) fn safe_error_message(error: &SearchExecutionError) -> String {
    match error {
        SearchExecutionError::InvalidArguments(message) => message.clone(),
        SearchExecutionError::Backend(SearchError::Timeout) => {
            "search request timed out".to_string()
        }
        SearchExecutionError::Backend(SearchError::RateLimited) => {
            "search provider rate limited or blocked the request".to_string()
        }
        SearchExecutionError::Backend(SearchError::AuthFailed) => {
            "provider account unauthenticated".to_string()
        }
        SearchExecutionError::Backend(SearchError::Http) => {
            "search provider request failed".to_string()
        }
        SearchExecutionError::Backend(SearchError::Parse) => {
            "search provider response could not be parsed".to_string()
        }
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
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["query"]["maxLength"],
            noema_capabilities::web::search::MAX_QUERY_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["max_results"]["maximum"],
            noema_capabilities::web::search::MAX_RESULTS
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

        assert_eq!(
            arguments.max_results,
            noema_capabilities::web::search::MAX_RESULTS
        );
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
