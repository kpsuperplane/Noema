//! Size-bounded reqwest implementation of rmcp's Streamable HTTP client port.

use std::{borrow::Cow, collections::HashMap, future::ready, sync::Arc};

use futures_util::{StreamExt, stream::BoxStream};
use http::{HeaderName, HeaderValue, header::WWW_AUTHENTICATE};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use rmcp::{
    model::{ClientJsonRpcMessage, JsonRpcMessage, ServerJsonRpcMessage},
    transport::streamable_http_client::{
        AuthRequiredError, InsufficientScopeError, SseError, StreamableHttpClient,
        StreamableHttpError, StreamableHttpPostResponse,
    },
};
use sse_stream::{Sse, SseStream};

use crate::http_body::{BodyReadError, bounded_response_body};
use crate::limits::MAX_WIRE_FRAME_BYTES;

const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
const JSON_MIME_TYPE: &str = "application/json";
const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const HEADER_LAST_EVENT_ID: &str = "Last-Event-Id";
const WIRE_LIMIT_MESSAGE: &str = "MCP HTTP response exceeded the wire limit";

#[derive(Clone)]
pub(super) struct BoundedReqwestMcpClient {
    inner: reqwest::Client,
    max_frame_bytes: usize,
}

impl BoundedReqwestMcpClient {
    pub(super) const fn new(inner: reqwest::Client) -> Self {
        Self {
            inner,
            max_frame_bytes: MAX_WIRE_FRAME_BYTES,
        }
    }

    async fn bounded_body(
        &self,
        response: reqwest::Response,
    ) -> Result<Vec<u8>, StreamableHttpError<reqwest::Error>> {
        match bounded_response_body(response, self.max_frame_bytes).await {
            Ok(body) => Ok(body),
            Err(BodyReadError::Request(error)) => Err(StreamableHttpError::Client(error)),
            Err(BodyReadError::Limit) => Err(wire_limit_error()),
        }
    }

    fn bounded_sse(
        &self,
        response: reqwest::Response,
    ) -> BoxStream<'static, Result<Sse, SseError>> {
        let guarded = response
            .bytes_stream()
            .scan(SseFrameBudget::new(self.max_frame_bytes), |budget, item| {
                ready(budget.accept(item))
            });
        SseStream::from_byte_stream(guarded).boxed()
    }
}

impl StreamableHttpClient for BoundedReqwestMcpClient {
    type Error = reqwest::Error;

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        let mut request = self
            .inner
            .get(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "))
            .header(HEADER_SESSION_ID, session_id.as_ref());
        if let Some(last_event_id) = last_event_id {
            request = request.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        if let Some(auth_token) = auth_token {
            request = request.bearer_auth(auth_token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        let response = response
            .error_for_status()
            .map_err(StreamableHttpError::Client)?;
        require_content_type(&response, EVENT_STREAM_MIME_TYPE)?;
        Ok(self.bounded_sse(response))
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let mut request = self
            .inner
            .delete(uri.as_ref())
            .header(HEADER_SESSION_ID, session_id.as_ref());
        if let Some(auth_token) = auth_token {
            request = request.bearer_auth(auth_token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        response
            .error_for_status()
            .map_err(StreamableHttpError::Client)?;
        Ok(())
    }

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let encoded = serde_json::to_vec(&message)?;
        if encoded.len() > self.max_frame_bytes {
            return Err(StreamableHttpError::UnexpectedServerResponse(
                Cow::Borrowed("MCP HTTP request exceeded the wire limit"),
            ));
        }
        let mut request = self
            .inner
            .post(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "))
            .header(CONTENT_TYPE, JSON_MIME_TYPE)
            .body(encoded);
        if let Some(auth_token) = auth_token {
            request = request.bearer_auth(auth_token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let session_was_attached = session_id.is_some();
        if let Some(session_id) = session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let Some(header) = response.headers().get(WWW_AUTHENTICATE)
        {
            return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
                header
                    .to_str()
                    .map_err(|_| invalid_auth_header())?
                    .to_string(),
            )));
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN
            && let Some(header) = response.headers().get(WWW_AUTHENTICATE)
        {
            let header = header.to_str().map_err(|_| invalid_auth_header())?;
            return Err(StreamableHttpError::InsufficientScope(
                InsufficientScopeError::new(header.to_string(), extract_scope(header)),
            ));
        }

        let status = response.status();
        if matches!(
            status,
            reqwest::StatusCode::ACCEPTED | reqwest::StatusCode::NO_CONTENT
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == reqwest::StatusCode::NOT_FOUND && session_was_attached {
            return Err(StreamableHttpError::SessionExpired);
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).to_string());
        let content_length = response.content_length();
        let session_id = response
            .headers()
            .get(HEADER_SESSION_ID)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        if status.is_success()
            && content_length == Some(0)
            && matches!(
                message,
                ClientJsonRpcMessage::Notification(_)
                    | ClientJsonRpcMessage::Response(_)
                    | ClientJsonRpcMessage::Error(_)
            )
        {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if !status.is_success() {
            let body = self.bounded_body(response).await?;
            if content_type
                .as_deref()
                .is_some_and(|value| content_type_is(value, JSON_MIME_TYPE))
                && let Ok(message @ JsonRpcMessage::Error(_)) =
                    serde_json::from_slice::<ServerJsonRpcMessage>(&body)
            {
                return Ok(StreamableHttpPostResponse::Json(message, session_id));
            }
            return Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                format!("HTTP {status}: {}", String::from_utf8_lossy(&body)),
            )));
        }
        match content_type.as_deref() {
            Some(value) if content_type_is(value, EVENT_STREAM_MIME_TYPE) => Ok(
                StreamableHttpPostResponse::Sse(self.bounded_sse(response), session_id),
            ),
            Some(value) if content_type_is(value, JSON_MIME_TYPE) => {
                let body = self.bounded_body(response).await?;
                match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                    Ok(message) => Ok(StreamableHttpPostResponse::Json(message, session_id)),
                    Err(_) => Ok(StreamableHttpPostResponse::Accepted),
                }
            }
            _ => Err(StreamableHttpError::UnexpectedContentType(content_type)),
        }
    }
}

fn apply_custom_headers(
    mut request: reqwest::RequestBuilder,
    custom_headers: HashMap<HeaderName, HeaderValue>,
) -> Result<reqwest::RequestBuilder, StreamableHttpError<reqwest::Error>> {
    for (name, value) in custom_headers {
        if ["accept", HEADER_SESSION_ID, HEADER_LAST_EVENT_ID]
            .iter()
            .any(|reserved| name.as_str().eq_ignore_ascii_case(reserved))
        {
            return Err(StreamableHttpError::ReservedHeaderConflict(
                name.to_string(),
            ));
        }
        request = request.header(name, value);
    }
    Ok(request)
}

fn require_content_type(
    response: &reqwest::Response,
    required: &str,
) -> Result<(), StreamableHttpError<reqwest::Error>> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).to_string());
    if content_type
        .as_deref()
        .is_some_and(|value| content_type_is(value, required))
    {
        Ok(())
    } else {
        Err(StreamableHttpError::UnexpectedContentType(content_type))
    }
}

fn content_type_is(value: &str, expected: &str) -> bool {
    value.as_bytes().starts_with(expected.as_bytes())
}

fn invalid_auth_header() -> StreamableHttpError<reqwest::Error> {
    StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
        "invalid www-authenticate header value",
    ))
}

fn extract_scope(header: &str) -> Option<String> {
    let lowercase = header.to_ascii_lowercase();
    let start = lowercase.find("scope=")? + "scope=".len();
    let value = &header[start..];
    if let Some(value) = value.strip_prefix('"') {
        return value.find('"').map(|end| value[..end].to_string());
    }
    let end = value
        .find(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .unwrap_or(value.len());
    (end > 0).then(|| value[..end].to_string())
}

fn wire_limit_error() -> StreamableHttpError<reqwest::Error> {
    StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(WIRE_LIMIT_MESSAGE))
}

#[derive(Debug, thiserror::Error)]
enum SseBodyError {
    #[error("MCP HTTP body failed")]
    Request(#[from] reqwest::Error),
    #[error("MCP SSE event exceeded the wire limit")]
    FrameLimit,
}

struct SseFrameBudget {
    current_bytes: usize,
    max_bytes: usize,
    tail: [u8; 3],
    tail_len: usize,
    terminal: bool,
}

impl SseFrameBudget {
    const fn new(max_bytes: usize) -> Self {
        Self {
            current_bytes: 0,
            max_bytes,
            tail: [0; 3],
            tail_len: 0,
            terminal: false,
        }
    }

    fn accept(
        &mut self,
        item: Result<bytes::Bytes, reqwest::Error>,
    ) -> Option<Result<bytes::Bytes, SseBodyError>> {
        if self.terminal {
            return None;
        }
        let bytes = match item {
            Ok(bytes) => bytes,
            Err(error) => {
                self.terminal = true;
                return Some(Err(SseBodyError::Request(error)));
            }
        };
        for byte in &bytes {
            self.current_bytes = self.current_bytes.saturating_add(1);
            if self.ends_event(*byte) {
                self.current_bytes = 0;
            } else if self.current_bytes > self.max_bytes {
                self.terminal = true;
                return Some(Err(SseBodyError::FrameLimit));
            }
            self.push_tail(*byte);
        }
        Some(Ok(bytes))
    }

    fn ends_event(&self, byte: u8) -> bool {
        (self.tail_len >= 1
            && ((self.tail[self.tail_len - 1] == b'\n' && byte == b'\n')
                || (self.tail[self.tail_len - 1] == b'\r' && byte == b'\r')))
            || (self.tail_len == 3 && self.tail == [b'\r', b'\n', b'\r'] && byte == b'\n')
    }

    fn push_tail(&mut self, byte: u8) {
        if self.tail_len < self.tail.len() {
            self.tail[self.tail_len] = byte;
            self.tail_len += 1;
        } else {
            self.tail.rotate_left(1);
            self.tail[2] = byte;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_body::append_bounded;

    #[test]
    fn sse_budget_bounds_each_event_without_bounding_the_stream() {
        let mut budget = SseFrameBudget::new(8);
        assert!(
            budget
                .accept(Ok(bytes::Bytes::from_static(b"data:")))
                .expect("first chunk")
                .is_ok()
        );
        assert!(matches!(
            budget.accept(Ok(bytes::Bytes::from_static(b" oversized"))),
            Some(Err(SseBodyError::FrameLimit))
        ));

        let mut budget = SseFrameBudget::new(10);
        for _ in 0..1_000 {
            assert!(
                budget
                    .accept(Ok(bytes::Bytes::from_static(b"data:x\n\n")))
                    .expect("event")
                    .is_ok()
            );
        }
    }

    #[test]
    fn chunked_json_body_is_rejected_at_the_cumulative_limit() {
        let mut body = Vec::new();
        assert!(append_bounded(&mut body, b"1234", 8));
        assert!(append_bounded(&mut body, b"5678", 8));
        assert!(!append_bounded(&mut body, b"9", 8));
        assert_eq!(body, b"12345678");
    }
}
