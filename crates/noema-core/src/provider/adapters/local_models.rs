//! First-party local GGUF provider backed by a supervised llama.cpp server.

use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    LocalModelsProviderConfig,
    local_models::{
        LlamaServerConfig, LlamaServerError, LlamaServerSupervisor,
        bundled_llama_server_candidates_in,
    },
    provider::{
        GenerateInput, GenerateInputItem, GenerateMessageRole, GenerateRequest, GenerateResponse,
        GenerateResponseStatus, GenerateStreamEvent, ModelProvider, ParsedNoemaResponse,
        ProviderContextMetadata, ProviderError, ProviderToolCapabilities, ProviderToolFallbackMode,
        TokenUsage, output_items_from_text, required_noema_response_from_text,
    },
};

use super::{
    noema_response_stream::NoemaAssistantTextDeltaExtractor, responses::ResponsesDiagnosticContext,
};

/// Stable provider identifier for first-party local GGUF inference.
pub const LOCAL_MODELS_PROVIDER: &str = "local_models";
/// Default output reserve used by prompt planning for local models.
pub const LOCAL_MODELS_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 1_024;
/// Default compact-summary target for local models.
pub const LOCAL_MODELS_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 768;

/// Local model provider facade that preserves Noema's provider-neutral contracts.
#[derive(Clone, Debug)]
pub struct LocalModelsProvider {
    config: LocalModelsProviderConfig,
    supervisor: LlamaServerSupervisor,
    client: reqwest::Client,
}

impl LocalModelsProvider {
    /// Builds a provider for one installed, checksum-verified GGUF model.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when installation or runtime configuration is invalid.
    pub fn new(config: LocalModelsProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let model_path =
            config
                .model_path
                .clone()
                .ok_or_else(|| ProviderError::ProviderUnavailable {
                    provider: LOCAL_MODELS_PROVIDER.to_string(),
                    message: "no installed local model is active".to_string(),
                })?;
        let candidates = bundled_llama_server_candidates_in(
            config.preferred_backend,
            config.runtime_root.as_deref(),
        );
        let supervisor = LlamaServerSupervisor::new(LlamaServerConfig {
            model_id: config.default_model.clone(),
            model_path,
            candidates,
            context_window_tokens: config.context_window_tokens,
            startup_timeout: Duration::from_secs(config.startup_timeout_seconds),
        })
        .map_err(runtime_unavailable)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;
        Ok(Self {
            config,
            supervisor,
            client,
        })
    }

    /// Returns the supervised runtime for status subscriptions and explicit retries.
    #[must_use]
    pub fn runtime(&self) -> &LlamaServerSupervisor {
        &self.supervisor
    }

    fn selected_model(&self, requested: Option<&str>) -> Result<String, ProviderError> {
        let requested = requested
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or(&self.config.default_model);
        if requested != self.config.default_model {
            return Err(ProviderError::InvalidRequest {
                message: format!(
                    "local model `{requested}` is not loaded; active model is `{}`",
                    self.config.default_model
                ),
            });
        }
        Ok(requested.to_string())
    }

    async fn send_chat_stream(
        &self,
        request: &GenerateRequest,
        model: &str,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ChatStreamOutput, ProviderError> {
        let _generation_permit = self
            .supervisor
            .acquire_generation()
            .await
            .map_err(runtime_unavailable)?;
        let endpoint = self
            .supervisor
            .ensure_ready()
            .await
            .map_err(runtime_unavailable)?;
        let url = endpoint
            .base_url
            .join("v1/chat/completions")
            .map_err(|error| ProviderError::InvalidRequest {
                message: format!("invalid local inference endpoint: {error}"),
            })?;
        let body = ChatCompletionRequest::from_generate(request, model.to_string())?;
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|error| error.to_string());
            return Err(ProviderError::ApiError {
                status,
                message,
                request_id: None,
            });
        }

        let mut stream = response.bytes_stream();
        let mut accumulator = ChatSseAccumulator::default();
        let mut structured_extractor = NoemaAssistantTextDeltaExtractor::default();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| ProviderError::HttpFailure { source })?;
            accumulator.push_bytes(&chunk, |delta| {
                if request.options.require_noema_response {
                    structured_extractor.push_delta(&delta, on_event);
                } else {
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        response_index: 0,
                        delta,
                    });
                }
            })?;
        }
        accumulator.finish()
    }
}

fn normalize_config(
    mut config: LocalModelsProviderConfig,
) -> Result<LocalModelsProviderConfig, ProviderError> {
    config.default_model = config.default_model.trim().to_string();
    if config.default_model.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "local models default model cannot be empty".to_string(),
        });
    }
    if config.context_window_tokens == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "local models context window must be greater than zero".to_string(),
        });
    }
    if config.timeout_seconds == 0 || config.startup_timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "local models timeouts must be greater than zero".to_string(),
        });
    }
    Ok(config)
}

fn runtime_unavailable(error: LlamaServerError) -> ProviderError {
    ProviderError::ProviderUnavailable {
        provider: LOCAL_MODELS_PROVIDER.to_string(),
        message: error.to_string(),
    }
}

impl ModelProvider for LocalModelsProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let mut ignore_event = |_| {};
        self.generate_streaming(request, &mut ignore_event).await
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        Some(self.config.default_model.clone())
    }

    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(self.config.context_window_tokens),
            default_output_reserve_tokens: Some(LOCAL_MODELS_DEFAULT_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(LOCAL_MODELS_COMPACT_SUMMARY_TARGET_TOKENS),
        }
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            fallback_mode: ProviderToolFallbackMode::BuiltinOnlyEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    async fn count_tokens(
        &self,
        instructions: Option<&str>,
        input: &str,
        model: Option<&str>,
    ) -> Result<Option<u32>, ProviderError> {
        let _model = self.selected_model(model)?;
        let endpoint = self
            .supervisor
            .ensure_ready()
            .await
            .map_err(runtime_unavailable)?;
        let url =
            endpoint
                .base_url
                .join("tokenize")
                .map_err(|error| ProviderError::InvalidRequest {
                    message: format!("invalid local tokenizer endpoint: {error}"),
                })?;
        let content = match instructions.filter(|instructions| !instructions.trim().is_empty()) {
            Some(instructions) => format!("{instructions}\n\n{input}"),
            None => input.to_string(),
        };
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({ "content": content }))
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let body = response
            .json::<TokenizeResponse>()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let count = body
            .count
            .or_else(|| body.tokens.map(|tokens| tokens.len()))
            .and_then(|count| u32::try_from(count).ok());
        Ok(count)
    }

    async fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let model = self.selected_model(request.model.as_deref())?;
        let diagnostics = ResponsesDiagnosticContext::new(
            self.config.system_errors.clone(),
            LOCAL_MODELS_PROVIDER,
            model.clone(),
            request.conversation_id.clone(),
        );
        let stream = self.send_chat_stream(&request, &model, on_event).await?;
        let parsed = if request.options.require_noema_response {
            required_noema_response_from_text(stream.text.clone()).inspect_err(|error| {
                diagnostics.log_malformed(
                    error.to_string(),
                    serde_json::json!({ "provider_text": stream.text }),
                );
            })?
        } else {
            ParsedNoemaResponse {
                responses: output_items_from_text(stream.text)?,
                tool_calls: Vec::new(),
                response_status: GenerateResponseStatus::Final,
            }
        };
        Ok(GenerateResponse::from_parsed(
            parsed,
            LOCAL_MODELS_PROVIDER,
            stream.model.unwrap_or(model),
            stream.response_id,
            stream.usage,
        ))
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    stream_options: ChatStreamOptions,
    cache_prompt: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

impl ChatCompletionRequest {
    fn from_generate(request: &GenerateRequest, model: String) -> Result<Self, ProviderError> {
        let mut messages = Vec::new();
        if let Some(instructions) = request
            .instructions
            .as_deref()
            .filter(|instructions| !instructions.trim().is_empty())
        {
            messages.push(ChatMessage::new("system", instructions));
        }
        append_generate_input(&mut messages, &request.input);
        if messages.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "local model request must contain model-visible input".to_string(),
            });
        }
        Ok(Self {
            model,
            messages,
            stream: true,
            stream_options: ChatStreamOptions {
                include_usage: true,
            },
            cache_prompt: true,
            max_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatStreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

impl ChatMessage {
    fn new(role: &'static str, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

fn append_generate_input(messages: &mut Vec<ChatMessage>, input: &GenerateInput) {
    match input {
        GenerateInput::Text(text) => messages.push(ChatMessage::new("user", text)),
        GenerateInput::Messages(input_messages) => {
            messages.extend(input_messages.iter().map(|message| {
                ChatMessage::new(
                    match message.role {
                        GenerateMessageRole::System => "system",
                        GenerateMessageRole::User => "user",
                        GenerateMessageRole::Assistant => "assistant",
                    },
                    &message.content,
                )
            }))
        }
        GenerateInput::Items(items) => messages.extend(items.iter().map(|item| match item {
            GenerateInputItem::Message(message) => ChatMessage::new(
                match message.role {
                    GenerateMessageRole::System => "system",
                    GenerateMessageRole::User => "user",
                    GenerateMessageRole::Assistant => "assistant",
                },
                &message.content,
            ),
            GenerateInputItem::Reasoning(_) | GenerateInputItem::ToolCall(_) => {
                ChatMessage::new("assistant", item.render_for_token_count())
            }
            GenerateInputItem::ToolResult(_) => {
                ChatMessage::new("user", item.render_for_token_count())
            }
        })),
        GenerateInput::NativeToolResults(_) => {
            messages.push(ChatMessage::new("user", input.render_for_token_count()));
        }
    }
}

#[derive(Debug, Default)]
struct ChatSseAccumulator {
    pending: Vec<u8>,
    text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<TokenUsage>,
}

#[derive(Debug)]
struct ChatStreamOutput {
    text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<TokenUsage>,
}

impl ChatSseAccumulator {
    fn push_bytes(
        &mut self,
        bytes: &[u8],
        mut on_delta: impl FnMut(String),
    ) -> Result<(), ProviderError> {
        self.pending.extend_from_slice(bytes);
        while let Some((index, delimiter_len)) = next_event_boundary(&self.pending) {
            let raw = self.pending[..index].to_vec();
            self.pending.drain(..index + delimiter_len);
            self.handle_event(&raw, &mut on_delta)?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<ChatStreamOutput, ProviderError> {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.handle_event(&pending, &mut |_| {})?;
        }
        if self.text.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "local model produced empty output".to_string(),
            });
        }
        Ok(ChatStreamOutput {
            text: self.text,
            response_id: self.response_id,
            model: self.model,
            usage: self.usage,
        })
    }

    fn handle_event(
        &mut self,
        raw: &[u8],
        on_delta: &mut impl FnMut(String),
    ) -> Result<(), ProviderError> {
        let raw = std::str::from_utf8(raw).map_err(|error| ProviderError::MalformedResponse {
            message: format!("local model stream was not UTF-8: {error}"),
        })?;
        let data = raw
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            return Ok(());
        }
        let chunk: ChatCompletionChunk =
            serde_json::from_str(&data).map_err(|error| ProviderError::MalformedResponse {
                message: format!("invalid local model stream event: {error}"),
            })?;
        if self.response_id.is_none() {
            self.response_id = chunk.id;
        }
        if self.model.is_none() {
            self.model = chunk.model;
        }
        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.into());
        }
        for choice in chunk.choices {
            if let Some(delta) = choice.delta.content.filter(|delta| !delta.is_empty()) {
                self.text.push_str(&delta);
                on_delta(delta);
            }
        }
        Ok(())
    }
}

fn next_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2))
        .or_else(|| {
            bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| (index, 4))
        })
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

impl From<ChatUsage> for TokenUsage {
    fn from(usage: ChatUsage) -> Self {
        Self {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            cached_input_tokens: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenizeResponse {
    #[serde(default)]
    tokens: Option<Vec<Value>>,
    #[serde(default)]
    count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{GenerateInputItem, GenerateMessage, GenerateToolResultInput};

    #[test]
    fn chat_request_preserves_replay_items_and_generation_controls() {
        let mut request = GenerateRequest::text("ignored");
        request.instructions = Some("system rules".to_string());
        request.input = GenerateInput::Items(vec![
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::User,
                content: "question".to_string(),
            }),
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::Assistant,
                content: "answer".to_string(),
            }),
            GenerateInputItem::ToolResult(GenerateToolResultInput {
                id: None,
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                provider_name: None,
                arguments: Value::Null,
                success: true,
                payload: serde_json::json!({ "text": "contents" }),
            }),
        ]);
        request.options.max_output_tokens = Some(321);
        request.options.temperature = Some(0.2);

        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");

        assert_eq!(body.messages.len(), 4);
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[2].role, "assistant");
        assert_eq!(body.messages[3].role, "user");
        assert_eq!(body.max_tokens, Some(321));
        assert_eq!(body.temperature, Some(0.2));
        assert!(body.cache_prompt);
    }

    #[test]
    fn chat_sse_accumulator_streams_deltas_and_usage_across_chunks() {
        let mut accumulator = ChatSseAccumulator::default();
        let mut deltas = Vec::new();
        accumulator
            .push_bytes(
                b"data: {\"id\":\"chat-1\",\"model\":\"local-8b\",\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
                |delta| deltas.push(delta),
            )
            .expect("first event");
        accumulator
            .push_bytes(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\ndata: [DONE]\n\n",
                |delta| deltas.push(delta),
            )
            .expect("remaining events");

        let output = accumulator.finish().expect("stream output");

        assert_eq!(deltas, vec!["hel", "lo"]);
        assert_eq!(output.text, "hello");
        assert_eq!(output.response_id.as_deref(), Some("chat-1"));
        assert_eq!(output.model.as_deref(), Some("local-8b"));
        assert_eq!(
            output.usage,
            Some(TokenUsage {
                input_tokens: 3,
                output_tokens: 2,
                total_tokens: 5,
                cached_input_tokens: None,
            })
        );
    }

    #[test]
    fn chat_sse_accumulator_rejects_empty_output() {
        let mut accumulator = ChatSseAccumulator::default();
        accumulator
            .push_bytes(b"data: [DONE]\n\n", |_| {})
            .expect("done event");

        assert!(matches!(
            accumulator.finish(),
            Err(ProviderError::MalformedResponse { .. })
        ));
    }
}
