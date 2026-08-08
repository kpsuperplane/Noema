//! First-party local GGUF provider backed by a supervised llama.cpp server.

use std::{fmt, time::Duration};

use futures_util::StreamExt;
use serde_json::json;

use super::{
    LlamaServerConfig, LlamaServerError, LlamaServerSupervisor, bundled_llama_server_candidates_in,
};
use crate::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseItem,
    GenerateStreamEvent, GenerateToolCall, LocalModelsProviderConfig, ModelProvider,
    NoemaToolChoice, ProviderContextMetadata, ProviderError, ProviderSchemaCapabilities,
    ProviderTool, ProviderToolCapabilities, ProviderToolSchemaDialect, ProviderToolTransport,
    reqwest_transport_error,
};
use noema_capabilities::ToolSpec;

use request::ChatCompletionRequest;
use streaming::{ChatSseAccumulator, ChatStreamEvent, ChatStreamOutput, TokenizeResponse};

mod request;
mod streaming;

/// Stable provider identifier for first-party local GGUF inference.
pub const LOCAL_MODELS_PROVIDER: &str = "local_models";
/// Default output reserve used by prompt planning for local models.
const LOCAL_MODELS_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 1_024;
/// Default compact-summary target for local models.
const LOCAL_MODELS_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 768;
/// Stable model-visible function used to qualify native tool calling.
const LOCAL_MODELS_QUALIFICATION_TOOL: &str = "noema_local_qualification";

/// Local model provider facade that preserves Noema's provider-neutral contracts.
#[derive(Clone)]
pub struct LocalModelsProvider {
    config: LocalModelsProviderConfig,
    supervisor: LlamaServerSupervisor,
    client: reqwest::Client,
}

impl fmt::Debug for LocalModelsProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelsProvider")
            .field("config", &self.config)
            .field("supervisor", &"[CONFIGURED]")
            .field("client", &"[CONFIGURED]")
            .finish()
    }
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
        let mut candidates = bundled_llama_server_candidates_in(
            config.preferred_backend,
            config.runtime_root.as_deref(),
        );
        for candidate in &mut candidates {
            if !candidate.extra_args.iter().any(|arg| arg == "--jinja") {
                candidate.extra_args.push("--jinja".to_string());
            }
        }
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
            .map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "build_client", &source)
            })?;
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

    /// Exercises one native function call before the process is registered.
    ///
    /// The probe intentionally uses a model-visible tool rather than a health
    /// endpoint, because a healthy llama-server can still be running without a
    /// Jinja template that emits parseable native tool calls.
    pub(super) async fn qualify_native_tools(&self) -> Result<(), ProviderError> {
        let request = native_tool_qualification_request(&self.config.default_model)?;
        let response = self.generate(request).await?;
        validate_native_tool_qualification(&response)
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
            .acquire_generation(request.options.generation_priority)
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
            .map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "send_generation", &source)
            })?;
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
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "read_generation_stream", &source)
            })?;
            accumulator.push_bytes(&chunk, |event| forward_stream_event(event, on_event))?;
        }
        accumulator.finish(|event| forward_stream_event(event, on_event))
    }
}

fn forward_stream_event(
    event: ChatStreamEvent,
    on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
) {
    match event {
        ChatStreamEvent::AssistantTextDelta(delta) => {
            on_event(GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta,
            });
        }
        ChatStreamEvent::ToolCallStarted { output_index, name } => {
            on_event(GenerateStreamEvent::ToolCallStarted { output_index, name })
        }
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

fn native_tool_qualification_request(model: &str) -> Result<GenerateRequest, ProviderError> {
    let tool = ToolSpec::new(
        LOCAL_MODELS_QUALIFICATION_TOOL,
        "Qualification-only function. Call it once with an empty object.",
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false,
        }),
    )
    .map_err(|error| ProviderError::InvalidRequest {
        message: format!("invalid local native-tool qualification schema: {error}"),
    })?;

    Ok(GenerateRequest {
        model: Some(model.to_string()),
        input: GenerateInput::Text(
            "Call the noema_local_qualification function exactly once with an empty JSON object. Do not answer in prose."
                .to_string(),
        ),
        instructions: Some(
            "This is a local runtime qualification request. Emit the requested native function call and stop."
                .to_string(),
        ),
        options: GenerateOptions {
            max_output_tokens: Some(64),
            ..GenerateOptions::default()
        },
        tools: vec![ProviderTool::canonical(tool)],
        tool_transport: ProviderToolTransport::Native,
        tool_choice: NoemaToolChoice::Required,
        parallel_tool_calls: false,
        conversation_id: None,
    })
}

fn validate_native_tool_qualification(response: &GenerateResponse) -> Result<(), ProviderError> {
    if response.tool_calls.len() != 1 {
        return Err(ProviderError::MalformedResponse {
            message: format!(
                "local native-tool qualification expected exactly one tool call, got {}",
                response.tool_calls.len()
            ),
        });
    }
    let call = &response.tool_calls[0];
    if call.name != LOCAL_MODELS_QUALIFICATION_TOOL {
        return Err(ProviderError::MalformedResponse {
            message: format!(
                "local native-tool qualification returned `{}` instead of `{LOCAL_MODELS_QUALIFICATION_TOOL}`",
                call.name
            ),
        });
    }
    if !call
        .payload
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
    {
        return Err(ProviderError::MalformedResponse {
            message: "local native-tool qualification arguments were not the required empty object"
                .to_string(),
        });
    }
    Ok(())
}

impl ModelProvider for LocalModelsProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let mut ignore_event = |_| {};
        self.generate_streaming(request, &mut ignore_event).await
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        Some(self.config.default_model.clone())
    }

    async fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(self.config.context_window_tokens),
            default_output_reserve_tokens: Some(LOCAL_MODELS_DEFAULT_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(LOCAL_MODELS_COMPACT_SUMMARY_TARGET_TOKENS),
        }
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: true,
            native_tool_results: true,
            ..ProviderToolCapabilities::default()
        }
    }

    fn schema_capabilities(&self, _model: Option<&str>) -> ProviderSchemaCapabilities {
        ProviderSchemaCapabilities::strict()
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
            .map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "send_tokenize", &source)
            })?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let body = response
            .json::<TokenizeResponse>()
            .await
            .map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "read_tokenize_response", &source)
            })?;
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
        let stream = self.send_chat_stream(&request, &model, on_event).await?;
        let tool_calls = canonicalize_tool_calls(stream.tool_calls, &request)?;
        let responses = if stream.text.trim().is_empty() {
            Vec::new()
        } else {
            vec![GenerateResponseItem::Text {
                phase: None,
                text: stream.text,
            }]
        };
        Ok(GenerateResponse {
            responses,
            tool_calls,
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
            citations: Vec::new(),
            provider: LOCAL_MODELS_PROVIDER.to_string(),
            model: stream.model.unwrap_or(model),
            response_id: stream.response_id,
            usage: stream.usage,
        })
    }
}

fn canonicalize_tool_calls(
    calls: Vec<GenerateToolCall>,
    request: &GenerateRequest,
) -> Result<Vec<GenerateToolCall>, ProviderError> {
    let selected_tools = request_tools_for_response(request)?;
    calls
        .into_iter()
        .map(|mut call| {
            let Some(tool) = selected_tools
                .iter()
                .find(|tool| tool.exposed_name() == call.name)
            else {
                return Err(ProviderError::MalformedResponse {
                    message: format!("local model returned an unadvertised tool `{}`", call.name),
                });
            };
            call.provider_name = Some(call.name.clone());
            call.name = tool.canonical_spec().name.as_str().to_string();
            Ok(call)
        })
        .collect()
}

fn request_tools_for_response(
    request: &GenerateRequest,
) -> Result<Vec<&crate::ProviderTool>, ProviderError> {
    match &request.tool_choice {
        crate::NoemaToolChoice::None => Ok(Vec::new()),
        crate::NoemaToolChoice::Auto | crate::NoemaToolChoice::Required => {
            if matches!(&request.tool_choice, crate::NoemaToolChoice::Required)
                && request.tools.is_empty()
            {
                return Err(ProviderError::InvalidRequest {
                    message: "required tool choice needs a non-empty tool catalog".to_string(),
                });
            }
            Ok(request.tools.iter().collect())
        }
        crate::NoemaToolChoice::Allowed(allowed) => {
            let mut selected = Vec::with_capacity(allowed.tools.len());
            for allowed_name in &allowed.tools {
                let Some(tool) = request
                    .tools
                    .iter()
                    .find(|tool| &tool.canonical_spec().name == allowed_name)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate as noema_providers;
    use crate::{
        GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole,
        GenerateToolCallInput, GenerateToolResultInput,
    };
    use serde_json::Value;

    #[test]
    fn provider_debug_preserves_local_model_paths() {
        let provider = LocalModelsProvider::new(LocalModelsProviderConfig {
            default_model: "local-8b".to_string(),
            model_path: Some(std::path::PathBuf::from("/private/model-secret.gguf")),
            preferred_backend: Some(noema_providers::LocalModelBackend::Cpu),
            runtime_root: Some(std::path::PathBuf::from("/private/runtime-secret")),
            context_window_tokens: 8_192,
            timeout_seconds: 600,
            startup_timeout_seconds: 180,
            system_errors: None,
        })
        .expect("provider");

        let debug = format!("{provider:?}");
        assert!(debug.contains("/private/model-secret.gguf"));
        assert!(debug.contains("/private/runtime-secret"));
    }

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
            GenerateInputItem::ToolCall(GenerateToolCallInput {
                id: Some("call-item-1".to_string()),
                call_id: "call-1".to_string(),
                name: "read_file".to_string(),
                provider_name: None,
                arguments: serde_json::json!({"path": "notes.txt"}),
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

        assert_eq!(body.messages.len(), 5);
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[2].role, "assistant");
        assert_eq!(body.messages[3].role, "assistant");
        assert_eq!(body.messages[3].content, None);
        assert_eq!(body.messages[3].tool_calls.as_ref().map(Vec::len), Some(1));
        let wire = serde_json::to_value(&body).expect("wire request");
        assert_eq!(wire["messages"][3]["tool_calls"][0]["id"], "call-1");
        assert_eq!(body.messages[4].role, "tool");
        assert_eq!(body.messages[4].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(body.max_tokens, Some(321));
        assert_eq!(body.temperature, Some(0.2));
        assert!(body.cache_prompt);
        assert!(!body.chat_template_kwargs.enable_thinking);
        assert!(body.tools.is_none());
        assert!(body.tool_choice.is_none());
        assert!(body.parallel_tool_calls.is_none());
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
            body.messages[0].content.as_deref(),
            Some("system rules\n\nidentity update\n\nmemory tool catalog")
        );
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[1].content.as_deref(), Some("question"));
    }

    #[test]
    fn native_request_uses_openai_tool_fields_without_response_format() {
        let request = tool_request(
            vec![test_tool("search_memory", serde_json::json!({}))],
            noema_providers::NoemaToolChoice::Auto,
        );
        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let wire = serde_json::to_value(&body).expect("wire request");

        assert_eq!(wire["tools"][0]["type"], "function");
        assert_eq!(wire["tools"][0]["function"]["name"], "search_memory");
        assert_eq!(wire["tool_choice"], "auto");
        assert_eq!(wire["parallel_tool_calls"], false);
        assert!(wire.get("response_format").is_none());
    }

    #[test]
    fn native_tool_qualification_requests_one_required_empty_object_function() {
        let request = native_tool_qualification_request("local-8b").expect("qualification request");
        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let wire = serde_json::to_value(&body).expect("wire request");

        assert_eq!(request.model.as_deref(), Some("local-8b"));
        assert_eq!(request.options.max_output_tokens, Some(64));
        assert_eq!(
            request.tool_choice,
            noema_providers::NoemaToolChoice::Required
        );
        assert!(!request.parallel_tool_calls);
        assert_eq!(wire["tools"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            wire["tools"][0]["function"]["name"],
            LOCAL_MODELS_QUALIFICATION_TOOL
        );
        assert_eq!(wire["tools"][0]["function"]["parameters"]["type"], "object");
        assert_eq!(wire["tool_choice"], "required");
        assert_eq!(wire["parallel_tool_calls"], false);
    }

    #[test]
    fn native_tool_qualification_requires_the_expected_object_call() {
        let response = GenerateResponse {
            responses: Vec::new(),
            tool_calls: vec![GenerateToolCall {
                id: Some("call-1".to_string()),
                provider_call_id: Some("call-1".to_string()),
                provider_name: Some(LOCAL_MODELS_QUALIFICATION_TOOL.to_string()),
                name: LOCAL_MODELS_QUALIFICATION_TOOL.to_string(),
                payload: serde_json::json!({}),
            }],
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
            citations: Vec::new(),
            provider: LOCAL_MODELS_PROVIDER.to_string(),
            model: "local-8b".to_string(),
            response_id: None,
            usage: None,
        };
        validate_native_tool_qualification(&response).expect("valid qualification response");

        let mut invalid = response;
        invalid.tool_calls[0].name = "other_tool".to_string();
        assert!(matches!(
            validate_native_tool_qualification(&invalid),
            Err(ProviderError::MalformedResponse { .. })
        ));

        invalid.tool_calls[0].name = LOCAL_MODELS_QUALIFICATION_TOOL.to_string();
        invalid.tool_calls[0].payload = serde_json::json!("not an object");
        assert!(matches!(
            validate_native_tool_qualification(&invalid),
            Err(ProviderError::MalformedResponse { .. })
        ));

        invalid.tool_calls[0].payload = serde_json::json!({"unexpected": true});
        assert!(matches!(
            validate_native_tool_qualification(&invalid),
            Err(ProviderError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn required_tool_choice_is_sent_as_native_policy() {
        let request = tool_request(
            vec![test_tool(
                "task.submit_result",
                serde_json::json!({
                    "properties": {"summary": {"type": "string"}},
                    "required": ["summary"]
                }),
            )],
            noema_providers::NoemaToolChoice::Required,
        );
        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let wire = serde_json::to_value(&body).expect("wire request");

        assert_eq!(wire["tool_choice"], "required");
        assert_eq!(
            wire["tools"][0]["function"]["parameters"]["required"],
            serde_json::json!(["summary"])
        );
    }

    #[test]
    fn required_tool_choice_rejects_an_empty_catalog() {
        let request = GenerateRequest {
            tool_choice: noema_providers::NoemaToolChoice::Required,
            ..GenerateRequest::text("finish")
        };

        let error = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect_err("empty required catalog should fail");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn allowed_tool_choice_filters_native_catalog_to_the_selected_tool() {
        let first = test_tool("search_memory", serde_json::json!({}));
        let request = tool_request(
            vec![
                first.clone(),
                test_tool("task.inspect", serde_json::json!({})),
            ],
            noema_providers::NoemaToolChoice::Allowed(noema_providers::NoemaAllowedTools {
                mode: noema_providers::NoemaAllowedToolsMode::Auto,
                tools: vec![first.name],
            }),
        );
        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let wire = serde_json::to_value(&body).expect("wire request");

        assert_eq!(wire["tools"].as_array().map(Vec::len), Some(1));
        assert_eq!(wire["tools"][0]["function"]["name"], "search_memory");
    }

    #[test]
    fn local_tool_schema_drops_unsupported_string_grammar() {
        let tool = test_tool(
            "artifact.create_local_file",
            serde_json::json!({
                "properties": {
                    "title": {
                        "type": "string",
                        "pattern": ".*\\S.*",
                        "minLength": 1,
                        "maxLength": 4000
                    }
                },
                "required": ["title"]
            }),
        );
        let source_schema = tool.input_schema.clone();
        let request = tool_request(vec![tool], noema_providers::NoemaToolChoice::Auto);
        let body = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request");
        let wire = serde_json::to_value(&body).expect("wire request");
        let title = &wire["tools"][0]["function"]["parameters"]["properties"]["title"];

        assert_eq!(title["type"], "string");
        assert!(title.get("pattern").is_none());
        assert!(title.get("minLength").is_none());
        assert!(title.get("maxLength").is_none());
        assert_eq!(
            source_schema.as_value()["properties"]["title"]["pattern"],
            ".*\\S.*"
        );
    }

    fn test_tool(name: &str, schema: Value) -> noema_capabilities::ToolSpec {
        let mut schema = schema;
        schema["type"] = serde_json::json!("object");
        schema["additionalProperties"] = serde_json::json!(false);
        noema_capabilities::ToolSpec::new(name, "Test tool.", schema).expect("tool")
    }

    fn tool_request(
        tools: Vec<noema_capabilities::ToolSpec>,
        tool_choice: noema_providers::NoemaToolChoice,
    ) -> GenerateRequest {
        GenerateRequest {
            tools: tools.into_iter().map(Into::into).collect(),
            tool_choice,
            ..GenerateRequest::text("test")
        }
    }
}
