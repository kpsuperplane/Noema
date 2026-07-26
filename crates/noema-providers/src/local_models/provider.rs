//! First-party local GGUF provider backed by a supervised llama.cpp server.

use std::{fmt, time::Duration};

use futures_util::StreamExt;
use serde_json::Value;

use super::{
    LlamaServerConfig, LlamaServerError, LlamaServerSupervisor, bundled_llama_server_candidates_in,
};
use crate::{
    GenerateRequest, GenerateResponse, GenerateResponseStatus, GenerateStreamEvent,
    LocalModelsProviderConfig, ModelProvider, ParsedNoemaResponse, ProviderContextMetadata,
    ProviderError, ProviderSchemaCapabilities, ProviderToolCapabilities, ProviderToolTransport,
    SchemaEnforcement, output_items_from_text, required_noema_response_from_text,
    reqwest_transport_error,
    response_support::{NoemaAssistantTextDeltaExtractor, StructuredResponseDiagnosticContext},
};

use request::ChatCompletionRequest;
use streaming::{ChatSseAccumulator, ChatStreamOutput, TokenizeResponse};

mod request;
mod streaming;

/// Stable provider identifier for first-party local GGUF inference.
pub const LOCAL_MODELS_PROVIDER: &str = "local_models";
/// Default output reserve used by prompt planning for local models.
const LOCAL_MODELS_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 1_024;
/// Default compact-summary target for local models.
const LOCAL_MODELS_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 768;

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
        diagnostics: &StructuredResponseDiagnosticContext,
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
        if body
            .response_format
            .as_ref()
            .is_some_and(|format| format["json_schema"]["strict"] == Value::Bool(false))
        {
            diagnostics.log_schema_fallback(
                "local_response",
                "the selected tool catalog is outside the llama.cpp strict subset",
            );
        }
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
        let mut structured_extractor = NoemaAssistantTextDeltaExtractor::default();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| {
                reqwest_transport_error(LOCAL_MODELS_PROVIDER, "read_generation_stream", &source)
            })?;
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
            strict_schema: true,
            ..ProviderToolCapabilities::default()
        }
    }

    fn schema_capabilities(&self, _model: Option<&str>) -> ProviderSchemaCapabilities {
        ProviderSchemaCapabilities {
            native_tool_arguments: SchemaEnforcement::Unsupported,
            structured_output: SchemaEnforcement::Strict,
            structured_output_with_tools: SchemaEnforcement::Strict,
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
        let diagnostics = StructuredResponseDiagnosticContext::new(
            self.config.system_errors.clone(),
            LOCAL_MODELS_PROVIDER,
            model.clone(),
            request.conversation_id.clone(),
        );
        let stream = self
            .send_chat_stream(&request, &model, &diagnostics, on_event)
            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate as noema_providers;
    use crate::{
        GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole,
        GenerateToolResultInput,
    };

    #[test]
    fn provider_debug_redacts_local_model_paths() {
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
        assert!(!debug.contains("model-secret.gguf"));
        assert!(!debug.contains("runtime-secret"));
        assert!(debug.contains("[REDACTED PATH]"));
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
    fn none_tool_choice_forbids_calls_even_when_specs_are_present() {
        let schema = tool_response_schema(
            vec![test_tool("search_memory", serde_json::json!({}))],
            noema_providers::NoemaToolChoice::None,
        );

        assert_eq!(
            schema["properties"]["response_status"]["enum"],
            serde_json::json!(["final"])
        );
        assert_eq!(schema["properties"]["tool_calls"]["maxItems"], 0);
    }

    #[test]
    fn required_tool_choice_constrains_local_response_schema() {
        let schema = tool_response_schema(
            vec![test_tool(
                "task.submit_result",
                serde_json::json!({
                    "properties": {"summary": {"type": "string"}},
                    "required": ["summary"]
                }),
            )],
            noema_providers::NoemaToolChoice::Required,
        );

        assert_eq!(
            schema["properties"]["response_status"]["enum"],
            serde_json::json!(["needs_tools"])
        );
        assert_eq!(schema["properties"]["responses"]["maxItems"], 0);
        assert_eq!(schema["properties"]["tool_calls"]["minItems"], 1);
        assert_eq!(schema["properties"]["tool_calls"]["maxItems"], 1);
        assert_eq!(
            schema["properties"]["tool_calls"]["items"]["anyOf"][0]["properties"]["name"]["enum"],
            serde_json::json!(["task.submit_result"])
        );
        assert_eq!(
            schema["properties"]["tool_calls"]["items"]["anyOf"][0]["properties"]["payload"]["required"],
            serde_json::json!(["summary"])
        );
    }

    #[test]
    fn required_tool_choice_rejects_an_empty_catalog() {
        let request = GenerateRequest {
            options: noema_providers::GenerateOptions {
                require_noema_response: true,
                ..noema_providers::GenerateOptions::default()
            },
            tool_choice: noema_providers::NoemaToolChoice::Required,
            ..GenerateRequest::text("finish")
        };

        let error = ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect_err("empty required catalog should fail");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn allowed_tool_choice_specializes_schema_to_the_selected_catalog_entry() {
        let first = test_tool("search_memory", serde_json::json!({}));
        let schema = tool_response_schema(
            vec![
                first.clone(),
                test_tool("task.inspect", serde_json::json!({})),
            ],
            noema_providers::NoemaToolChoice::Allowed(noema_providers::NoemaAllowedTools {
                mode: noema_providers::NoemaAllowedToolsMode::Auto,
                tools: vec![first.name],
            }),
        );
        let tool_variants = &schema["properties"]["tool_calls"]["items"]["anyOf"];

        assert_eq!(tool_variants.as_array().map(Vec::len), Some(1));
        assert_eq!(
            tool_variants[0]["properties"]["name"]["enum"],
            serde_json::json!(["search_memory"])
        );
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
        let schema = tool_response_schema(vec![tool], noema_providers::NoemaToolChoice::Auto);
        let title = &schema["properties"]["tool_calls"]["items"]["anyOf"][0]["properties"]["payload"]
            ["properties"]["title"];

        assert_eq!(title["type"], "string");
        assert!(title.get("pattern").is_none());
        assert!(title.get("minLength").is_none());
        assert!(title.get("maxLength").is_none());
        assert_eq!(
            source_schema.as_value()["properties"]["title"]["pattern"],
            ".*\\S.*"
        );
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

    fn test_tool(name: &str, schema: Value) -> noema_capabilities::ToolSpec {
        let mut schema = schema;
        schema["type"] = serde_json::json!("object");
        schema["additionalProperties"] = serde_json::json!(false);
        noema_capabilities::ToolSpec::new(name, "Test tool.", schema).expect("tool")
    }

    fn tool_response_schema(
        tools: Vec<noema_capabilities::ToolSpec>,
        tool_choice: noema_providers::NoemaToolChoice,
    ) -> Value {
        let request = GenerateRequest {
            options: noema_providers::GenerateOptions {
                require_noema_response: true,
                ..noema_providers::GenerateOptions::default()
            },
            tools: tools.into_iter().map(Into::into).collect(),
            tool_choice,
            ..GenerateRequest::text("test")
        };
        ChatCompletionRequest::from_generate(&request, "local-8b".to_string())
            .expect("chat request")
            .response_format
            .expect("response format")["json_schema"]["schema"]
            .clone()
    }
}
