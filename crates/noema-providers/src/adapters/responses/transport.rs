//! HTTP transport for Responses-compatible endpoints.

use std::fmt;

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Serialize;
use serde_json::Value;

use super::{ResponsesDiagnosticContext, ResponsesResponse, sse::SseAccumulator};
use crate::response_support::http::{error_from_status, normalize_base_url, request_id};
use crate::{GenerateStreamEvent, ProviderError, reqwest_transport_error};

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

        let transport_provider = diagnostics.provider_kind.clone();
        let mut accumulator = SseAccumulator::new(diagnostics);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| {
                reqwest_transport_error(&transport_provider, "read_generation_stream", &source)
            })?;
            accumulator.push_bytes(&chunk, on_event)?;
        }

        accumulator.finish(on_event)
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
