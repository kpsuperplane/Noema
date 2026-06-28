//! Shared transport and parser for OpenAI-compatible Responses API calls.

use crate::provider::{GenerateStreamEvent, ProviderError, TokenUsage};
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
    /// User-visible input text.
    pub input: String,
    /// Optional system/developer instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Optional maximum output token budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Whether the upstream should store this response.
    pub store: bool,
}

/// Parsed Responses-compatible API response.
#[derive(Debug, Deserialize)]
pub struct ResponsesResponse {
    /// Provider response id.
    pub id: Option<String>,
    /// Model reported by the provider.
    pub model: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
    /// Token usage reported by the provider.
    pub usage: Option<ResponsesUsage>,
}

impl ResponsesResponse {
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
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct ResponsesUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "total_tokens")]
    total: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
        }
    }
}

/// HTTP transport for a Responses-compatible endpoint.
#[derive(Debug, Clone)]
pub struct ResponsesTransport {
    client: reqwest::Client,
    responses_url: String,
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

        serde_json::from_str(&body_text).map_err(|source| ProviderError::MalformedResponse {
            message: format!("failed to parse JSON: {source}"),
        })
    }

    /// Send one streaming Responses request and collect the terminal response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] for HTTP transport failures, API errors, or
    /// malformed Server-Sent Events.
    pub async fn send_stream<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
    ) -> Result<ResponsesResponse, ProviderError>
    where
        T: Serialize,
    {
        self.send_streaming(bearer_token, body, extra_headers, &mut |_| {})
            .await
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

        let mut accumulator = SseAccumulator::default();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| ProviderError::HttpFailure { source })?;
            accumulator.push_bytes(&chunk, on_event)?;
        }

        accumulator.finish(on_event)
    }
}

#[allow(dead_code)]
fn response_from_sse(text: &str) -> Result<ResponsesResponse, ProviderError> {
    let mut accumulator = SseAccumulator::default();
    accumulator.push_chunk(text, &mut |_| {})?;
    accumulator.finish(&mut |_| {})
}

#[derive(Default)]
struct SseAccumulator {
    pending: Vec<u8>,
    output_values: Vec<Value>,
    output_text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<ResponsesUsage>,
    terminal_error: Option<Value>,
}

impl SseAccumulator {
    fn push_chunk(
        &mut self,
        chunk: &str,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        self.push_bytes(chunk.as_bytes(), on_event)
    }

    fn push_bytes(
        &mut self,
        chunk: &[u8],
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        self.pending.extend_from_slice(chunk);

        while let Some((index, delimiter_len)) = next_sse_event_boundary(&self.pending) {
            let raw = self.pending[..index].to_vec();
            self.pending.drain(..index + delimiter_len);
            let event = parse_sse_event_bytes(&raw)?;
            self.handle_event(event, on_event)?;
        }

        Ok(())
    }

    fn finish(
        mut self,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ResponsesResponse, ProviderError> {
        if !self.pending.is_empty() {
            let event = parse_sse_event_bytes(&self.pending)?;
            self.pending.clear();
            self.handle_event(event, on_event)?;
        }

        if let Some(error) = self.terminal_error {
            return Err(ProviderError::ApiError {
                status: 200,
                message: response_stream_error_message(&error),
                request_id: self.response_id,
            });
        }

        if self.output_values.is_empty() && !self.output_text.is_empty() {
            self.output_values.push(serde_json::json!({
                "type": "message",
                "content": [{"type": "output_text", "text": self.output_text}]
            }));
        }

        let output = self
            .output_values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
                    message: format!("failed to parse SSE output item: {source}"),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ResponsesResponse {
            id: self.response_id,
            model: self.model,
            output,
            usage: self.usage,
        })
    }

    fn handle_event(
        &mut self,
        event: SseEvent,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        let Some(data) = event.data else {
            return Ok(());
        };
        if data == "[DONE]" {
            return Ok(());
        }

        let value: Value =
            serde_json::from_str(&data).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse SSE JSON: {source}"),
            })?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or_default();

        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    self.output_text.push_str(delta);
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        delta: delta.to_string(),
                    });
                }
            }
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    self.output_values.push(item.clone());
                }
            }
            "response.completed" | "response.incomplete" => {
                if let Some(response) = value.get("response") {
                    collect_terminal_response_metadata(
                        response,
                        &mut self.response_id,
                        &mut self.model,
                        &mut self.usage,
                    );
                    if self.output_values.is_empty()
                        && let Some(items) = response.get("output").and_then(Value::as_array)
                    {
                        self.output_values.extend(items.iter().cloned());
                    }
                }
            }
            "response.failed" => {
                if let Some(response) = value.get("response") {
                    collect_terminal_response_metadata(
                        response,
                        &mut self.response_id,
                        &mut self.model,
                        &mut self.usage,
                    );
                    self.terminal_error = response.get("error").cloned();
                }
            }
            "error" => {
                self.terminal_error = Some(value);
            }
            _ => {}
        }

        Ok(())
    }
}

fn collect_terminal_response_metadata(
    response: &Value,
    response_id: &mut Option<String>,
    model: &mut Option<String>,
    usage: &mut Option<ResponsesUsage>,
) {
    if response_id.is_none() {
        *response_id = response
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if model.is_none() {
        *model = response
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if usage.is_none() {
        *usage = response
            .get("usage")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
    }
}

fn response_stream_error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            error
                .get("error")
                .and_then(|body| body.get("message"))
                .and_then(Value::as_str)
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string())
}

struct SseEvent {
    event: Option<String>,
    data: Option<String>,
}

fn parse_sse_event(raw: &str) -> SseEvent {
    let mut event = None;
    let mut data = Vec::new();
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start().to_string());
        }
    }

    SseEvent {
        event,
        data: (!data.is_empty()).then(|| data.join("\n")),
    }
}

fn parse_sse_event_bytes(raw: &[u8]) -> Result<SseEvent, ProviderError> {
    let raw = std::str::from_utf8(raw).map_err(|source| ProviderError::MalformedResponse {
        message: format!("failed to decode SSE event as UTF-8: {source}"),
    })?;
    Ok(parse_sse_event(raw))
}

#[allow(dead_code)]
fn sse_events(text: &str) -> impl Iterator<Item = SseEvent> + '_ {
    text.split("\n\n").filter_map(|chunk| {
        let event = parse_sse_event(chunk);
        (event.event.is_some() || event.data.is_some()).then_some(event)
    })
}

fn next_sse_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    [(b"\n\n".as_slice(), 2), (b"\r\n\r\n".as_slice(), 4)]
        .into_iter()
        .filter_map(|(delimiter, len)| {
            bytes
                .windows(delimiter.len())
                .position(|window| window == delimiter)
                .map(|index| (index, len))
        })
        .min_by_key(|(index, _)| *index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_sse_parser_emits_deltas_before_terminal_response() {
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::default();
        accumulator
            .push_chunk(
                "event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\n",
                &mut |event| events.push(event),
            )
            .expect("first chunk");
        accumulator
            .push_chunk(
                "event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n\
                 event: response.completed\n\
                 data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\n",
                &mut |event| events.push(event),
            )
            .expect("second chunk");

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "lo".to_string()
                }
            ]
        );

        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "Hello");
    }

    #[test]
    fn response_from_sse_prefers_output_item_done_over_terminal_output() {
        let response = response_from_sse(
            "event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"from item\"}]}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
        )
        .expect("sse response");

        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "from item");
    }

    #[test]
    fn incremental_sse_parser_buffers_utf8_split_across_byte_chunks() {
        let accent = "\u{00e9}";
        let payload = format!(
            "event: response.output_text.delta\n\
             data: {{\"type\":\"response.output_text.delta\",\"delta\":\"caf{accent}\"}}\n\n\
             event: response.completed\n\
             data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp_test\",\"status\":\"completed\"}}}}\n\n"
        );
        let split_at = payload.find(accent).expect("accent byte offset") + 1;
        let bytes = payload.as_bytes();
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::default();

        accumulator
            .push_bytes(&bytes[..split_at], &mut |event| events.push(event))
            .expect("first partial byte chunk");
        accumulator
            .push_bytes(&bytes[split_at..], &mut |event| events.push(event))
            .expect("second partial byte chunk");

        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: format!("caf{accent}")
            }]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(
            response.output_text().expect("output text"),
            format!("caf{accent}")
        );
    }

    #[test]
    fn incremental_sse_parser_buffers_delimiter_split_across_byte_chunks() {
        let payload = b"event: response.output_text.delta\r\n\
                        data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\r\n\
                        \r\n\
                        event: response.completed\r\n\
                        data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\r\n\
                        \r\n";
        let first_delimiter = payload
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("delimiter");
        let split_at = first_delimiter + 2;
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::default();

        accumulator
            .push_bytes(&payload[..split_at], &mut |event| events.push(event))
            .expect("first delimiter chunk");
        accumulator
            .push_bytes(&payload[split_at..], &mut |event| events.push(event))
            .expect("second delimiter chunk");

        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: "Hi".to_string()
            }]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "Hi");
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
