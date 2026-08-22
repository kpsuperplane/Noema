//! Provider adapter for Codex direct Responses API calls.

use std::{sync::Arc, time::Duration};

use super::{catalog::latest_codex_client_version, oauth::chatgpt_account_id_from_access_token};
use crate::adapters::{
    account_service::{ProviderCredential, ProviderCredentialAccessHandle},
    reqwest_transport_error,
    responses::{
        CODEX_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest,
        ResponsesToolNameMap, ResponsesTransport, ResponsesWebSocketError,
        ResponsesWebSocketSession, normalize_base_url,
    },
};
use crate::{
    CodexProviderConfig, DEFAULT_CODEX_MODEL, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest,
    GenerateResponse, GenerateStreamEvent, ModelProvider, ProviderContextMetadata, ProviderError,
    ProviderGenerationFuture, ProviderGenerationMetadata, ProviderGenerationSession,
    ProviderResponseContinuation, ProviderSchemaRequest, ProviderSchemaRequestCapabilities,
    ProviderSessionInput, ProviderToolCapabilities, ProviderToolSchemaDialect,
    ProviderToolTransport,
};
use noema_home::SystemErrorLogger;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use tokio::sync::OnceCell;

const CODEX_ORIGINATOR: &str = "codex_cli_rs";
// The current Codex catalog's smallest listed model window is 128k tokens.
// Using that lower bound keeps unknown/new profiles on the safe side until
// catalog metadata becomes part of provider-instance construction.
const CODEX_CONTEXT_WINDOW_TOKENS: u32 = 128_000;
const CODEX_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 8_000;
const CODEX_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 2_048;

/// Provider implementation backed by Codex OAuth and direct Responses calls.
#[derive(Clone)]
pub struct CodexResponsesProvider {
    transport: ResponsesTransport,
    version_client: reqwest::Client,
    resolved_client_version: Arc<OnceCell<String>>,
    credentials: CodexCredentialSource,
    config: CodexProviderConfig,
    system_errors: Option<SystemErrorLogger>,
}

#[derive(Clone)]
enum CodexCredentialSource {
    Service {
        provider_account_id: String,
        access: ProviderCredentialAccessHandle,
    },
}

impl std::fmt::Debug for CodexCredentialSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Service {
                provider_account_id,
                ..
            } => formatter
                .debug_struct("CodexCredentialSource::Service")
                .field("provider_account_id", provider_account_id)
                .field("access", &"[CONFIGURED]")
                .finish(),
        }
    }
}

impl std::fmt::Debug for CodexResponsesProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexResponsesProvider")
            .field("credentials", &self.credentials)
            .field("config", &self.config)
            .field("system_errors", &self.system_errors)
            .finish_non_exhaustive()
    }
}

impl CodexResponsesProvider {
    /// Build a Codex provider using provider-account credential access.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration or the HTTP client is
    /// invalid.
    pub(crate) fn new_with_credentials(
        config: CodexProviderConfig,
        provider_account_id: impl Into<String>,
        access: ProviderCredentialAccessHandle,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| reqwest_transport_error("codex", "build_client", &source))?;
        Self::with_client_and_credentials(client, config, provider_account_id, access)
    }

    /// Build a Codex provider with injected HTTP and credential access.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn with_client_and_credentials(
        client: reqwest::Client,
        config: CodexProviderConfig,
        provider_account_id: impl Into<String>,
        access: ProviderCredentialAccessHandle,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let provider_account_id = provider_account_id.into();
        if provider_account_id.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex provider account id is required".to_string(),
            });
        }
        let transport = ResponsesTransport::new(
            client.clone(),
            config.base_url.clone(),
            Duration::from_secs(config.timeout_seconds),
        )?;
        Ok(Self {
            transport,
            version_client: client,
            resolved_client_version: Arc::new(OnceCell::new()),
            credentials: CodexCredentialSource::Service {
                provider_account_id,
                access,
            },
            system_errors: config.system_errors.clone(),
            config,
        })
    }

    fn model_for_request(&self, model: Option<String>) -> Result<String, ProviderError> {
        let model = model
            .filter(|model| !model.trim().is_empty())
            .or_else(|| self.config.default_model.clone())
            .unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string());
        let model = model.trim().to_string();
        if model.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex model cannot be empty".to_string(),
            });
        }
        Ok(model)
    }

    async fn client_version(&self) -> &str {
        if let Some(version) = self.config.client_version.as_deref() {
            return version;
        }
        self.resolved_client_version
            .get_or_init(|| latest_codex_client_version(&self.version_client))
            .await
    }

    async fn request_headers(
        &self,
        access_token: &str,
        session_id: Option<&str>,
    ) -> Result<HeaderMap, ProviderError> {
        let version = self.client_version().await;
        let mut headers = HeaderMap::new();
        headers.insert("originator", HeaderValue::from_static(CODEX_ORIGINATOR));
        headers.insert(
            USER_AGENT,
            crate::adapters::responses::header_value(
                &format!("{CODEX_ORIGINATOR}/{version} (Noema)"),
                "codex user agent",
            )?,
        );
        headers.insert(
            "version",
            crate::adapters::responses::header_value(version, "codex client version")?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        if let Some(account_id) = chatgpt_account_id_from_access_token(access_token) {
            headers.insert(
                "ChatGPT-Account-ID",
                crate::adapters::responses::header_value(&account_id, "ChatGPT account id")?,
            );
        }
        if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
            headers.insert(
                "session-id",
                crate::adapters::responses::header_value(session_id, "Codex session id")?,
            );
        }
        Ok(headers)
    }

    async fn access_token(&self) -> Result<String, ProviderError> {
        match &self.credentials {
            CodexCredentialSource::Service {
                provider_account_id,
                access,
            } => access
                .codex_access_token(provider_account_id)
                .await
                .map(ProviderCredential::into_secret),
        }
    }

    async fn refresh_access_token(&self) -> Result<String, ProviderError> {
        match &self.credentials {
            CodexCredentialSource::Service {
                provider_account_id,
                access,
            } => access
                .refresh_codex_access_token(provider_account_id)
                .await
                .map(ProviderCredential::into_secret),
        }
    }
}

fn normalize_config(mut config: CodexProviderConfig) -> Result<CodexProviderConfig, ProviderError> {
    config.base_url = normalize_base_url(config.base_url, "codex base URL")?;
    if config.timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "codex timeout must be greater than zero seconds".to_string(),
        });
    }
    config.default_model = config.default_model.and_then(|model| {
        let model = model.trim().to_string();
        (!model.is_empty()).then_some(model)
    });
    config.tool_classification_model = config.tool_classification_model.and_then(|model| {
        let model = model.trim().to_string();
        (!model.is_empty()).then_some(model)
    });
    config.client_version = config.client_version.and_then(|version| {
        let version = version.trim().to_string();
        (!version.is_empty()).then_some(version)
    });
    Ok(config)
}

fn codex_encrypted_reasoning_supported() -> bool {
    false
}

impl CodexResponsesProvider {
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
            .as_ref()
            .filter(|model| !model.trim().is_empty())
            .map(|model| model.trim().to_string());
        let using_config_default_model = request_model.is_none();
        let model = self.model_for_request(request_model)?;
        let default_reasoning_effort = using_config_default_model
            .then_some(self.config.reasoning_effort)
            .flatten();
        let (mut body, tool_names, tool_transport) =
            ResponsesRequest::from_generate_with_schema_request_capabilities(
                request,
                model.clone(),
                default_reasoning_effort,
                self.schema_request_capabilities(Some(&model)),
                CODEX_RESPONSES_PROFILE,
            )?;
        body.set_fast_mode(request.options.fast_mode);
        let diagnostics = ResponsesDiagnosticContext::new(
            self.system_errors.clone(),
            "codex",
            model,
            request.conversation_id.clone(),
        );
        Ok((body, tool_names, tool_transport, diagnostics))
    }

    async fn generate_with_events(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let (body, tool_names, tool_transport, diagnostics) = self.lower_request(&request)?;
        let access_token = self.access_token().await?;
        let request_headers = self
            .request_headers(&access_token, request.conversation_id.as_deref())
            .await?;
        let mut forward_event = |event| on_event(event);
        let response = match self
            .transport
            .send_streaming(
                &access_token,
                body.clone(),
                request_headers,
                diagnostics.clone(),
                &mut forward_event,
            )
            .await
        {
            Ok(response) => response,
            Err(ProviderError::AuthenticationFailure { .. }) => {
                let refreshed = self.refresh_access_token().await?;
                let request_headers = self
                    .request_headers(&refreshed, request.conversation_id.as_deref())
                    .await?;
                self.transport
                    .send_streaming(
                        &refreshed,
                        body,
                        request_headers,
                        diagnostics.clone(),
                        &mut forward_event,
                    )
                    .await?
            }
            Err(error) => return Err(error),
        };
        response.finalize(&tool_names, tool_transport, &diagnostics)
    }
}

struct CodexGenerationSession<'a> {
    provider: &'a CodexResponsesProvider,
    responses: ResponsesWebSocketSession,
}

impl ProviderGenerationSession for CodexGenerationSession<'_> {
    fn generate<'a>(
        &'a mut self,
        mut request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a> {
        Box::pin(async move {
            request.options.previous_response_id = None;
            request.options.store_response = false;
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
            let mut access_token = self.provider.access_token().await?;
            let mut headers = self
                .provider
                .request_headers(&access_token, request.conversation_id.as_deref())
                .await?;
            let mut result = self
                .responses
                .send(
                    &access_token,
                    &prepared.body,
                    &headers,
                    diagnostics.clone(),
                    on_event,
                )
                .await;
            if matches!(result, Err(ResponsesWebSocketError::Authentication(_))) {
                access_token = self.provider.refresh_access_token().await?;
                headers = self
                    .provider
                    .request_headers(&access_token, request.conversation_id.as_deref())
                    .await?;
                result = self
                    .responses
                    .send(
                        &access_token,
                        &prepared.body,
                        &headers,
                        diagnostics.clone(),
                        on_event,
                    )
                    .await;
            }
            if matches!(
                result,
                Err(ResponsesWebSocketError::PreviousResponseNotFound)
            ) && prepared.used_response_id
            {
                if self.responses.has_hosted_web_state() {
                    return Err(ProviderError::ProtocolError {
                        provider: "codex".to_string(),
                        message: "provider-hosted web state expired; this response cannot continue safely"
                            .to_string(),
                    });
                }
                self.responses.clear_response_id();
                prepared = self.responses.prepare_request(replay_body.clone(), None)?;
                self.responses.replay_missing_response();
                result = self
                    .responses
                    .send(
                        &access_token,
                        &prepared.body,
                        &headers,
                        diagnostics.clone(),
                        on_event,
                    )
                    .await;
            }
            let response = match result {
                Ok(response) => response,
                Err(ResponsesWebSocketError::Unsupported(_)) => {
                    if self.responses.has_hosted_web_state() {
                        return Err(ProviderError::ProtocolError {
                            provider: "codex".to_string(),
                            message: "the Codex connection lost provider-hosted web state; this response cannot continue safely"
                                .to_string(),
                        });
                    }
                    self.responses.use_http("unsupported_websocket", false);
                    let mut replay_body = replay_body;
                    replay_body.previous_response_id = None;
                    self.provider
                        .transport
                        .send_streaming(
                            &access_token,
                            replay_body,
                            headers,
                            diagnostics.clone(),
                            on_event,
                        )
                        .await?
                }
                Err(ResponsesWebSocketError::Setup(_)) => {
                    if self.responses.has_hosted_web_state() {
                        return Err(ProviderError::ProtocolError {
                            provider: "codex".to_string(),
                            message: "the Codex connection lost provider-hosted web state; this response cannot continue safely"
                                .to_string(),
                        });
                    }
                    self.responses.use_http("websocket_setup_failed", false);
                    let mut replay_body = replay_body;
                    replay_body.previous_response_id = None;
                    self.provider
                        .transport
                        .send_streaming(
                            &access_token,
                            replay_body,
                            headers,
                            diagnostics.clone(),
                            on_event,
                        )
                        .await?
                }
                Err(error) => return Err(error.into_provider_error()),
            };
            self.responses.record_response(&prepared, &response);
            if self.responses.metadata().transport != Some("responses_websocket") {
                self.responses.clear_response_id();
            }
            response.finalize(&tool_names, tool_transport, &diagnostics)
        })
    }

    fn metadata(&self) -> ProviderGenerationMetadata {
        self.responses.metadata()
    }

    fn has_active_continuation(&self) -> bool {
        self.responses.metadata().transport == Some("responses_websocket")
            && self.responses.has_active_continuation()
    }
}

impl ModelProvider for CodexResponsesProvider {
    fn open_generation_session(&self) -> Box<dyn ProviderGenerationSession + '_> {
        Box::new(CodexGenerationSession {
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

    async fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(CODEX_CONTEXT_WINDOW_TOKENS),
            default_output_reserve_tokens: Some(CODEX_DEFAULT_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(CODEX_COMPACT_SUMMARY_TARGET_TOKENS),
        }
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            request_strict_schema_when_possible: true,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: true,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: codex_encrypted_reasoning_supported(),
            hosted_web_provider_name: Some("OpenAI"),
        }
    }

    fn schema_request_capabilities(
        &self,
        _model: Option<&str>,
    ) -> ProviderSchemaRequestCapabilities {
        ProviderSchemaRequestCapabilities {
            native_tool_arguments: ProviderSchemaRequest::RequestStrictWhenPossible,
        }
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::PreviousResponseId {
            store_response: false,
        }
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

#[cfg(test)]
mod tests;
