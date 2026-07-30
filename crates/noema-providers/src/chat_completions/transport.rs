use super::ChatCompletionResponse;
use super::sse::ChatSseAccumulator;
use crate::response_support::StructuredResponseDiagnosticContext;
use crate::response_support::http::{error_from_status, normalize_base_url, request_id};
use crate::{GenerateStreamEvent, ProviderError, reqwest_transport_error};
use futures_util::StreamExt;
use reqwest::header::HeaderMap;
use serde::Serialize;
pub(crate) type ChatDiagnosticContext = StructuredResponseDiagnosticContext;

#[derive(Clone)]
pub(crate) struct ChatTransport {
    client: reqwest::Client,
    chat_url: String,
}

impl ChatTransport {
    pub(crate) fn new(
        client: reqwest::Client,
        base_url: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let base_url = normalize_base_url(base_url.into(), "chat completions base URL")?;
        Ok(Self {
            client,
            chat_url: format!("{base_url}/chat/completions"),
        })
    }
    pub(crate) async fn send_streaming<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
        diagnostics: ChatDiagnosticContext,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ChatCompletionResponse, ProviderError>
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
        let mut accumulator = ChatSseAccumulator::new();
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
                provider: "chat_completions".to_string(),
                credential: "bearer_token".to_string(),
            });
        }
        Ok(extra_headers.iter().fold(
            self.client
                .post(&self.chat_url)
                .bearer_auth(bearer_token)
                .json(&body),
            |request, (name, value)| request.header(name, value),
        ))
    }
}
