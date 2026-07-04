//! Shared transport and parser for OpenAI-compatible Responses API calls.

use super::sse::SseAccumulator;
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        GenerateInput, GenerateMessageRole, GenerateStreamEvent, PromptCacheRetention,
        ProviderError, TokenUsage,
    },
};
use futures_util::StreamExt;
use reqwest::{
    StatusCode,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON request body sent to a Responses-compatible endpoint.
#[derive(Debug, Serialize)]
pub struct ResponsesRequest {
    /// Model identifier to use for the response.
    pub model: String,
    /// User-visible input.
    pub input: ResponsesInput,
    /// Optional system/developer instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Optional maximum output token budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Optional Responses text controls such as JSON schema output format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Value>,
    /// Native Responses API tool definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ResponsesTool>,
    /// Responses API tool-choice policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<&'static str>,
    /// Whether parallel independent tool calls are allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Whether the upstream should store this response.
    pub store: bool,
    /// Provider prompt-cache retention request when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<PromptCacheRetention>,
}

/// Native Responses API tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
    description: String,
    parameters: Value,
}

impl ResponsesTool {
    /// Build a native function tool definition.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

pub(super) fn noema_response_text_format() -> Value {
    serde_json::json!({
        "format": {
            "type": "json_schema",
            "name": "noema_response",
            "strict": false,
            "schema": {
                "type": "object",
                "properties": {
                    "response_status": {
                        "type": "string",
                        "enum": ["needs_tools", "final"]
                    },
                    "responses": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": [
                                        "text",
                                        "structured"
                                    ]
                                }
                            },
                            "required": ["kind"],
                            "additionalProperties": true
                        }
                    },
                    "tool_calls": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": ["string", "null"]},
                                "name": {"type": "string"},
                                "payload": {"type": "object"}
                            },
                            "required": ["name", "payload"],
                            "additionalProperties": false
                        }
                    },
                    "memory_proposals": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": true
                        }
                    }
                },
                "required": ["response_status", "responses", "tool_calls", "memory_proposals"],
                "additionalProperties": false
            }
        }
    })
}

/// Responses API input shape.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ResponsesInput {
    /// Plain text input.
    Text(String),
    /// Role-tagged input messages.
    Messages(Vec<ResponsesInputMessage>),
}

impl From<&GenerateInput> for ResponsesInput {
    fn from(value: &GenerateInput) -> Self {
        match value {
            GenerateInput::Text(text) => Self::Text(text.clone()),
            GenerateInput::Messages(messages) => Self::Messages(
                messages
                    .iter()
                    .filter(|message| !message.content.trim().is_empty())
                    .map(|message| ResponsesInputMessage {
                        role: match message.role {
                            GenerateMessageRole::User => "user",
                            GenerateMessageRole::Assistant => "assistant",
                        },
                        content: message.content.clone(),
                    })
                    .collect(),
            ),
        }
    }
}

/// One Responses API input message.
#[derive(Debug, Serialize)]
pub struct ResponsesInputMessage {
    /// Provider role.
    pub role: &'static str,
    /// Message text.
    pub content: String,
}

/// Parsed Responses-compatible API response.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResponsesResponse {
    /// Provider response id.
    pub id: Option<String>,
    /// Model reported by the provider.
    pub model: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
    /// Token usage reported by the provider.
    pub usage: Option<ResponsesUsage>,
    #[serde(default, skip)]
    raw: Option<Value>,
}

impl ResponsesResponse {
    /// Return the raw provider payload preserved for developer diagnostics.
    #[must_use]
    pub fn raw_payload(&self) -> Value {
        self.raw.clone().unwrap_or_else(|| {
            serde_json::json!({
                "id": self.id.clone(),
                "model": self.model.clone(),
                "output": self.output.clone(),
                "usage": self.usage.clone(),
            })
        })
    }

    /// Collect assistant output text in provider order.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MalformedResponse`] when the response contains
    /// no text, or [`ProviderError::ApiError`] when the only textual payload is
    /// a refusal.
    pub fn output_text(&self) -> Result<String, ProviderError> {
        let mut output = String::new();
        let mut refusals = Vec::new();

        for item in &self.output {
            let ResponsesOutputItem::Message { content } = item else {
                continue;
            };

            for content_item in content {
                match content_item {
                    ResponsesContent::OutputText { text } => output.push_str(text),
                    ResponsesContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                    ResponsesContent::Other => {}
                }
            }
        }

        if !output.is_empty() {
            return Ok(output);
        }

        if !refusals.is_empty() {
            return Err(ProviderError::ApiError {
                status: 200,
                message: refusals.join("\n"),
                request_id: self.id.clone(),
            });
        }

        Err(ProviderError::MalformedResponse {
            message: "response did not contain output_text".to_string(),
        })
    }

    /// Collect provider-native function-call output items.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MalformedResponse`] when a function-call item
    /// contains invalid JSON arguments.
    pub fn native_tool_calls(
        &self,
    ) -> Result<Vec<crate::provider::GenerateToolCall>, ProviderError> {
        let mut calls = Vec::new();
        for item in &self.output {
            let ResponsesOutputItem::FunctionCall {
                id,
                call_id,
                name,
                arguments,
            } = item
            else {
                continue;
            };
            let payload: Value = serde_json::from_str(arguments).map_err(|source| {
                ProviderError::MalformedResponse {
                    message: format!(
                        "failed to parse native tool call arguments for {name}: {source}"
                    ),
                }
            })?;
            if !payload.is_object() {
                return Err(ProviderError::MalformedResponse {
                    message: format!("native tool call arguments for {name} must be a JSON object"),
                });
            }
            calls.push(crate::provider::GenerateToolCall {
                id: call_id.clone().or_else(|| id.clone()),
                name: name.clone(),
                payload,
            });
        }
        Ok(calls)
    }

    pub(super) fn from_stream_parts(
        id: Option<String>,
        model: Option<String>,
        output_values: Vec<Value>,
        usage: Option<ResponsesUsage>,
    ) -> Result<Self, ProviderError> {
        let raw_output_values = output_values.clone();
        let output = output_values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
                    message: format!("failed to parse SSE output item: {source}"),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let raw = serde_json::json!({
            "id": id.clone(),
            "model": model.clone(),
            "output": raw_output_values,
            "usage": usage.clone(),
        });
        Ok(Self {
            id,
            model,
            output,
            usage,
            raw: Some(raw),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
    #[serde(rename = "function_call")]
    FunctionCall {
        id: Option<String>,
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

/// Token usage reported by a Responses-compatible API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "total_tokens")]
    total: u64,
    #[serde(default, rename = "input_tokens_details")]
    input_details: Option<ResponsesInputTokenDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponsesInputTokenDetails {
    #[serde(default)]
    cached_tokens: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
            cached_input_tokens: value.input_details.map(|details| details.cached_tokens),
        }
    }
}

/// HTTP transport for a Responses-compatible endpoint.
#[derive(Debug, Clone)]
pub struct ResponsesTransport {
    client: reqwest::Client,
    responses_url: String,
}

/// Diagnostic context for Responses-compatible provider calls.
#[derive(Debug, Clone)]
pub struct ResponsesDiagnosticContext {
    /// Developer diagnostic logger.
    pub logger: Option<SystemErrorLogger>,
    /// Provider kind.
    pub provider_kind: String,
    /// Model requested by Noema.
    pub model: String,
    /// Noema conversation id, when available.
    pub conversation_id: Option<String>,
}

impl ResponsesDiagnosticContext {
    /// Build a provider diagnostic context.
    #[must_use]
    pub fn new(
        logger: Option<SystemErrorLogger>,
        provider_kind: impl Into<String>,
        model: impl Into<String>,
        conversation_id: Option<String>,
    ) -> Self {
        Self {
            logger,
            provider_kind: provider_kind.into(),
            model: model.into(),
            conversation_id,
        }
    }

    pub(crate) fn context_json(&self, request_id: Option<&str>) -> Value {
        serde_json::json!({
            "provider_kind": self.provider_kind,
            "model": self.model,
            "conversation_id": self.conversation_id,
            "request_id": request_id,
        })
    }

    pub(crate) fn log_malformed(&self, message: impl Into<String>, raw: Value) {
        if let Some(logger) = &self.logger {
            let message = message.into();
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
                    .with_context(self.context_json(None))
                    .with_error_chain([message])
                    .with_raw(raw),
            );
        }
    }
}

impl ResponsesTransport {
    /// Build a transport from a reqwest client and base API URL.
    ///
    /// The base URL should not include `/responses`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when the base URL is empty or
    /// not absolute.
    pub fn new(
        client: reqwest::Client,
        base_url: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let base_url = normalize_base_url(base_url.into(), "responses base URL")?;
        Ok(Self {
            client,
            responses_url: format!("{base_url}/responses"),
        })
    }

    /// Send one Responses request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] for HTTP transport failures, API errors, or
    /// malformed JSON responses.
    pub async fn send<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
        diagnostics: ResponsesDiagnosticContext,
    ) -> Result<ResponsesResponse, ProviderError>
    where
        T: Serialize,
    {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "responses".to_string(),
                credential: "bearer_token".to_string(),
            });
        }

        let mut builder = self
            .client
            .post(&self.responses_url)
            .bearer_auth(bearer_token);
        for (name, value) in &extra_headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());
        let body_text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;

        if !status.is_success() {
            return Err(error_from_status(status, request_id, &body_text));
        }

        let value = serde_json::from_str::<Value>(&body_text).map_err(|source| {
            let message = format!("failed to parse JSON: {source}");
            diagnostics.log_malformed(
                message.clone(),
                serde_json::json!({
                    "http_status": status.as_u16(),
                    "body_text": body_text,
                }),
            );
            ProviderError::MalformedResponse { message }
        })?;
        let mut response =
            serde_json::from_value::<ResponsesResponse>(value.clone()).map_err(|source| {
                let message = format!("failed to parse JSON: {source}");
                diagnostics.log_malformed(
                    message.clone(),
                    serde_json::json!({
                        "http_status": status.as_u16(),
                        "body_text": body_text,
                    }),
                );
                ProviderError::MalformedResponse { message }
            })?;
        response.raw = Some(value);
        Ok(response)
    }

    /// Send one streaming Responses request, emitting incremental assistant text
    /// events as SSE chunks arrive, and collect the terminal response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] for HTTP transport failures, API errors, or
    /// malformed Server-Sent Events.
    pub async fn send_streaming<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
        diagnostics: ResponsesDiagnosticContext,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ResponsesResponse, ProviderError>
    where
        T: Serialize,
    {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "responses".to_string(),
                credential: "bearer_token".to_string(),
            });
        }

        let mut builder = self
            .client
            .post(&self.responses_url)
            .bearer_auth(bearer_token);
        for (name, value) in &extra_headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());

        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .map_err(|source| ProviderError::HttpFailure { source })?;
            return Err(error_from_status(status, request_id, &body_text));
        }

        let mut accumulator = SseAccumulator::new(diagnostics);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| ProviderError::HttpFailure { source })?;
            accumulator.push_bytes(&chunk, on_event)?;
        }

        accumulator.finish(on_event)
    }
}

/// Normalize and validate a Responses-compatible base URL.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidRequest`] when the URL is empty or invalid.
pub fn normalize_base_url(value: String, label: &str) -> Result<String, ProviderError> {
    let base_url = value.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} cannot be empty"),
        });
    }

    if reqwest::Url::parse(&base_url).is_err() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} must be an absolute URL"),
        });
    }

    Ok(base_url)
}

/// Convert a non-empty string to an HTTP header value.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidRequest`] when the value is not a legal
/// header value.
pub fn header_value(value: &str, label: &str) -> Result<HeaderValue, ProviderError> {
    HeaderValue::from_str(value).map_err(|_| ProviderError::InvalidRequest {
        message: format!("{label} contains invalid header characters"),
    })
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorResponse {
    error: Option<ResponsesErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorBody {
    message: Option<String>,
}

fn error_from_status(
    status: StatusCode,
    request_id: Option<String>,
    body_text: &str,
) -> ProviderError {
    let message = serde_json::from_str::<ResponsesErrorResponse>(body_text)
        .ok()
        .and_then(|body| body.error)
        .and_then(|error| error.message)
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| body_text.trim().to_string())
        .if_empty_then(|| status.canonical_reason().unwrap_or("API error").to_string());

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthenticationFailure {
            message,
            request_id,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimit {
            message,
            request_id,
        },
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id,
        },
    }
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

trait EmptyStringExt {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.is_empty() { fallback() } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_request_serializes_native_tools() {
        let body = ResponsesRequest {
            model: "gpt-test".to_string(),
            input: ResponsesInput::Text("hi".to_string()),
            instructions: None,
            max_output_tokens: None,
            temperature: None,
            text: None,
            tools: vec![ResponsesTool::function(
                "search_memory",
                "Search governed Noema memory.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"],
                    "additionalProperties": false
                }),
            )],
            tool_choice: Some("auto"),
            parallel_tool_calls: Some(false),
            store: false,
            prompt_cache_retention: None,
        };

        let value = serde_json::to_value(body).expect("serialize");

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["name"], "search_memory");
        assert_eq!(
            value["tools"][0]["parameters"]["required"],
            serde_json::json!(["query"])
        );
        assert_eq!(value["tool_choice"], "auto");
        assert_eq!(value["parallel_tool_calls"], false);
    }

    #[test]
    fn responses_response_parses_function_call_output_items() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "search_memory",
                    "arguments": "{\"query\":\"trains\",\"scope_ids\":[\"human:local\"]}"
                }
            ]
        }))
        .expect("response");

        let calls = response.native_tool_calls().expect("tool calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(calls[0].name, "search_memory");
        assert_eq!(calls[0].payload["query"], "trains");
        assert_eq!(
            calls[0].payload["scope_ids"],
            serde_json::json!(["human:local"])
        );
    }

    #[test]
    fn responses_response_rejects_invalid_function_call_arguments_json() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "search_memory",
                    "arguments": "{\"query\":"
                }
            ]
        }))
        .expect("response");

        let error = response
            .native_tool_calls()
            .expect_err("invalid arguments rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("failed to parse native tool call arguments for search_memory")
        );
    }

    #[test]
    fn responses_response_rejects_non_object_function_call_arguments() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "search_memory",
                    "arguments": "[]"
                }
            ]
        }))
        .expect("response");

        let error = response
            .native_tool_calls()
            .expect_err("non-object arguments rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("native tool call arguments for search_memory must be a JSON object")
        );
    }

    #[test]
    fn responses_response_uses_item_id_when_function_call_id_is_missing() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "name": "search_memory",
                    "arguments": "{\"query\":\"trains\"}"
                }
            ]
        }))
        .expect("response");

        let calls = response.native_tool_calls().expect("tool calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id.as_deref(), Some("item_1"));
    }
}
