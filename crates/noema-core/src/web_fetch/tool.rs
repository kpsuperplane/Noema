//! Model-visible `web.fetch` tool contract and runtime executor.

use crate::web_fetch::types::{FetchError, FetchRuntimeContext, WebFetchRuntimeProvider};
use noema_capabilities::{
    ToolContractError, ToolSpec,
    web::fetch::{FetchRequest, FetchResponse, WEB_FETCH_TOOL},
};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFetchToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[must_use]
pub fn is_web_fetch_tool(name: &str) -> bool {
    name == WEB_FETCH_TOOL
}

pub fn web_fetch_tool_spec() -> Result<ToolSpec, ToolContractError> {
    noema_capabilities::web::fetch::tool_spec()
}

pub async fn execute_web_fetch(
    provider: &WebFetchRuntimeProvider,
    context: &FetchRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> WebFetchToolResult {
    match execute_web_fetch_inner(provider, context, payload).await {
        Ok(response) => WebFetchToolResult {
            call_id,
            name: WEB_FETCH_TOOL.to_string(),
            success: true,
            payload: serde_json::to_value(response)
                .unwrap_or_else(|_| json!({"error": "fetch response serialization failed"})),
        },
        Err(error) => WebFetchToolResult {
            call_id,
            name: WEB_FETCH_TOOL.to_string(),
            success: false,
            payload: json!({"error": safe_error_message(&error)}),
        },
    }
}

async fn execute_web_fetch_inner(
    provider: &WebFetchRuntimeProvider,
    context: &FetchRuntimeContext,
    payload: &Value,
) -> Result<FetchResponse, FetchError> {
    let request = parse_web_fetch_arguments(payload)?;
    provider.fetch(&request, context).await
}

pub fn parse_web_fetch_arguments(payload: &Value) -> Result<FetchRequest, FetchError> {
    noema_capabilities::web::fetch::parse_arguments(payload)
        .map_err(|error| FetchError::InvalidArguments(error.message().to_string()))
}

#[must_use]
pub fn safe_error_message(error: &FetchError) -> String {
    match error {
        FetchError::InvalidArguments(message) => message.clone(),
        FetchError::UnsupportedScheme => {
            "only public http and https URLs are supported".to_string()
        }
        FetchError::MalformedUrl => "url could not be parsed".to_string(),
        FetchError::BlockedTarget => "private, internal, or local URLs are blocked".to_string(),
        FetchError::Dns => "DNS lookup failed".to_string(),
        FetchError::RedirectBlocked => "redirect target is private, internal, or local".to_string(),
        FetchError::TooManyRedirects => "too many redirects".to_string(),
        FetchError::Timeout => "web fetch request timed out".to_string(),
        FetchError::Http => "web fetch request failed".to_string(),
        FetchError::AuthFailed => "provider account unauthenticated".to_string(),
        FetchError::UnsupportedContentType => "content type is not supported".to_string(),
        FetchError::ResponseTooLarge => "response exceeded the web fetch size limit".to_string(),
        FetchError::Extraction => "readable page content could not be extracted".to_string(),
        FetchError::Summarization => "page summarization failed".to_string(),
        FetchError::PageTooLarge => "page is too large to summarize responsibly".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capabilities::web::fetch::{
        HARD_MAX_CHARS, MAX_URL_CHARS, REDACTED_SENSITIVE_URL, sanitize_payload_for_storage,
    };
    use serde_json::json;

    #[test]
    fn web_fetch_tool_spec_matches_runtime_arguments() {
        let spec = web_fetch_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), WEB_FETCH_TOOL);
        assert_eq!(spec.input_schema.as_value()["required"], json!(["url"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["url"]["maxLength"],
            MAX_URL_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["max_chars"]["maximum"],
            HARD_MAX_CHARS
        );
    }

    #[test]
    fn parses_nested_arguments_and_trims_url() {
        let request = parse_web_fetch_arguments(&json!({
            "arguments": {
                "url": "  https://www.rust-lang.org/learn  ",
                "reason": "  read the public page  ",
                "max_chars": 7000
            }
        }))
        .expect("arguments");

        assert_eq!(request.url, "https://www.rust-lang.org/learn");
        assert_eq!(request.reason.as_deref(), Some("read the public page"));
        assert_eq!(request.max_chars, 7000);
    }

    #[test]
    fn defaults_and_clamps_max_chars() {
        let request = parse_web_fetch_arguments(&json!({
            "url": "https://example.com",
            "max_chars": 99_999
        }))
        .expect("arguments");

        assert_eq!(request.max_chars, HARD_MAX_CHARS);
    }

    #[test]
    fn rejects_empty_url() {
        let error =
            parse_web_fetch_arguments(&json!({"url": "   "})).expect_err("empty URL rejected");

        assert_eq!(safe_error_message(&error), "url is required");
    }

    #[test]
    fn rejects_sanitized_sensitive_url_marker() {
        let error = parse_web_fetch_arguments(&json!({
            "url": REDACTED_SENSITIVE_URL,
            "__noema_rejected_sensitive_url": true
        }))
        .expect_err("sensitive URL rejected");

        assert_eq!(
            safe_error_message(&error),
            "url must not include credentials or fragments"
        );
    }

    #[test]
    fn redacts_sensitive_url_components_for_storage() {
        let sanitized = sanitize_payload_for_storage(&json!({
            "arguments": {
                "url": "https://user:secret@example.com/path#token",
                "reason": "read"
            }
        }));

        assert_eq!(sanitized["arguments"]["url"], REDACTED_SENSITIVE_URL);
        assert_eq!(
            sanitized["arguments"]["__noema_rejected_sensitive_url"],
            true
        );
    }

    #[test]
    fn display_url_redacts_sensitive_components() {
        assert_eq!(
            noema_capabilities::web::fetch::sanitized_display_url("https://example.com/path#token"),
            REDACTED_SENSITIVE_URL
        );
        assert_eq!(
            noema_capabilities::web::fetch::sanitized_display_url("https://example.com/path"),
            "https://example.com/path"
        );
    }

    #[test]
    fn rejects_nested_outer_fields() {
        let error = parse_web_fetch_arguments(&json!({
            "arguments": {"url": "https://example.com"},
            "provider": "direct_http"
        }))
        .expect_err("outer fields rejected");

        assert_eq!(
            safe_error_message(&error),
            "nested arguments payload cannot include outer fields"
        );
    }
}
