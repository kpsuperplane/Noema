//! Provider adapter for the OpenAI Responses API.

use super::reqwest_transport_error;
use super::responses::{
    OPENAI_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest, ResponsesTransport,
    header_value, normalize_base_url,
};
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, ModelProvider,
    OpenAiProviderConfig, ProviderError, ProviderResponseContinuation,
    ProviderSchemaRequestCapabilities, ProviderToolCapabilities, ProviderToolSchemaDialect,
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
            request_strict_schema_when_possible: true,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: !explicit_prompt_cache,
            prompt_cache_key: true,
            prompt_cache_options: explicit_prompt_cache,
            prompt_cache_breakpoints: explicit_prompt_cache,
            encrypted_reasoning: true,
            hosted_web_provider_name: Some("OpenAI"),
        }
    }

    fn schema_request_capabilities(
        &self,
        _model: Option<&str>,
    ) -> ProviderSchemaRequestCapabilities {
        ProviderSchemaRequestCapabilities::request_strict_when_possible()
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
        let (mut body, tool_names, tool_transport) =
            ResponsesRequest::from_generate_with_schema_request_capabilities(
                &request,
                model.clone(),
                default_reasoning_effort,
                self.schema_request_capabilities(Some(&model)),
                OPENAI_RESPONSES_PROFILE,
            )?;
        body.set_fast_mode(self.config.fast_mode);

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
        response.finalize(&tool_names, tool_transport, &diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_OPENAI_TIMEOUT_SECONDS;
    use crate::TokenUsage;
    use crate::adapters::test_support::spawn_server;

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
            fast_mode: true,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        })
        .expect("provider");

        let mut request = GenerateRequest::text("Hello?");
        request.model = Some("gpt-test".to_string());
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
            fast_mode: false,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        }
    }
}
