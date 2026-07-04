//! Provider adapter for the OpenAI Responses API.

use super::responses::{
    ResponsesDiagnosticContext, ResponsesInput, ResponsesRequest, ResponsesToolNameMap,
    ResponsesTransport, header_value, noema_response_text_format, normalize_base_url,
    responses_tool_choice,
};
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse,
        GenerateResponseStatus, ModelProvider, ParsedNoemaResponse, ProviderError,
        ProviderToolCapabilities, ProviderToolFallbackMode, ProviderToolSchemaDialect,
        output_items_from_text, required_noema_response_from_text_with_native_tool_calls,
    },
};
use reqwest::header::{HeaderMap, HeaderName};
use serde_json::Value;
use std::time::Duration;

/// Default request timeout for `OpenAI` calls.
pub const DEFAULT_OPENAI_TIMEOUT_SECONDS: u64 = 120;

/// Configuration for the `OpenAI` provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiProviderConfig {
    /// API key sent as a bearer token.
    pub api_key: String,
    /// Base URL for an OpenAI-compatible Responses API.
    pub base_url: String,
    /// Optional `OpenAI` organization id.
    pub organization_id: Option<String>,
    /// Optional `OpenAI` project id.
    pub project_id: Option<String>,
    /// Default model used when a request does not override it.
    pub default_model: String,
    /// Optional model override for metadata-only tool classification.
    pub tool_classification_model: Option<String>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Developer diagnostic system error logger.
    pub system_errors: Option<SystemErrorLogger>,
}

/// Provider implementation backed by the `OpenAI` Responses API.
#[derive(Debug)]
pub struct OpenAiProvider {
    transport: ResponsesTransport,
    config: OpenAiProviderConfig,
    system_errors: Option<SystemErrorLogger>,
}

impl OpenAiProvider {
    /// Build an `OpenAI` provider with a default reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid or the HTTP
    /// client cannot be built.
    pub fn new(config: OpenAiProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;

        Self::with_client(client, config)
    }

    /// Build an `OpenAI` provider with a caller-supplied reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] or
    /// [`ProviderError::MissingCredentials`] when configuration validation
    /// fails.
    pub fn with_client(
        client: reqwest::Client,
        config: OpenAiProviderConfig,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let transport = ResponsesTransport::new(client, config.base_url.clone())?;
        Ok(Self {
            transport,
            system_errors: config.system_errors.clone(),
            config,
        })
    }

    fn extra_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();

        if let Some(organization_id) = self
            .config
            .organization_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            headers.insert(
                HeaderName::from_static("openai-organization"),
                header_value(organization_id, "openai organization id")?,
            );
        }

        if let Some(project_id) = self
            .config
            .project_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            headers.insert(
                HeaderName::from_static("openai-project"),
                header_value(project_id, "openai project id")?,
            );
        }

        Ok(headers)
    }
}

fn normalize_config(
    mut config: OpenAiProviderConfig,
) -> Result<OpenAiProviderConfig, ProviderError> {
    if config.api_key.trim().is_empty() {
        return Err(ProviderError::MissingCredentials {
            provider: "openai".to_string(),
            credential: "api_key".to_string(),
        });
    }

    config.base_url = normalize_base_url(config.base_url, "openai base URL")?;

    let default_model = config.default_model.trim().to_string();
    if default_model.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "openai default model cannot be empty".to_string(),
        });
    }

    if config.timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "openai timeout must be greater than zero seconds".to_string(),
        });
    }

    config.default_model = default_model;
    config.tool_classification_model = config.tool_classification_model.and_then(|model| {
        let model = model.trim().to_string();
        (!model.is_empty()).then_some(model)
    });
    Ok(config)
}

impl ModelProvider for OpenAiProvider {
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(
            self.config
                .tool_classification_model
                .clone()
                .unwrap_or_else(|| DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string()),
        )
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            native_tools: true,
            parallel_tool_calls: true,
            tool_choice: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: true,
            fallback_mode: ProviderToolFallbackMode::NativeRequired,
        }
    }

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_model.clone());

        if model.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "model cannot be empty".to_string(),
            });
        }

        let tool_names = ResponsesToolNameMap::from_tools(&request.tools)?;
        let has_tools = !tool_names.tools.is_empty();
        let body = ResponsesRequest {
            model: model.clone(),
            input: ResponsesInput::from(&request.input),
            instructions: request
                .instructions
                .filter(|instructions| !instructions.trim().is_empty()),
            max_output_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
            text: request
                .options
                .require_noema_response
                .then(noema_response_text_format),
            tools: tool_names.tools.clone(),
            tool_choice: responses_tool_choice(request.tool_choice, has_tools),
            parallel_tool_calls: has_tools.then_some(request.parallel_tool_calls),
            store: false,
            prompt_cache_retention: request.options.prompt_cache_retention,
        };

        let diagnostics = ResponsesDiagnosticContext::new(
            self.system_errors.clone(),
            "openai",
            model.clone(),
            request.conversation_id.clone(),
        );
        let response = self
            .transport
            .send(
                &self.config.api_key,
                body,
                self.extra_headers()?,
                diagnostics,
            )
            .await?;
        let native_tool_calls = response.native_tool_calls_with_names(&tool_names)?;
        let text = match response.output_text() {
            Ok(text) => text,
            Err(error @ ProviderError::MalformedResponse { .. }) => {
                if !native_tool_calls.is_empty() {
                    let parsed = ParsedNoemaResponse {
                        responses: Vec::new(),
                        tool_calls: native_tool_calls,
                        memory_proposals: Vec::new(),
                        response_status: GenerateResponseStatus::NeedsTools,
                    };
                    return Ok(GenerateResponse::from_parsed(
                        parsed,
                        "openai",
                        response.model.unwrap_or(model),
                        response.id,
                        response.usage.map(Into::into),
                    ));
                }
                self.log_malformed_response_raw(
                    &error,
                    &model,
                    request.conversation_id.as_deref(),
                    response.id.as_deref(),
                    response.raw_payload(),
                );
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        let raw_text = text.clone();

        let parsed = if request.options.require_noema_response {
            match required_noema_response_from_text_with_native_tool_calls(
                text,
                native_tool_calls.clone(),
            ) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.log_malformed_response(
                        &error,
                        &model,
                        request.conversation_id.as_deref(),
                        response.id.as_deref(),
                        raw_text,
                    );
                    return Err(error);
                }
            }
        } else {
            ParsedNoemaResponse {
                responses: output_items_from_text(text)?,
                tool_calls: native_tool_calls.clone(),
                memory_proposals: Vec::new(),
                response_status: if native_tool_calls.is_empty() {
                    GenerateResponseStatus::Final
                } else {
                    GenerateResponseStatus::NeedsTools
                },
            }
        };

        Ok(GenerateResponse::from_parsed(
            parsed,
            "openai",
            response.model.unwrap_or(model),
            response.id,
            response.usage.map(Into::into),
        ))
    }
}

impl OpenAiProvider {
    fn log_malformed_response(
        &self,
        error: &ProviderError,
        model: &str,
        conversation_id: Option<&str>,
        request_id: Option<&str>,
        provider_text: String,
    ) {
        self.log_malformed_response_raw(
            error,
            model,
            conversation_id,
            request_id,
            serde_json::json!({
                "provider_text": provider_text,
            }),
        );
    }

    fn log_malformed_response_raw(
        &self,
        error: &ProviderError,
        model: &str,
        conversation_id: Option<&str>,
        request_id: Option<&str>,
        raw: Value,
    ) {
        let Some(logger) = &self.system_errors else {
            return;
        };
        logger.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, error.to_string())
                .with_context(serde_json::json!({
                    "provider_kind": "openai",
                    "model": model,
                    "conversation_id": conversation_id,
                    "request_id": request_id,
                }))
                .with_error_chain([error.to_string()])
                .with_raw(raw),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::adapters::test_support::spawn_server;
    use crate::provider::{
        NoemaToolChoice, NoemaToolExecution, NoemaToolSpec, ProviderToolFallbackMode,
        ProviderToolSchemaDialect, TokenUsage,
    };
    use crate::{GenerateInput, PromptCacheRetention};
    use serde_json::Value;

    #[tokio::test]
    async fn sends_expected_request_and_extracts_text() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [{
                "type": "message",
                "content": [
                  {"type": "output_text", "text": "Hello"},
                  {"type": "output_text", "text": ", world"}
                ]
              }],
              "usage": {
                "input_tokens": 2,
                "output_tokens": 3,
                "total_tokens": 5,
                "input_tokens_details": {
                  "cached_tokens": 1
                }
              }
            }"#,
        )
        .await;

        let provider = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url,
            organization_id: Some("org_test".to_string()),
            project_id: Some("proj_test".to_string()),
            default_model: "default-model".to_string(),
            tool_classification_model: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .expect("provider");

        let response = provider
            .generate(GenerateRequest {
                conversation_id: None,
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Hello?".to_string()),
                instructions: Some("Be brief.".to_string()),
                options: crate::provider::GenerateOptions {
                    max_output_tokens: Some(32),
                    temperature: Some(0.4),
                    prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                    ..crate::provider::GenerateOptions::default()
                },
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/responses");
        assert_eq!(
            captured.headers.get("authorization").map(String::as_str),
            Some("Bearer secret")
        );
        assert_eq!(
            captured
                .headers
                .get("openai-organization")
                .map(String::as_str),
            Some("org_test")
        );
        assert_eq!(
            captured.headers.get("openai-project").map(String::as_str),
            Some("proj_test")
        );

        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"], "Hello?");
        assert_eq!(body["instructions"], "Be brief.");
        assert_eq!(body["store"], false);
        assert_eq!(body["max_output_tokens"], 32);
        assert_eq!(body["temperature"], 0.4);
        assert_eq!(body["prompt_cache_retention"], "24h");
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("parallel_tool_calls").is_none());

        assert_eq!(response.assistant_text(), "Hello, world");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-test");
        assert_eq!(response.response_id.as_deref(), Some("resp_test"));
        assert_eq!(
            response.usage,
            Some(TokenUsage {
                input_tokens: 2,
                output_tokens: 3,
                total_tokens: 5,
                cached_input_tokens: Some(1),
            })
        );
    }

    #[tokio::test]
    async fn required_noema_response_requests_json_schema_text_format() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [{
                "type": "message",
                "content": [
                  {"type": "output_text", "text": "{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"Hello\"}],\"tool_calls\":[],\"memory_proposals\":[]}"}
                ]
              }]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let response = provider
            .generate(GenerateRequest {
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                ..GenerateRequest::text("Hello?")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["text"]["format"]["name"], "noema_response");
        assert_eq!(
            body["text"]["format"]["schema"]["properties"]["response_status"]["enum"][1],
            "final"
        );
        assert_eq!(response.assistant_text(), "Hello");
    }

    #[tokio::test]
    async fn request_sends_native_tool_specs_with_provider_safe_names_and_maps_calls_back() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "message",
                  "content": [
                    {"type": "output_text", "text": "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Reading docs.\"}],\"tool_calls\":[],\"memory_proposals\":[]}"}
                  ]
                },
                {
                  "type": "function_call",
                  "id": "item_1",
                  "call_id": "call_1",
                  "name": "mcp_x2e_docs_x3a_read",
                  "arguments": "{\"document_id\":\"doc_1\"}"
                }
              ]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let response = provider
            .generate(GenerateRequest {
                conversation_id: None,
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Read it".to_string()),
                instructions: None,
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                tools: vec![mcp_docs_read_tool()],
                tool_choice: NoemaToolChoice::Required,
                parallel_tool_calls: true,
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["tools"][0]["name"], "mcp_x2e_docs_x3a_read");
        assert_eq!(body["tool_choice"], "required");
        assert_eq!(body["parallel_tool_calls"], true);
        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert_eq!(response.assistant_text(), "Reading docs.");
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("item_1"));
        assert_eq!(
            response.tool_calls[0].provider_call_id.as_deref(),
            Some("call_1")
        );
        assert_eq!(response.tool_calls[0].name, "mcp.docs:read");
        assert_eq!(response.tool_calls[0].payload["document_id"], "doc_1");
    }

    #[tokio::test]
    async fn native_tool_only_required_response_returns_needs_tools() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "function_call",
                  "id": "item_1",
                  "call_id": "call_1",
                  "name": "search_memory",
                  "arguments": "{\"query\":\"trains\"}"
                }
              ]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let response = provider
            .generate(GenerateRequest {
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                tools: vec![search_memory_tool()],
                tool_choice: NoemaToolChoice::Auto,
                parallel_tool_calls: false,
                ..GenerateRequest::text("Search memory")
            })
            .await
            .expect("native tool response");

        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert!(response.assistant_text().is_empty());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "search_memory");
    }

    #[tokio::test]
    async fn native_required_needs_tools_text_with_empty_envelope_tool_calls_succeeds() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "message",
                  "content": [
                    {"type": "output_text", "text": "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Checking memory.\"}],\"tool_calls\":[],\"memory_proposals\":[]}"}
                  ]
                },
                {
                  "type": "function_call",
                  "id": "item_1",
                  "call_id": "call_1",
                  "name": "search_memory",
                  "arguments": "{\"query\":\"trains\"}"
                }
              ]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let response = provider
            .generate(GenerateRequest {
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                tools: vec![search_memory_tool()],
                ..GenerateRequest::text("Search memory")
            })
            .await
            .expect("native tool response");

        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert_eq!(response.assistant_text(), "Checking memory.");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "search_memory");
    }

    #[tokio::test]
    async fn native_required_response_rejects_final_answer_text() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "message",
                  "content": [
                    {"type": "output_text", "text": "{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"Done.\"}],\"tool_calls\":[],\"memory_proposals\":[]}"}
                  ]
                },
                {
                  "type": "function_call",
                  "id": "item_1",
                  "call_id": "call_1",
                  "name": "search_memory",
                  "arguments": "{\"query\":\"trains\"}"
                }
              ]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest {
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                tools: vec![search_memory_tool()],
                ..GenerateRequest::text("Search memory")
            })
            .await
            .expect_err("final answer rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("native tool response cannot include final_answer text")
        );
    }

    #[tokio::test]
    async fn native_required_response_rejects_legacy_json_tool_calls() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "message",
                  "content": [
                    {"type": "output_text", "text": "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Checking.\"}],\"tool_calls\":[{\"id\":\"legacy_1\",\"name\":\"search_memory\",\"payload\":{\"query\":\"legacy\"}}],\"memory_proposals\":[]}"}
                  ]
                },
                {
                  "type": "function_call",
                  "id": "item_1",
                  "call_id": "call_1",
                  "name": "search_memory",
                  "arguments": "{\"query\":\"trains\"}"
                }
              ]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest {
                options: crate::provider::GenerateOptions {
                    require_noema_response: true,
                    ..crate::provider::GenerateOptions::default()
                },
                tools: vec![search_memory_tool()],
                ..GenerateRequest::text("Search memory")
            })
            .await
            .expect_err("legacy tool calls rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("native tool response cannot include legacy JSON tool_calls")
        );
    }

    #[tokio::test]
    async fn rejects_provider_safe_tool_name_collisions_before_http_call() {
        let provider = test_provider("http://127.0.0.1:1".to_string());
        let error = provider
            .generate(GenerateRequest {
                tools: vec![collision_source_tool(), collision_target_tool()],
                ..GenerateRequest::text("hello")
            })
            .await
            .expect_err("collision rejected");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(
            error
                .to_string()
                .contains("provider-safe tool name collision")
        );
    }

    #[tokio::test]
    async fn maps_authentication_errors() {
        let (base_url, _request_rx) = spawn_server(
            401,
            r#"{"error":{"message":"bad key","type":"invalid_request_error"}}"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::AuthenticationFailure { message, .. } if message == "bad key"
        ));
    }

    #[tokio::test]
    async fn maps_rate_limit_errors() {
        let (base_url, _request_rx) =
            spawn_server(429, r#"{"error":{"message":"slow down"}}"#).await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::RateLimit { message, .. } if message == "slow down"
        ));
    }

    #[tokio::test]
    async fn maps_api_errors() {
        let (base_url, _request_rx) =
            spawn_server(500, r#"{"error":{"message":"upstream broke"}}"#).await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::ApiError { status: 500, message, .. } if message == "upstream broke"
        ));
    }

    #[tokio::test]
    async fn malformed_when_success_response_has_no_output_text() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [{"type": "message", "content": []}]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    }

    #[tokio::test]
    async fn rejects_empty_input_before_http_call() {
        let provider = test_provider("http://127.0.0.1:1".to_string());
        let error = provider
            .generate(GenerateRequest::text("   "))
            .await
            .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn rejects_missing_api_key() {
        let error = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: " ".to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            tool_classification_model: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::MissingCredentials { .. }));
    }

    #[test]
    fn rejects_zero_timeout() {
        let error = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            tool_classification_model: None,
            timeout_seconds: 0,
            system_errors: None,
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn default_tool_classification_model_is_gpt_5_4_mini() {
        let provider = test_provider("http://127.0.0.1:1".to_string());

        assert_eq!(
            provider.default_tool_classification_model().as_deref(),
            Some(DEFAULT_TOOL_CLASSIFICATION_MODEL)
        );
    }

    #[test]
    fn advertises_openai_responses_native_tool_capabilities() {
        let provider = test_provider("http://127.0.0.1:1".to_string());

        let capabilities = provider.tool_capabilities(Some("gpt-test"));

        assert!(capabilities.native_tools);
        assert!(capabilities.parallel_tool_calls);
        assert!(capabilities.tool_choice);
        assert!(capabilities.native_tool_results);
        assert!(capabilities.prompt_cache_retention);
        assert_eq!(
            capabilities.schema_dialect,
            ProviderToolSchemaDialect::OpenAiResponses
        );
        assert_eq!(
            capabilities.fallback_mode,
            ProviderToolFallbackMode::NativeRequired
        );
    }

    #[test]
    fn configured_tool_classification_model_overrides_provider_default() {
        let provider = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            tool_classification_model: Some("custom-tool-classifier".to_string()),
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .expect("provider");

        assert_eq!(
            provider.default_tool_classification_model().as_deref(),
            Some("custom-tool-classifier")
        );
    }

    fn test_provider(base_url: String) -> OpenAiProvider {
        OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url,
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            tool_classification_model: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .expect("provider")
    }

    fn search_memory_tool() -> NoemaToolSpec {
        NoemaToolSpec::new(
            "search_memory",
            "Search memory.",
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool")
    }

    fn mcp_docs_read_tool() -> NoemaToolSpec {
        NoemaToolSpec::new(
            "mcp.docs:read",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {"document_id": {"type": "string"}},
                "required": ["document_id"],
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool")
    }

    fn collision_source_tool() -> NoemaToolSpec {
        NoemaToolSpec::new(
            "mcp.docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool")
    }

    fn collision_target_tool() -> NoemaToolSpec {
        NoemaToolSpec::new(
            "mcp_x2e_docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool")
    }
}
