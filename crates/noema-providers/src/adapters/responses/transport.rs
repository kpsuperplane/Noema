//! HTTP transport for Responses-compatible endpoints.

use std::fmt;

use futures_util::{SinkExt, StreamExt};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Serialize;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use super::{ResponsesDiagnosticContext, ResponsesResponse, sse::ResponsesAccumulator};
use crate::response_support::http::{error_from_status, normalize_base_url, request_id};
use crate::{GenerateStreamEvent, ProviderError, ProviderTimingMilestone, reqwest_transport_error};

/// HTTP transport for a Responses-compatible endpoint.
#[derive(Clone)]
pub struct ResponsesTransport {
    client: reqwest::Client,
    responses_url: String,
}

impl fmt::Debug for ResponsesTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponsesTransport")
            .field("client", &"[CONFIGURED]")
            .field("responses_url", &self.responses_url)
            .finish()
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
        let response = self
            .request(bearer_token, body, &extra_headers)?
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(&diagnostics.provider_kind, "send_generation", &source)
            })?;
        let status = response.status();
        let request_id = request_id(response.headers());
        let body_text = response.text().await.map_err(|source| {
            reqwest_transport_error(&diagnostics.provider_kind, "read_generation_body", &source)
        })?;

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
        let response = self
            .request(bearer_token, body, &extra_headers)?
            .send()
            .await
            .map_err(|source| {
                reqwest_transport_error(
                    &diagnostics.provider_kind,
                    "send_streaming_generation",
                    &source,
                )
            })?;
        let status = response.status();
        let request_id = request_id(response.headers());

        if !status.is_success() {
            let body_text = response.text().await.map_err(|source| {
                reqwest_transport_error(
                    &diagnostics.provider_kind,
                    "read_generation_error_body",
                    &source,
                )
            })?;
            return Err(error_from_status(status, request_id, &body_text));
        }
        on_event(GenerateStreamEvent::ProviderTiming {
            milestone: ProviderTimingMilestone::ResponseHeaders,
            output_index: None,
        });

        let transport_provider = diagnostics.provider_kind.clone();
        let mut accumulator = ResponsesAccumulator::new(diagnostics);
        let mut stream = response.bytes_stream();
        let mut body_started = false;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| {
                reqwest_transport_error(&transport_provider, "read_generation_stream", &source)
            })?;
            if !body_started {
                body_started = true;
                on_event(GenerateStreamEvent::ProviderTiming {
                    milestone: ProviderTimingMilestone::ResponseBodyStarted,
                    output_index: None,
                });
            }
            accumulator.push_bytes(&chunk, on_event)?;
        }

        accumulator.finish(on_event)
    }

    /// Open a lazy WebSocket session for this Responses endpoint.
    pub(crate) fn websocket_session(&self) -> ResponsesWebSocketSession {
        ResponsesWebSocketSession {
            websocket_url: self
                .responses_url
                .replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1),
            socket: None,
            unsupported: false,
            previous_response_id: None,
            fingerprint: None,
            metadata: crate::ProviderGenerationMetadata::default(),
        }
    }

    fn request(
        &self,
        bearer_token: &str,
        body: impl Serialize,
        extra_headers: &HeaderMap,
    ) -> Result<reqwest::RequestBuilder, ProviderError> {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "responses".to_string(),
                credential: "bearer_token".to_string(),
            });
        }
        Ok(extra_headers.iter().fold(
            self.client
                .post(&self.responses_url)
                .bearer_auth(bearer_token)
                .json(&body),
            |request, (name, value)| request.header(name, value),
        ))
    }
}

type ResponsesSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// One lazy Responses WebSocket connection.
pub(crate) struct ResponsesWebSocketSession {
    websocket_url: String,
    socket: Option<ResponsesSocket>,
    unsupported: bool,
    previous_response_id: Option<String>,
    fingerprint: Option<Value>,
    metadata: crate::ProviderGenerationMetadata,
}

pub(crate) struct PreparedResponsesRequest {
    pub(crate) body: super::ResponsesRequest,
    fingerprint: Value,
    pub(crate) used_response_id: bool,
    input_mode: &'static str,
}

/// A WebSocket failure classified for safe HTTP fallback.
pub(crate) enum ResponsesWebSocketError {
    /// The endpoint definitively does not support this transport.
    Unsupported(ProviderError),
    /// Authentication failed while the connection was established.
    Authentication(ProviderError),
    /// The connection failed before provider output arrived.
    Setup(ProviderError),
    /// The provider no longer has the referenced response.
    PreviousResponseNotFound,
    /// The provider rejected the request without permitting transport fallback.
    Fatal(ProviderError),
    /// Provider output arrived, so automatic replay is unsafe.
    AfterOutput(ProviderError),
}

impl ResponsesWebSocketError {
    pub(crate) fn into_provider_error(self) -> ProviderError {
        match self {
            Self::Unsupported(error)
            | Self::Authentication(error)
            | Self::Setup(error)
            | Self::Fatal(error)
            | Self::AfterOutput(error) => error,
            Self::PreviousResponseNotFound => ProviderError::ProtocolError {
                provider: "responses".to_string(),
                message: "the previous response is no longer available".to_string(),
            },
        }
    }
}

impl ResponsesWebSocketSession {
    pub(crate) fn prepare_request(
        &self,
        mut replay: super::ResponsesRequest,
        incremental: Option<super::ResponsesRequest>,
    ) -> Result<PreparedResponsesRequest, ProviderError> {
        let fingerprint = replay.continuation_fingerprint()?;
        let can_continue =
            self.fingerprint.as_ref() == Some(&fingerprint) && self.previous_response_id.is_some();
        let (mut body, used_response_id) = match (can_continue, incremental) {
            (true, Some(body)) => (body, true),
            _ => {
                replay.previous_response_id = None;
                (replay, false)
            }
        };
        body.previous_response_id = used_response_id
            .then(|| self.previous_response_id.clone())
            .flatten();
        body.use_websocket_events();
        Ok(PreparedResponsesRequest {
            body,
            fingerprint,
            used_response_id,
            input_mode: if used_response_id {
                "incremental"
            } else if self.fingerprint.is_some() {
                "replay"
            } else {
                "full"
            },
        })
    }

    pub(crate) fn begin_request(&mut self, prepared: &PreparedResponsesRequest) {
        self.metadata = crate::ProviderGenerationMetadata {
            transport: Some("responses_websocket"),
            input_mode: Some(prepared.input_mode),
            fallback_reason: None,
            used_response_id: prepared.used_response_id,
        };
    }

    pub(crate) fn use_http(&mut self, reason: &'static str, used_response_id: bool) {
        self.metadata.transport = Some("responses_http");
        self.metadata.fallback_reason = Some(reason);
        self.metadata.used_response_id = used_response_id;
        if !used_response_id && self.metadata.input_mode == Some("incremental") {
            self.metadata.input_mode = Some("replay");
        }
    }

    pub(crate) fn replay_missing_response(&mut self) {
        self.metadata.input_mode = Some("replay");
        self.metadata.fallback_reason = Some("previous_response_not_found");
        self.metadata.used_response_id = false;
    }

    pub(crate) fn metadata(&self) -> crate::ProviderGenerationMetadata {
        self.metadata
    }

    pub(crate) fn record_response(
        &mut self,
        prepared: &PreparedResponsesRequest,
        response: &ResponsesResponse,
    ) {
        self.fingerprint = Some(prepared.fingerprint.clone());
        self.previous_response_id.clone_from(&response.id);
    }

    pub(crate) fn clear_response_id(&mut self) {
        self.previous_response_id = None;
    }

    /// Send one request and collect Responses events through the shared accumulator.
    pub(crate) async fn send(
        &mut self,
        bearer_token: &str,
        body: &impl Serialize,
        extra_headers: &HeaderMap,
        diagnostics: ResponsesDiagnosticContext,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ResponsesResponse, ResponsesWebSocketError> {
        if self.unsupported {
            return Err(ResponsesWebSocketError::Unsupported(websocket_failure(
                &diagnostics.provider_kind,
                "responses WebSocket is unsupported",
                None,
            )));
        }
        if self.socket.is_none() {
            self.connect(bearer_token, extra_headers, &diagnostics)
                .await?;
        }
        let mut value = serde_json::to_value(body).map_err(|_| {
            ResponsesWebSocketError::Fatal(ProviderError::InvalidRequest {
                message: "failed to encode Responses WebSocket request".to_string(),
            })
        })?;
        value
            .as_object_mut()
            .ok_or_else(|| {
                ResponsesWebSocketError::Fatal(ProviderError::InvalidRequest {
                    message: "Responses WebSocket request must be an object".to_string(),
                })
            })?
            .insert(
                "type".to_string(),
                Value::String("response.create".to_string()),
            );
        let encoded = serde_json::to_string(&value).map_err(|_| {
            ResponsesWebSocketError::Fatal(ProviderError::InvalidRequest {
                message: "failed to encode Responses WebSocket request".to_string(),
            })
        })?;
        let mut socket = self.socket.take().expect("WebSocket connected");
        socket
            .send(Message::Text(encoded.into()))
            .await
            .map_err(|_| {
                ResponsesWebSocketError::Setup(websocket_failure(
                    &diagnostics.provider_kind,
                    "failed to send the Responses WebSocket request",
                    None,
                ))
            })?;

        let mut accumulator = ResponsesAccumulator::new(diagnostics.clone());
        let mut saw_output = false;
        loop {
            let message = socket.next().await.ok_or_else(|| {
                websocket_read_error(&diagnostics.provider_kind, saw_output, "connection closed")
            })?;
            let message = message.map_err(|_| {
                websocket_read_error(&diagnostics.provider_kind, saw_output, "read failed")
            })?;
            let text = match message {
                Message::Text(text) => text.to_string(),
                Message::Binary(bytes) => String::from_utf8(bytes.to_vec()).map_err(|_| {
                    websocket_read_error(
                        &diagnostics.provider_kind,
                        saw_output,
                        "received non-UTF-8 data",
                    )
                })?,
                Message::Ping(payload) => {
                    socket.send(Message::Pong(payload)).await.map_err(|_| {
                        websocket_read_error(
                            &diagnostics.provider_kind,
                            saw_output,
                            "failed to answer a ping",
                        )
                    })?;
                    continue;
                }
                Message::Pong(_) | Message::Frame(_) => continue,
                Message::Close(_) => {
                    self.socket = None;
                    return Err(websocket_read_error(
                        &diagnostics.provider_kind,
                        saw_output,
                        "connection closed",
                    ));
                }
            };
            let value = serde_json::from_str::<Value>(&text).map_err(|_| {
                websocket_read_error(
                    &diagnostics.provider_kind,
                    saw_output,
                    "received malformed JSON",
                )
            })?;
            let event_type = value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            saw_output |= websocket_event_has_output(&event_type);
            if response_error_code(&value) == Some("previous_response_not_found") {
                self.socket = Some(socket);
                return if saw_output {
                    Err(ResponsesWebSocketError::AfterOutput(
                        ProviderError::ProtocolError {
                            provider: diagnostics.provider_kind.clone(),
                            message: "the previous response became unavailable after output began"
                                .to_string(),
                        },
                    ))
                } else {
                    Err(ResponsesWebSocketError::PreviousResponseNotFound)
                };
            }
            accumulator
                .push_value(value, None, on_event)
                .map_err(|error| websocket_provider_error(error, saw_output))?;
            if matches!(
                event_type.as_str(),
                "response.completed" | "response.incomplete" | "response.failed" | "error"
            ) {
                let result = accumulator
                    .finish(on_event)
                    .map_err(|error| websocket_provider_error(error, saw_output));
                self.socket = Some(socket);
                return result;
            }
        }
    }

    async fn connect(
        &mut self,
        bearer_token: &str,
        extra_headers: &HeaderMap,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> Result<(), ResponsesWebSocketError> {
        install_crypto_provider();
        let mut request = self
            .websocket_url
            .as_str()
            .into_client_request()
            .map_err(|_| {
                ResponsesWebSocketError::Fatal(ProviderError::InvalidRequest {
                    message: "invalid Responses WebSocket endpoint".to_string(),
                })
            })?;
        let authorization =
            HeaderValue::from_str(&format!("Bearer {bearer_token}")).map_err(|_| {
                ResponsesWebSocketError::Authentication(ProviderError::AuthenticationFailure {
                    message: "invalid provider credential".to_string(),
                    request_id: None,
                })
            })?;
        request.headers_mut().insert(AUTHORIZATION, authorization);
        for (name, value) in extra_headers {
            request.headers_mut().insert(name, value.clone());
        }
        match connect_async(request).await {
            Ok((socket, _)) => {
                self.socket = Some(socket);
                Ok(())
            }
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                let status = response.status().as_u16();
                let error = websocket_failure(
                    &diagnostics.provider_kind,
                    "Responses WebSocket handshake failed",
                    Some(status),
                );
                if matches!(status, 404 | 405 | 426 | 501) {
                    self.unsupported = true;
                    Err(ResponsesWebSocketError::Unsupported(error))
                } else if matches!(status, 401 | 403) {
                    Err(ResponsesWebSocketError::Authentication(error))
                } else if status == 429 {
                    Err(ResponsesWebSocketError::Fatal(ProviderError::RateLimit {
                        message: "Responses WebSocket handshake was rate limited".to_string(),
                        request_id: None,
                    }))
                } else {
                    Err(ResponsesWebSocketError::Fatal(error))
                }
            }
            Err(_) => Err(ResponsesWebSocketError::Setup(websocket_failure(
                &diagnostics.provider_kind,
                "Responses WebSocket connection failed",
                None,
            ))),
        }
    }
}

fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }
}

fn websocket_event_has_output(event_type: &str) -> bool {
    matches!(
        event_type,
        "response.output_text.delta"
            | "response.output_item.added"
            | "response.output_item.done"
            | "response.web_search_call.in_progress"
            | "response.web_search_call.searching"
            | "response.web_search_call.completed"
    )
}

fn websocket_provider_error(error: ProviderError, saw_output: bool) -> ResponsesWebSocketError {
    if saw_output {
        ResponsesWebSocketError::AfterOutput(error)
    } else {
        ResponsesWebSocketError::Fatal(error)
    }
}

fn websocket_read_error(
    provider: &str,
    saw_output: bool,
    message: &str,
) -> ResponsesWebSocketError {
    if saw_output {
        ResponsesWebSocketError::AfterOutput(websocket_failure(provider, message, None))
    } else {
        ResponsesWebSocketError::Setup(websocket_failure(provider, message, None))
    }
}

fn response_error_code(value: &Value) -> Option<&str> {
    value
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("error"))
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str)
        })
}

fn websocket_failure(provider: &str, message: &str, status_code: Option<u16>) -> ProviderError {
    ProviderError::TransportFailure {
        provider: provider.to_string(),
        kind: crate::ProviderTransportKind::Connection,
        message: message.to_string(),
        context: crate::ProviderTransportContext {
            operation: "responses_websocket".to_string(),
            status_code,
            request_id: None,
        },
    }
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
