//! Provider adapter for the OpenAI Responses API.

use super::reqwest_transport_error;
use super::responses::{
    OPENAI_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest, ResponsesToolNameMap,
    ResponsesTransport, ResponsesWebSocketSession, header_value, normalize_base_url,
};
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    ModelProvider, OpenAiProviderConfig, ProviderError, ProviderGenerationFuture,
    ProviderGenerationMetadata, ProviderGenerationSession, ProviderResponseContinuation,
    ProviderSessionInput, ProviderToolCapabilities, ProviderToolSchemaDialect,
    ProviderToolTransport,
};
use noema_home::SystemErrorLogger;
use reqwest::header::{HeaderMap, HeaderName};
use std::time::Duration;

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
            .map_err(|source| reqwest_transport_error("openai", "build_client", &source))?;

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
        let transport = ResponsesTransport::new(
            client,
            config.base_url.clone(),
            Duration::from_secs(config.timeout_seconds),
        )?;
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

    fn lower_request(
        &self,
        request: &GenerateRequest,
    ) -> Result<
        (
            ResponsesRequest,
            ResponsesToolNameMap,
            ProviderToolTransport,
            ResponsesDiagnosticContext,
        ),
        ProviderError,
    > {
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
        let (mut body, tool_names, tool_transport) =
            ResponsesRequest::from_generate_with_schema_request_capabilities(
                request,
                model.clone(),
                default_reasoning_effort,
                self.schema_request_capabilities(Some(&model)),
                OPENAI_RESPONSES_PROFILE,
            )?;
        body.set_fast_mode(request.options.fast_mode);
        let diagnostics = ResponsesDiagnosticContext::new(
            self.system_errors.clone(),
            "openai",
            model,
            request.conversation_id.clone(),
        );
        Ok((body, tool_names, tool_transport, diagnostics))
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
    fn open_generation_session(&self) -> Box<dyn ProviderGenerationSession + '_> {
        Box::new(OpenAiGenerationSession {
            provider: self,
            responses: self.transport.websocket_session(),
        })
    }

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
            allowed_tools: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            request_strict_schema_when_possible: true,
            native_tool_results: true,
            prompt_cache_retention: !explicit_prompt_cache,
            prompt_cache_key: true,
            prompt_cache_options: explicit_prompt_cache,
            prompt_cache_breakpoints: explicit_prompt_cache,
            hosted_web_provider_name: Some("OpenAI"),
        }
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::PreviousResponseId {
            store_response: true,
        }
    }

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let (body, tool_names, tool_transport, diagnostics) = self.lower_request(&request)?;
        let response = self
            .transport
            .send(
                &self.config.api_key,
                body,
                self.extra_headers()?,
                diagnostics.clone(),
            )
            .await?;
        response.finalize(&tool_names, tool_transport, &diagnostics)
    }
}

struct OpenAiGenerationSession<'a> {
    provider: &'a OpenAiProvider,
    responses: ResponsesWebSocketSession,
}

impl ProviderGenerationSession for OpenAiGenerationSession<'_> {
    fn generate<'a>(
        &'a mut self,
        mut request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a> {
        Box::pin(async move {
            request.options.previous_response_id = None;
            request.options.store_response = true;
            request.input = input.replay;
            let (replay_body, tool_names, tool_transport, diagnostics) =
                self.provider.lower_request(&request)?;
            let incremental_body = input
                .incremental
                .map(|incremental| {
                    let mut incremental_request = request.clone();
                    incremental_request.input = incremental;
                    self.provider
                        .lower_request(&incremental_request)
                        .map(|(body, _, _, _)| body)
                })
                .transpose()?;
            let mut prepared = self
                .responses
                .prepare_request(replay_body.clone(), incremental_body)?;
            self.responses.begin_request(&prepared);
            let headers = self.provider.extra_headers()?;
            let mut result = self
                .responses
                .send(
                    &self.provider.config.api_key,
                    &prepared.body,
                    &headers,
                    diagnostics.clone(),
                    on_event,
                )
                .await;
            if self.responses.recover_missing_response(
                &result,
                &mut prepared,
                replay_body,
                "openai",
            )? {
                result = self
                    .responses
                    .send(
                        &self.provider.config.api_key,
                        &prepared.body,
                        &headers,
                        diagnostics.clone(),
                        on_event,
                    )
                    .await;
            }
            let response = match result {
                Ok(response) => response,
                Err(error) => {
                    let Some(reason) = error.http_fallback_reason() else {
                        return Err(error.into_provider_error());
                    };
                    self.responses.use_http(reason, prepared.used_response_id);
                    let mut body = prepared.body.clone();
                    body.stream = None;
                    self.provider
                        .transport
                        .send(
                            &self.provider.config.api_key,
                            body,
                            headers,
                            diagnostics.clone(),
                        )
                        .await?
                }
            };
            self.responses.record_response(&response);
            response.finalize(&tool_names, tool_transport, &diagnostics)
        })
    }

    fn metadata(&self) -> ProviderGenerationMetadata {
        self.responses.metadata()
    }

    fn has_active_continuation(&self) -> bool {
        self.responses.has_active_continuation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{spawn_server, spawn_websocket_server};
    use crate::{DEFAULT_OPENAI_TIMEOUT_SECONDS, GenerateInput, ProviderSessionInput, TokenUsage};

    #[tokio::test]
    async fn sends_expected_request_and_extracts_text() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [
                {
                  "type": "reasoning",
                  "id": "rs_1",
                  "encrypted_content": "opaque",
                  "summary": [{"type": "summary_text", "text": "Searching the workspace"}]
                },
                {"type": "message", "content": [
                  {"type": "output_text", "text": "Hello"},
                  {"type": "output_text", "text": ", world"}
                ]}
              ],
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
        let mut request = GenerateRequest::text("Hello?");
        request.model = Some("gpt-test".to_string());
        request.options.fast_mode = true;
        let response = provider.generate(request).await.expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: serde_json::Value = serde_json::from_str(&captured.body).expect("request body");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/responses");
        assert_eq!(body["service_tier"], "priority");
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

        assert_eq!(response.assistant_text(), "Hello, world");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-test");
        assert_eq!(response.reasoning_items[0].id.as_deref(), Some("rs_1"));
        assert_eq!(
            response.reasoning_items[0].encrypted_content.as_deref(),
            Some("opaque")
        );
        assert_eq!(
            response.reasoning_items[0].summary,
            ["Searching the workspace"]
        );
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
    async fn websocket_session_preserves_openai_storage_policy() {
        let (base_url, requests_rx) = spawn_websocket_server(vec![vec![serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "resp_1",
                "model": "gpt-test",
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "done"}]
                }]
            }
        })]])
        .await;
        let provider = test_provider(base_url);
        let mut session = provider.open_generation_session();
        session
            .generate(
                GenerateRequest::text("hello"),
                ProviderSessionInput::initial(GenerateInput::Text("hello".to_string())),
                &mut |_| {},
            )
            .await
            .expect("response");
        let requests = requests_rx.await.expect("WebSocket requests");
        assert_eq!(requests[0]["type"], "response.create");
        assert_eq!(requests[0]["store"], true);
    }

    #[tokio::test]
    async fn maps_provider_http_error_classes() {
        for (status, body, expected) in [
            (
                401,
                r#"{"error":{"message":"bad key","type":"invalid_request_error"}}"#,
                "auth",
            ),
            (429, r#"{"error":{"message":"slow down"}}"#, "rate"),
            (500, r#"{"error":{"message":"upstream broke"}}"#, "api"),
        ] {
            let (base_url, _request_rx) = spawn_server(status, body).await;
            let error = test_provider(base_url)
                .generate(GenerateRequest::text("hello"))
                .await
                .unwrap_err();
            assert!(match (expected, error) {
                ("auth", ProviderError::AuthenticationFailure { message, .. }) => {
                    message == "bad key"
                }
                ("rate", ProviderError::RateLimit { message, .. }) => message == "slow down",
                (
                    "api",
                    ProviderError::ApiError {
                        status, message, ..
                    },
                ) => {
                    status == 500 && message == "upstream broke"
                }
                _ => false,
            });
        }
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

    #[test]
    fn rejects_invalid_configuration_without_leaking_credentials() {
        let mut config = test_config("http://127.0.0.1:1");
        config.api_key = " ".to_string();
        let error = OpenAiProvider::new(config).unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredentials { .. }));

        let mut config = test_config("http://127.0.0.1:1");
        config.timeout_seconds = 0;
        let error = OpenAiProvider::new(config).unwrap_err();
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));

        let error = OpenAiProvider::new(test_config(
            "https://user:password@example.test/v1?token=secret",
        ))
        .unwrap_err();
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(!error.to_string().contains("password"));
        assert!(!error.to_string().contains("token=secret"));
    }

    #[test]
    fn provider_debug_redacts_api_key() {
        let mut config = test_config("http://127.0.0.1:1");
        config.api_key = "openai-provider-secret".to_string();
        let provider = OpenAiProvider::new(config).expect("provider");

        let debug = format!("{provider:?}");
        assert!(!debug.contains("openai-provider-secret"));
        assert!(debug.contains("[REDACTED]"));
    }

    fn test_provider(base_url: String) -> OpenAiProvider {
        OpenAiProvider::new(test_config(&base_url)).expect("provider")
    }

    fn test_config(base_url: &str) -> OpenAiProviderConfig {
        OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url: base_url.to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            tool_classification_model: None,
            reasoning_effort: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        }
    }
}
