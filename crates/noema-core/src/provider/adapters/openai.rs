//! Provider adapter for the OpenAI Responses API.

use super::responses::{
    ResponsesDiagnosticContext, ResponsesInput, ResponsesRequest, ResponsesTransport, header_value,
    noema_response_text_format, normalize_base_url,
};
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, ModelProvider,
        ProviderError, output_items_from_text, required_output_items_from_text,
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
        let text = match response.output_text() {
            Ok(text) => text,
            Err(error @ ProviderError::MalformedResponse { .. }) => {
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

        let output = if request.options.require_noema_response {
            match required_output_items_from_text(text) {
                Ok(output) => output,
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
            output_items_from_text(text)?
        };

        Ok(GenerateResponse {
            output,
            provider: "openai".to_string(),
            model: response.model.unwrap_or(model),
            response_id: response.id,
            usage: response.usage.map(Into::into),
        })
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
    use crate::provider::TokenUsage;
    use crate::provider::adapters::test_support::spawn_server;
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
                  {"type": "output_text", "text": "{\"type\":\"noema_response\",\"output\":[{\"kind\":\"assistant_text\",\"text\":\"Hello\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}"}
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
            body["text"]["format"]["schema"]["properties"]["type"]["const"],
            "noema_response"
        );
        assert_eq!(response.assistant_text(), "Hello");
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
}
