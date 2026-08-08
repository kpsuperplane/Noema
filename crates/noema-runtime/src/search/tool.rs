//! Model-visible `web.search` tool contract and runtime executor.

use noema_capabilities::{
    ToolContractError, ToolSpec,
    web::search::{SearchResponse, WEB_SEARCH_TOOL},
};
use noema_providers::{WebSearchBackendHandle, WebSearchError};
use serde_json::Value;

use crate::web_fetch::tool::{WebFetchToolResult, web_tool_result};

#[derive(Debug)]
pub(crate) enum SearchExecutionError {
    InvalidArguments(String),
    Backend(WebSearchError),
}

pub(crate) type WebSearchToolResult = WebFetchToolResult;

pub(crate) fn is_web_search_tool(name: &str) -> bool {
    name == WEB_SEARCH_TOOL
}

pub(crate) fn web_search_tool_spec() -> Result<ToolSpec, ToolContractError> {
    noema_capabilities::web::search::tool_spec()
}

pub(crate) async fn execute_web_search(
    provider: &WebSearchBackendHandle,
    call_id: Option<String>,
    payload: &Value,
) -> WebSearchToolResult {
    let result = execute_web_search_inner(provider, payload)
        .await
        .map_err(|error| safe_error_message(&error));
    web_tool_result(
        call_id,
        WEB_SEARCH_TOOL,
        result,
        "search response serialization failed",
    )
}

async fn execute_web_search_inner(
    provider: &WebSearchBackendHandle,
    payload: &Value,
) -> Result<SearchResponse, SearchExecutionError> {
    let request = noema_capabilities::web::search::parse_arguments(payload)
        .map_err(|error| SearchExecutionError::InvalidArguments(error.message().to_string()))?;
    provider
        .search(&request)
        .await
        .map_err(SearchExecutionError::Backend)
}

pub(crate) fn safe_error_message(error: &SearchExecutionError) -> String {
    let error = match error {
        SearchExecutionError::InvalidArguments(message) => return message.clone(),
        SearchExecutionError::Backend(error) => error,
    };
    match error {
        WebSearchError::Timeout => "search request timed out",
        WebSearchError::RateLimited => "search provider rate limited or blocked the request",
        WebSearchError::AuthFailed => "provider account unauthenticated",
        WebSearchError::Http => "search provider request failed",
        WebSearchError::Parse => "search provider response could not be parsed",
    }
    .to_string()
}
