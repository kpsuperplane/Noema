//! Provider adapter for the OpenAI Responses API.

use super::responses::{
    OPENAI_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest, ResponsesTransport,
    header_value, normalize_base_url,
};
use crate::provider::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, ModelProvider,
    ProviderError, ProviderResponseContinuation, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport,
};
use noema_home::SystemErrorLogger;
use reqwest::header::{HeaderMap, HeaderName};
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
    /// Optional explicit reasoning effort used only when config supplies an explicit model.
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
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

    fn tool_capabilities(&self, model: Option<&str>) -> ProviderToolCapabilities {
        let model = model.unwrap_or(&self.config.default_model);
        let explicit_prompt_cache = model == "gpt-5.6" || model.starts_with("gpt-5.6-");
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: !explicit_prompt_cache,
            prompt_cache_key: true,
            prompt_cache_options: explicit_prompt_cache,
            prompt_cache_breakpoints: explicit_prompt_cache,
            encrypted_reasoning: true,
        }
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::PreviousResponseId {
            store_response: true,
        }
    }

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let request_model = request
            .model
            .as_deref()
            .filter(|model| !model.trim().is_empty())
            .map(|model| model.trim().to_string());
        let using_config_default_model = request_model.is_none();
        let model = request_model.unwrap_or_else(|| self.config.default_model.clone());

        if model.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "model cannot be empty".to_string(),
            });
        }

        let default_reasoning_effort = using_config_default_model
            .then_some(self.config.reasoning_effort)
            .flatten();
        let (body, tool_names) = ResponsesRequest::from_generate(
            &request,
            model.clone(),
            default_reasoning_effort,
            OPENAI_RESPONSES_PROFILE,
        )?;

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
                diagnostics.clone(),
            )
            .await?;
        response.finalize(
            &tool_names,
            request.options.require_noema_response,
            &diagnostics,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::adapters::test_support::spawn_server;
    use crate::provider::{
        GenerateResponseStatus, NoemaToolChoice, ProviderToolSchemaDialect, ProviderToolTransport,
        TokenUsage,
    };
    use crate::{GenerateInput, PromptCacheRetention};
    use noema_capabilities::ToolSpec;
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
            reasoning_effort: None,
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
    async fn openai_requests_and_parses_encrypted_reasoning_items() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "reasoning",
                  "id": "rs_1",
                  "encrypted_content": "opaque-openai-reasoning"
                },
                {
                  "type": "message",
                  "content": [{"type": "output_text", "text": "Done"}]
                }
              ]
            }"#,
        )
        .await;
        let provider = test_provider(base_url);

        let response = provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:reasoning".to_string()),
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Think privately.".to_string()),
                ..GenerateRequest::text("ignored")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["include"][0], "reasoning.encrypted_content");
        assert_eq!(response.reasoning_items.len(), 1);
        assert_eq!(response.reasoning_items[0].id.as_deref(), Some("rs_1"));
        assert_eq!(
            response.reasoning_items[0].encrypted_content.as_deref(),
            Some("opaque-openai-reasoning")
        );
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
                    {"type": "output_text", "text": "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Reading docs.\"}],\"tool_calls\":[]}"}
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
            reasoning_effort: None,
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
            reasoning_effort: None,
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
        let continuation = provider.response_continuation(Some("gpt-test"));

        assert_eq!(capabilities.tool_transport, ProviderToolTransport::Native);
        assert!(capabilities.parallel_tool_calls);
        assert!(capabilities.tool_choice);
        assert!(capabilities.native_tool_results);
        assert!(capabilities.prompt_cache_retention);
        assert!(capabilities.prompt_cache_key);
        assert!(capabilities.encrypted_reasoning);
        assert_eq!(
            capabilities.schema_dialect,
            ProviderToolSchemaDialect::OpenAiResponses
        );
        assert_eq!(
            continuation,
            ProviderResponseContinuation::PreviousResponseId {
                store_response: true
            }
        );
    }

    #[test]
    fn gpt_5_6_uses_explicit_prompt_cache_controls_without_legacy_retention() {
        let provider = test_provider("http://127.0.0.1:1".to_string());

        let capabilities = provider.tool_capabilities(Some("gpt-5.6"));

        assert!(!capabilities.prompt_cache_retention);
        assert!(capabilities.prompt_cache_key);
        assert!(capabilities.prompt_cache_options);
        assert!(capabilities.prompt_cache_breakpoints);
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
            reasoning_effort: None,
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
            reasoning_effort: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .expect("provider")
    }

    fn mcp_docs_read_tool() -> ToolSpec {
        ToolSpec::new(
            "mcp.docs:read",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {"document_id": {"type": "string"}},
                "required": ["document_id"],
                "additionalProperties": false
            }),
        )
        .expect("tool")
    }
}
