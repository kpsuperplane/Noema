//! OpenRouter Responses provider backed by a lazily connected account.

use std::time::Duration;

use noema_home::SystemErrorLogger;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::{
    account_service::ProviderCredentialAccessHandle,
    reqwest_transport_error,
    responses::{
        OPENROUTER_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest,
        ResponsesTransport, normalize_base_url,
    },
};
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    ModelProvider, OpenRouterProviderConfig, ProviderContextMetadata, ProviderError,
    ProviderResponseContinuation, ProviderSchemaCapabilities, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport, SchemaEnforcement,
};

pub(crate) mod catalog;

const OPENROUTER_PROVIDER_ACCOUNT_ID: &str = "provider_account:openrouter:default";
const OPENROUTER_CONTEXT_WINDOW_TOKENS: u32 = 32_768;
const OPENROUTER_OUTPUT_RESERVE_TOKENS: u32 = 8_192;
const OPENROUTER_SUMMARY_TARGET_TOKENS: u32 = 2_048;

#[derive(Clone)]
pub struct OpenRouterProvider {
    transport: ResponsesTransport,
    credentials: ProviderCredentialAccessHandle,
    config: OpenRouterProviderConfig,
    system_errors: Option<SystemErrorLogger>,
}

impl std::fmt::Debug for OpenRouterProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenRouterProvider")
            .field("credentials", &"[CONFIGURED]")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl OpenRouterProvider {
    pub(crate) fn new(
        config: OpenRouterProviderConfig,
        credentials: ProviderCredentialAccessHandle,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| reqwest_transport_error("openrouter", "build_client", &source))?;
        let transport = ResponsesTransport::new(client, config.base_url.clone())?;
        Ok(Self {
            transport,
            credentials,
            system_errors: config.system_errors.clone(),
            config,
        })
    }

    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("http-referer"),
            HeaderValue::from_static("https://github.com/kpsuperplane/Noema"),
        );
        headers.insert(
            HeaderName::from_static("x-openrouter-title"),
            HeaderValue::from_static("Noema"),
        );
        headers
    }

    async fn generate_with_events(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let request_model = request
            .model
            .as_deref()
            .filter(|model| !model.trim().is_empty())
            .map(str::trim)
            .unwrap_or(&self.config.default_model)
            .to_string();
        let default_effort = request
            .model
            .is_none()
            .then_some(self.config.reasoning_effort)
            .flatten();
        let (mut body, names, transport) =
            ResponsesRequest::from_generate_with_schema_capabilities(
                &request,
                request_model.clone(),
                default_effort,
                self.schema_capabilities(Some(&request_model)),
                OPENROUTER_RESPONSES_PROFILE,
            )?;
        body.store = false;
        body.previous_response_id = None;
        let diagnostics = ResponsesDiagnosticContext::new(
            self.system_errors.clone(),
            "openrouter",
            request_model,
            request.conversation_id.clone(),
        );
        let credential = self
            .credentials
            .api_key("openrouter", OPENROUTER_PROVIDER_ACCOUNT_ID)
            .await?;
        let response = self
            .transport
            .send_streaming(
                credential.expose_secret(),
                body,
                Self::headers(),
                diagnostics.clone(),
                on_event,
            )
            .await?;
        // OpenRouter may treat `text.format` as best effort on tool-bearing
        // Responses routes, so normalize either the envelope or native output.
        response.finalize(&names, transport, false, &diagnostics)
    }
}

fn normalize_config(
    mut config: OpenRouterProviderConfig,
) -> Result<OpenRouterProviderConfig, ProviderError> {
    config.base_url = normalize_base_url(config.base_url, "openrouter base URL")?;
    config.default_model = config.default_model.trim().to_string();
    if config.default_model.is_empty() || config.timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "openrouter model and positive timeout are required".to_string(),
        });
    }
    config.tool_classification_model = config.tool_classification_model.and_then(|model| {
        let model = model.trim().to_string();
        (!model.is_empty()).then_some(model)
    });
    Ok(config)
}

impl ModelProvider for OpenRouterProvider {
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(
            self.config
                .tool_classification_model
                .clone()
                .unwrap_or_else(|| DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string()),
        )
    }

    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(OPENROUTER_CONTEXT_WINDOW_TOKENS),
            default_output_reserve_tokens: Some(OPENROUTER_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(OPENROUTER_SUMMARY_TARGET_TOKENS),
        }
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: true,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: false,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: true,
            hosted_web_provider_name: Some("OpenRouter"),
        }
    }

    fn schema_capabilities(&self, _model: Option<&str>) -> ProviderSchemaCapabilities {
        ProviderSchemaCapabilities {
            native_tool_arguments: SchemaEnforcement::Strict,
            structured_output: SchemaEnforcement::Strict,
            structured_output_with_tools: SchemaEnforcement::Strict,
        }
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::Unsupported
    }

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        self.generate_with_events(request, &mut |_| {}).await
    }

    async fn generate_streaming(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        self.generate_with_events(request, on_event).await
    }
}
