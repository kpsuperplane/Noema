//! Model-visible `web.fetch` tool contract and runtime executor.

use noema_capabilities::{
    ToolContractError, ToolSpec,
    web::fetch::{FetchResponse, WEB_FETCH_TOOL},
};
use noema_providers::{WebFetchBackendHandle, WebFetchContext, WebFetchError};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug)]
pub enum FetchExecutionError {
    InvalidArguments(String),
    Backend(WebFetchError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFetchToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

pub(crate) fn web_tool_result<T: Serialize>(
    call_id: Option<String>,
    name: &str,
    result: Result<T, String>,
    serialization_error: &str,
) -> WebFetchToolResult {
    let (success, payload) = match result {
        Ok(response) => (
            true,
            serde_json::to_value(response)
                .unwrap_or_else(|_| json!({"error": serialization_error})),
        ),
        Err(error) => (false, json!({"error": error})),
    };
    WebFetchToolResult {
        call_id,
        name: name.to_string(),
        success,
        payload,
    }
}

#[must_use]
pub fn is_web_fetch_tool(name: &str) -> bool {
    name == WEB_FETCH_TOOL
}

pub fn web_fetch_tool_spec() -> Result<ToolSpec, ToolContractError> {
    noema_capabilities::web::fetch::tool_spec()
}

pub async fn execute_web_fetch(
    provider: &WebFetchBackendHandle,
    context: &WebFetchContext,
    call_id: Option<String>,
    payload: &Value,
) -> WebFetchToolResult {
    let result = execute_web_fetch_inner(provider, context, payload)
        .await
        .map_err(|error| safe_error_message(&error));
    web_tool_result(
        call_id,
        WEB_FETCH_TOOL,
        result,
        "fetch response serialization failed",
    )
}

async fn execute_web_fetch_inner(
    provider: &WebFetchBackendHandle,
    context: &WebFetchContext,
    payload: &Value,
) -> Result<FetchResponse, FetchExecutionError> {
    let request = noema_capabilities::web::fetch::parse_arguments(payload)
        .map_err(|error| FetchExecutionError::InvalidArguments(error.message().to_string()))?;
    provider
        .fetch(&request, context)
        .await
        .map_err(FetchExecutionError::Backend)
}

#[must_use]
pub fn safe_error_message(error: &FetchExecutionError) -> String {
    let error = match error {
        FetchExecutionError::InvalidArguments(message) => return message.clone(),
        FetchExecutionError::Backend(error) => error,
    };
    match error {
        WebFetchError::UnsupportedScheme => "only public http and https URLs are supported",
        WebFetchError::MalformedUrl => "url could not be parsed",
        WebFetchError::BlockedTarget => "private, internal, or local URLs are blocked",
        WebFetchError::Dns => "DNS lookup failed",
        WebFetchError::RedirectBlocked => "redirect target is private, internal, or local",
        WebFetchError::TooManyRedirects => "too many redirects",
        WebFetchError::Timeout => "web fetch request timed out",
        WebFetchError::Http => "web fetch request failed",
        WebFetchError::AuthFailed => "provider account unauthenticated",
        WebFetchError::UnsupportedContentType => "content type is not supported",
        WebFetchError::InvalidTextEncoding => "text response is not valid UTF-8",
        WebFetchError::ResponseTooLarge => "response exceeded the web fetch size limit",
        WebFetchError::Extraction => "readable page content could not be extracted",
        WebFetchError::Summarization => "page summarization failed",
        WebFetchError::PageTooLarge => "page is too large to summarize responsibly",
    }
    .to_string()
}
