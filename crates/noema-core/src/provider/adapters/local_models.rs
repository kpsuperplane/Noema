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
        ProviderContextMetadata, ProviderError, ProviderToolCapabilities, ProviderToolTransport,
        TokenUsage, output_items_from_text, required_noema_response_from_text,
    },
};

use super::{
    noema_response_stream::NoemaAssistantTextDeltaExtractor,
    responses::{ResponsesDiagnosticContext, noema_response_text_format},
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
            tool_transport: ProviderToolTransport::NoemaEnvelope,
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
            required_local_noema_response_from_text(&stream.text).inspect_err(|error| {
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

fn required_local_noema_response_from_text(
    text: &str,
) -> Result<ParsedNoemaResponse, ProviderError> {
    let mut value: Value =
        serde_json::from_str(text).map_err(|_| ProviderError::MalformedResponse {
            message: "provider did not return a Noema structured response object".to_string(),
        })?;
    let has_tool_calls = value
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|calls| !calls.is_empty());
    if has_tool_calls && let Some(object) = value.as_object_mut() {
        object.insert(
            "response_status".to_string(),
            Value::String("needs_tools".to_string()),
        );
        object.insert("responses".to_string(), Value::Array(Vec::new()));
    }
    required_noema_response_from_text(value.to_string())
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    stream_options: ChatStreamOptions,
    cache_prompt: bool,
    chat_template_kwargs: ChatTemplateKwargs,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<Value>,
}

impl ChatCompletionRequest {
    fn from_generate(request: &GenerateRequest, model: String) -> Result<Self, ProviderError> {
        let mut messages = Vec::new();
        if let Some(instructions) = request
            .instructions
            .as_deref()
            .filter(|instructions| !instructions.trim().is_empty())
        {
            push_chat_message(&mut messages, "system", instructions);
        }
        append_generate_input(&mut messages, &request.input);
        if messages.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "local model request must contain model-visible input".to_string(),
            });
        }
        let response_format = request
            .options
            .require_noema_response
            .then(|| chat_noema_response_format(request))
            .transpose()?;
        Ok(Self {
            model,
            messages,
            stream: true,
            stream_options: ChatStreamOptions {
                include_usage: true,
            },
            cache_prompt: true,
            chat_template_kwargs: ChatTemplateKwargs {
                enable_thinking: false,
            },
            max_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
            response_format,
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

fn chat_noema_response_format(request: &GenerateRequest) -> Result<Value, ProviderError> {
    let format = noema_response_text_format()
        .get("format")
        .cloned()
        .expect("shared Noema response format");
    let mut response_format = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": format["name"],
            "strict": format["strict"],
            "schema": format["schema"]
        }
    });
    let selected_tools = selected_local_tools(request)?;
    let tools_allowed = !selected_tools.is_empty();
    let schema = &mut response_format["json_schema"]["schema"];
    if tools_allowed {
        let tool_schemas = selected_tools
            .into_iter()
            .map(|tool| {
                let mut input_schema = tool.input_schema.as_value().clone();
                normalize_llama_cpp_schema(&mut input_schema);
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": ["string", "null"]},
                        "name": {"type": "string", "enum": [tool.name.as_str()]},
                        "payload": input_schema
                    },
                    "required": ["name", "payload"],
                    "additionalProperties": false
                })
            })
            .collect::<Vec<_>>();
        schema["properties"]["tool_calls"]["items"] = serde_json::json!({"oneOf": tool_schemas});
    }
    if !tools_allowed {
        schema["properties"]["response_status"]["enum"] = serde_json::json!(["final"]);
        schema["properties"]["responses"]["minItems"] = serde_json::json!(1);
        schema["properties"]["tool_calls"]["maxItems"] = serde_json::json!(0);
    }
    let tool_call_required = matches!(
        &request.tool_choice,
        crate::provider::NoemaToolChoice::Required
            | crate::provider::NoemaToolChoice::Allowed(crate::provider::NoemaAllowedTools {
                mode: crate::provider::NoemaAllowedToolsMode::Required,
                ..
            })
    );
    if tool_call_required {
        schema["properties"]["response_status"]["enum"] = serde_json::json!(["needs_tools"]);
        schema["properties"]["responses"]["maxItems"] = serde_json::json!(0);
        schema["properties"]["tool_calls"]["minItems"] = serde_json::json!(1);
    }
    if tools_allowed && !request.parallel_tool_calls {
        schema["properties"]["tool_calls"]["maxItems"] = serde_json::json!(1);
    }
    Ok(response_format)
}

fn selected_local_tools(
    request: &GenerateRequest,
) -> Result<Vec<&crate::provider::NoemaToolSpec>, ProviderError> {
    match &request.tool_choice {
        crate::provider::NoemaToolChoice::None => Ok(Vec::new()),
        crate::provider::NoemaToolChoice::Required if request.tools.is_empty() => {
            Err(ProviderError::InvalidRequest {
                message: "required tool choice needs a non-empty tool catalog".to_string(),
            })
        }
        crate::provider::NoemaToolChoice::Auto | crate::provider::NoemaToolChoice::Required => {
            Ok(request.tools.iter().collect())
        }
        crate::provider::NoemaToolChoice::Allowed(allowed) => {
            if allowed.tools.is_empty() {
                return Err(ProviderError::InvalidRequest {
                    message: "allowed tools cannot be empty".to_string(),
                });
            }
            let mut seen = std::collections::HashSet::with_capacity(allowed.tools.len());
            let mut selected = Vec::with_capacity(allowed.tools.len());
            for allowed_name in &allowed.tools {
                if !seen.insert(allowed_name.as_str()) {
                    return Err(ProviderError::InvalidRequest {
                        message: format!("allowed tool {allowed_name} is duplicated"),
                    });
                }
                let Some(tool) = request.tools.iter().find(|tool| &tool.name == allowed_name)
                else {
                    return Err(ProviderError::InvalidRequest {
                        message: format!(
                            "allowed tool {allowed_name} is not present in the request tool catalog"
                        ),
                    });
                };
                selected.push(tool);
            }
            Ok(selected)
        }
    }
}

fn normalize_llama_cpp_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            // llama.cpp lowers JSON Schema string constraints into grammar
            // productions. Large length bounds make the generated grammar too
            // large, while otherwise-valid expressions such as `\S` can make
            // b10015 reject the grammar entirely. Runtime tool handlers still
            // enforce the canonical schema after generation.
            object.remove("minLength");
            object.remove("maxLength");
            object.remove("pattern");
            for child in object.values_mut() {
                normalize_llama_cpp_schema(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize_llama_cpp_schema(child);
            }
        }
        _ => {}
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
        GenerateInput::Text(text) => push_chat_message(messages, "user", text),
        GenerateInput::Messages(input_messages) => {
            for message in input_messages {
                push_chat_message(
                    messages,
                    match message.role {
                        GenerateMessageRole::System | GenerateMessageRole::Developer => "system",
                        GenerateMessageRole::User => "user",
                        GenerateMessageRole::Assistant => "assistant",
                    },
                    &message.content,
                );
            }
        }
        GenerateInput::Items(items) => {
            for item in items {
                match item {
                    GenerateInputItem::Message(message) => push_chat_message(
                        messages,
                        match message.role {
                            GenerateMessageRole::System | GenerateMessageRole::Developer => {
                                "system"
                            }
                            GenerateMessageRole::User => "user",
                            GenerateMessageRole::Assistant => "assistant",
                        },
                        &message.content,
                    ),
                    GenerateInputItem::Reasoning(_) | GenerateInputItem::ToolCall(_) => {
                        push_chat_message(messages, "assistant", item.render_for_token_count());
                    }
                    GenerateInputItem::ToolResult(_) => {
                        push_chat_message(messages, "user", item.render_for_token_count());
                    }
                }
            }
        }
        GenerateInput::NativeToolResults(_) => {
            push_chat_message(messages, "user", input.render_for_token_count());
        }
    }
}

fn push_chat_message(
    messages: &mut Vec<ChatMessage>,
    role: &'static str,
    content: impl Into<String>,
) {
    let content = content.into();
    if role == "system"
        && let Some(previous) = messages.last_mut()
        && previous.role == "system"
    {
        previous.content.push_str("\n\n");
        previous.content.push_str(&content);
        return;
    }
    messages.push(ChatMessage::new(role, content));
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
        assert!(!body.chat_template_kwargs.enable_thinking);
        assert!(body.response_format.is_none());
    }

    #[test]
    fn chat_request_coalesces_developer_context_into_the_leading_system_message() {
        let mut request = GenerateRequest::text("ignored");
        request.instructions = Some("system rules".to_string());
        request.input = GenerateInput::Messages(vec![
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "identity update".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "memory tool catalog".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::User,
                content: "question".to_string(),
            },
        ]);

        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");

        assert_eq!(body.messages.len(), 2);
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(
            body.messages[0].content,
            "system rules\n\nidentity update\n\nmemory tool catalog"
        );
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[1].content, "question");
    }

    #[test]
    fn required_noema_response_without_tools_forbids_tool_calls() {
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            ..GenerateRequest::text("hello")
        };

        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let response_format = body.response_format.expect("response format");

        assert_eq!(response_format["type"], "json_schema");
        assert_eq!(response_format["json_schema"]["name"], "noema_response");
        assert_eq!(
            response_format["json_schema"]["schema"]["required"],
            serde_json::json!(["response_status", "responses", "tool_calls"])
        );
        assert_eq!(
            response_format["json_schema"]["schema"]["properties"]["response_status"]["enum"],
            serde_json::json!(["final"])
        );
        assert_eq!(
            response_format["json_schema"]["schema"]["properties"]["tool_calls"]["maxItems"],
            0
        );
    }

    #[test]
    fn none_tool_choice_forbids_calls_even_when_specs_are_present() {
        let tool = crate::provider::NoemaToolSpec::new(
            "search_memory",
            "Search memory.",
            serde_json::json!({"type": "object", "additionalProperties": false}),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool");
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![tool],
            tool_choice: crate::provider::NoemaToolChoice::None,
            ..GenerateRequest::text("answer without tools")
        };

        let response_format =
            ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
                .expect("chat request")
                .response_format
                .expect("response format");
        let schema = &response_format["json_schema"]["schema"];

        assert_eq!(
            schema["properties"]["response_status"]["enum"],
            serde_json::json!(["final"])
        );
        assert_eq!(schema["properties"]["tool_calls"]["maxItems"], 0);
    }

    #[test]
    fn required_tool_choice_constrains_local_response_schema() {
        let tool = crate::provider::NoemaToolSpec::new(
            "task.submit_result",
            "Submit a result.",
            serde_json::json!({
                "type": "object",
                "properties": {"summary": {"type": "string"}},
                "required": ["summary"],
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool");
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![tool],
            tool_choice: crate::provider::NoemaToolChoice::Required,
            ..GenerateRequest::text("finish")
        };

        let response_format =
            ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
                .expect("chat request")
                .response_format
                .expect("response format");
        let schema = &response_format["json_schema"]["schema"];

        assert_eq!(
            schema["properties"]["response_status"]["enum"],
            serde_json::json!(["needs_tools"])
        );
        assert_eq!(schema["properties"]["responses"]["maxItems"], 0);
        assert_eq!(schema["properties"]["tool_calls"]["minItems"], 1);
        assert_eq!(schema["properties"]["tool_calls"]["maxItems"], 1);
        assert_eq!(
            schema["properties"]["tool_calls"]["items"]["oneOf"][0]["properties"]["name"]["enum"],
            serde_json::json!(["task.submit_result"])
        );
        assert_eq!(
            schema["properties"]["tool_calls"]["items"]["oneOf"][0]["properties"]["payload"]["required"],
            serde_json::json!(["summary"])
        );
    }

    #[test]
    fn required_tool_choice_rejects_an_empty_catalog() {
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tool_choice: crate::provider::NoemaToolChoice::Required,
            ..GenerateRequest::text("finish")
        };

        let error = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect_err("empty required catalog should fail");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn allowed_tool_choice_specializes_schema_to_the_selected_catalog_entry() {
        let first = crate::provider::NoemaToolSpec::new(
            "search_memory",
            "Search memory.",
            serde_json::json!({"type": "object", "additionalProperties": false}),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("first tool");
        let second = crate::provider::NoemaToolSpec::new(
            "task.inspect",
            "Inspect a task.",
            serde_json::json!({"type": "object", "additionalProperties": false}),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("second tool");
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![first.clone(), second],
            tool_choice: crate::provider::NoemaToolChoice::Allowed(
                crate::provider::NoemaAllowedTools {
                    mode: crate::provider::NoemaAllowedToolsMode::Auto,
                    tools: vec![first.name],
                },
            ),
            ..GenerateRequest::text("search")
        };

        let response_format =
            ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
                .expect("chat request")
                .response_format
                .expect("response format");
        let tool_variants =
            &response_format["json_schema"]["schema"]["properties"]["tool_calls"]["items"]["oneOf"];

        assert_eq!(tool_variants.as_array().map(Vec::len), Some(1));
        assert_eq!(
            tool_variants[0]["properties"]["name"]["enum"],
            serde_json::json!(["search_memory"])
        );
    }

    #[test]
    fn local_tool_schema_drops_patterns_unsupported_by_llama_cpp() {
        let tool = crate::provider::NoemaToolSpec::new(
            "artifact.create_local_file",
            "Create a file.",
            serde_json::json!({
                "type": "object",
                "properties": {"title": {"type": "string", "pattern": ".*\\S.*"}},
                "required": ["title"],
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool");
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![tool],
            ..GenerateRequest::text("create")
        };

        let response_format =
            ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
                .expect("chat request")
                .response_format
                .expect("response format");
        let title = &response_format["json_schema"]["schema"]["properties"]["tool_calls"]["items"]
            ["oneOf"][0]["properties"]["payload"]["properties"]["title"];

        assert_eq!(title["type"], "string");
        assert!(title.get("pattern").is_none());
        assert_eq!(
            request.tools[0].input_schema.as_value()["properties"]["title"]["pattern"],
            ".*\\S.*"
        );
    }

    #[test]
    fn local_tool_schema_drops_expansive_string_length_grammar() {
        let tool = crate::provider::NoemaToolSpec::new(
            "task.submit_result",
            "Submit a result.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "minLength": 1, "maxLength": 4000}
                },
                "required": ["summary"],
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool");
        let request = GenerateRequest {
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![tool],
            ..GenerateRequest::text("finish")
        };

        let response_format =
            ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
                .expect("chat request")
                .response_format
                .expect("response format");
        let summary = &response_format["json_schema"]["schema"]["properties"]["tool_calls"]["items"]
            ["oneOf"][0]["properties"]["payload"]["properties"]["summary"];

        assert_eq!(summary["type"], "string");
        assert!(summary.get("minLength").is_none());
        assert!(summary.get("maxLength").is_none());
    }

    #[test]
    fn local_tool_call_is_canonicalized_to_needs_tools() {
        let parsed = required_local_noema_response_from_text(
            r#"{
                "response_status":"final",
                "responses":[{"kind":"text","phase":"final_answer","text":"untrusted answer"}],
                "tool_calls":[{"name":"search_memory","payload":{"scope_ids":["human:local"],"query":"comets","purpose":"answer_human_question","limit":8}}]
            }"#,
        )
        .expect("canonical local tool response");

        assert_eq!(parsed.response_status, GenerateResponseStatus::NeedsTools);
        assert!(parsed.responses.is_empty());
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].name, "search_memory");
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
