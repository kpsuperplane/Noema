//! Provider adapter for Codex direct Responses API calls.

use std::{sync::Arc, time::Duration};

use super::{
    catalog::latest_codex_client_version,
    oauth::{CodexOAuthClient, CodexTokenStore, chatgpt_account_id_from_access_token},
};
use crate::adapters::{
    reqwest_transport_error,
    responses::{
        CODEX_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest, ResponsesTransport,
        normalize_base_url,
    },
};
use crate::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexProviderConfig, DEFAULT_CODEX_MODEL,
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    ModelProvider, ProviderError, ProviderResponseContinuation, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport,
    response_support::NoemaAssistantTextDeltaExtractor,
};
use noema_home::SystemErrorLogger;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use tokio::sync::OnceCell;

const CODEX_ORIGINATOR: &str = "codex_cli_rs";

/// Provider implementation backed by Codex OAuth and direct Responses calls.
#[derive(Debug, Clone)]
pub struct CodexResponsesProvider {
    transport: ResponsesTransport,
    version_client: reqwest::Client,
    resolved_client_version: Arc<OnceCell<String>>,
    token_store: CodexTokenStore,
    oauth_client: CodexOAuthClient,
    config: CodexProviderConfig,
    system_errors: Option<SystemErrorLogger>,
}

impl CodexResponsesProvider {
    /// Build a Codex provider with a default reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid or the HTTP
    /// client cannot be built.
    pub fn new(config: CodexProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| reqwest_transport_error("codex", "build_client", &source))?;
        Self::with_client(client, config)
    }

    /// Build a Codex provider with a caller-supplied reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn with_client(
        client: reqwest::Client,
        config: CodexProviderConfig,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let account_home =
            config
                .account_home
                .clone()
                .ok_or_else(|| ProviderError::InvalidRequest {
                    message: "codex account home is required".to_string(),
                })?;
        let transport = ResponsesTransport::new(client.clone(), config.base_url.clone())?;
        let token_store = CodexTokenStore::new(account_home);
        let oauth_client = CodexOAuthClient::new(config.oauth.clone())?;
        Ok(Self {
            transport,
            version_client: client,
            resolved_client_version: Arc::new(OnceCell::new()),
            token_store,
            oauth_client,
            system_errors: config.system_errors.clone(),
            config,
        })
    }

    /// Return the configured token store.
    #[must_use]
    pub fn token_store(&self) -> &CodexTokenStore {
        &self.token_store
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
    async fn generate_with_events(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let require_noema_response = request.options.require_noema_response;
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
        let (body, tool_names) = ResponsesRequest::from_generate(
            &request,
            model.clone(),
            default_reasoning_effort,
            CODEX_RESPONSES_PROFILE,
        )?;
        let diagnostics = ResponsesDiagnosticContext::new(
            self.system_errors.clone(),
            "codex",
            model.clone(),
            request.conversation_id.clone(),
        );

        let access_token = self
            .token_store
            .access_token(&self.oauth_client, CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS)
            .await?;
        let request_headers = self
            .request_headers(&access_token, request.conversation_id.as_deref())
            .await?;
        let mut noema_delta_extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut forward_event = |event| {
            if require_noema_response {
                if let GenerateStreamEvent::AssistantTextDelta { delta, .. } = event {
                    noema_delta_extractor.push_delta(&delta, on_event);
                }
            } else {
                on_event(event);
            }
        };
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
                let refreshed = self
                    .token_store
                    .refresh_access_token(&self.oauth_client)
                    .await?;
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
        response.finalize(&tool_names, require_noema_response, &diagnostics)
    }
}

impl ModelProvider for CodexResponsesProvider {
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
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: true,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: codex_encrypted_reasoning_supported(),
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
