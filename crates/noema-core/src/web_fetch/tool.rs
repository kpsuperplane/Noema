//! Model-visible `web.fetch` tool contract and runtime executor.

use crate::{
    provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError},
    web_fetch::types::{
        DEFAULT_MAX_CHARS, FetchError, FetchRequest, FetchResponse, FetchRuntimeContext,
        HARD_MAX_CHARS, MAX_REASON_CHARS, MAX_URL_CHARS, WebFetchRuntimeProvider,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

pub const WEB_FETCH_TOOL: &str = "web.fetch";
pub const REDACTED_SENSITIVE_WEB_FETCH_URL: &str = "[redacted sensitive web.fetch URL]";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebFetchArguments {
    url: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    max_chars: Option<usize>,
    #[serde(default, rename = "__noema_rejected_sensitive_url")]
    rejected_sensitive_url: bool,
}

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

pub fn web_fetch_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
        WEB_FETCH_TOOL,
        "Fetch and read a public web page using Noema's configured web fetch provider.",
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_URL_CHARS,
                    "description": "The public http(s) URL to fetch and read."
                },
                "reason": {
                    "type": "string",
                    "maxLength": MAX_REASON_CHARS,
                    "description": "Brief reason this page is useful for the current response."
                },
                "max_chars": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": HARD_MAX_CHARS,
                    "description": "Maximum characters to return after extraction and optional summarization."
                }
            },
            "required": ["url"],
            "additionalProperties": false
        }),
        NoemaToolExecution::WebFetch,
    )
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
    let argument_value = if let Some(arguments) = payload.get("arguments") {
        reject_nested_outer_fields(payload)?;
        arguments.clone()
    } else {
        payload.clone()
    };
    let mut arguments: WebFetchArguments = serde_json::from_value(argument_value)
        .map_err(|error| FetchError::InvalidArguments(format!("invalid arguments: {error}")))?;
    if arguments.rejected_sensitive_url {
        return Err(FetchError::InvalidArguments(
            "url must not include credentials or fragments".to_string(),
        ));
    }

    arguments.url = arguments.url.trim().to_string();
    if arguments.url.is_empty() {
        return Err(FetchError::InvalidArguments("url is required".to_string()));
    }
    if arguments.url.chars().count() > MAX_URL_CHARS {
        return Err(FetchError::InvalidArguments(format!(
            "url must be {MAX_URL_CHARS} characters or fewer"
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
        return Err(FetchError::InvalidArguments(format!(
            "reason must be {MAX_REASON_CHARS} characters or fewer"
        )));
    }

    Ok(FetchRequest {
        url: arguments.url,
        reason,
        max_chars: arguments
            .max_chars
            .unwrap_or(DEFAULT_MAX_CHARS)
            .clamp(1000, HARD_MAX_CHARS),
    })
}

#[must_use]
pub fn sanitize_web_fetch_payload_for_storage(payload: &Value) -> Value {
    sanitize_web_fetch_url_field(payload)
}

#[must_use]
pub fn sanitized_web_fetch_display_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed == REDACTED_SENSITIVE_WEB_FETCH_URL {
        return REDACTED_SENSITIVE_WEB_FETCH_URL.to_string();
    }
    let Ok(url) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    if crate::web_fetch::url_policy::url_has_sensitive_components(&url) {
        return REDACTED_SENSITIVE_WEB_FETCH_URL.to_string();
    }
    trimmed.to_string()
}

fn sanitize_web_fetch_url_field(payload: &Value) -> Value {
    let mut sanitized = payload.clone();
    if let Some(object) = sanitized.as_object_mut() {
        if let Some(arguments) = object.get_mut("arguments") {
            sanitize_web_fetch_url_field_in_object(arguments);
        } else {
            sanitize_web_fetch_url_field_in_object(&mut sanitized);
        }
    }
    sanitized
}

fn sanitize_web_fetch_url_field_in_object(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(url_value) = object.get_mut("url") else {
        return;
    };
    let Some(url) = url_value.as_str() else {
        return;
    };
    let Ok(parsed) = Url::parse(url.trim()) else {
        return;
    };
    if !crate::web_fetch::url_policy::url_has_sensitive_components(&parsed) {
        return;
    }
    *url_value = Value::String(REDACTED_SENSITIVE_WEB_FETCH_URL.to_string());
    object.insert(
        "__noema_rejected_sensitive_url".to_string(),
        Value::Bool(true),
    );
}

fn reject_nested_outer_fields(payload: &Value) -> Result<(), FetchError> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    if object.keys().all(|key| key == "arguments") {
        Ok(())
    } else {
        Err(FetchError::InvalidArguments(
            "nested arguments payload cannot include outer fields".to_string(),
        ))
    }
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
    use serde_json::json;

    #[test]
    fn web_fetch_tool_spec_matches_runtime_arguments() {
        let spec = web_fetch_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), WEB_FETCH_TOOL);
        assert!(matches!(spec.execution, NoemaToolExecution::WebFetch));
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
            "url": REDACTED_SENSITIVE_WEB_FETCH_URL,
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
        let sanitized = sanitize_web_fetch_payload_for_storage(&json!({
            "arguments": {
                "url": "https://user:secret@example.com/path#token",
                "reason": "read"
            }
        }));

        assert_eq!(
            sanitized["arguments"]["url"],
            REDACTED_SENSITIVE_WEB_FETCH_URL
        );
        assert_eq!(
            sanitized["arguments"]["__noema_rejected_sensitive_url"],
            true
        );
    }

    #[test]
    fn display_url_redacts_sensitive_components() {
        assert_eq!(
            sanitized_web_fetch_display_url("https://example.com/path#token"),
            REDACTED_SENSITIVE_WEB_FETCH_URL
        );
        assert_eq!(
            sanitized_web_fetch_display_url("https://example.com/path"),
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
