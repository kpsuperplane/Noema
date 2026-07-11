//! Provider adapter for Codex direct Responses API calls.

use std::{path::PathBuf, sync::Arc, time::Duration};

use super::{
    codex_oauth::{
        CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthClient, CodexOAuthConfig,
        CodexTokenStore, DEFAULT_CODEX_BASE_URL, chatgpt_account_id_from_access_token,
    },
    noema_response_stream::NoemaAssistantTextDeltaExtractor,
    responses::{
        CODEX_RESPONSES_PROFILE, ResponsesDiagnosticContext, ResponsesRequest, ResponsesTransport,
        normalize_base_url,
    },
};
use crate::{
    SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
        ModelProvider, ProviderError, ProviderToolCapabilities, ProviderToolFallbackMode,
        ProviderToolSchemaDialect, model_catalog::latest_codex_client_version,
    },
};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use tokio::sync::OnceCell;

const CODEX_ORIGINATOR: &str = "codex_cli_rs";

/// Default Codex Responses model used when no override is supplied.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.5";
/// Default request timeout for Codex Responses calls.
pub const DEFAULT_CODEX_TIMEOUT_SECONDS: u64 = 300;

/// Configuration for the Codex direct Responses provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    /// Base URL for the Codex Responses API.
    pub base_url: String,
    /// Optional default model.
    pub default_model: Option<String>,
    /// Optional model override for metadata-only tool classification.
    pub tool_classification_model: Option<String>,
    /// Optional explicit reasoning effort used only when config supplies an explicit model.
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Codex client version advertised to the subscription backend.
    pub client_version: Option<String>,
    /// Provider account home containing Noema-owned token state.
    pub account_home: Option<PathBuf>,
    /// OAuth endpoint configuration used for token refresh and login.
    pub oauth: CodexOAuthConfig,
    /// Developer diagnostic system error logger.
    pub system_errors: Option<SystemErrorLogger>,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            default_model: Some(DEFAULT_CODEX_MODEL.to_string()),
            tool_classification_model: None,
            reasoning_effort: None,
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
            client_version: None,
            account_home: None,
            oauth: CodexOAuthConfig::default(),
            system_errors: None,
        }
    }
}

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
            .map_err(|source| ProviderError::HttpFailure { source })?;
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
            super::responses::header_value(
                &format!("{CODEX_ORIGINATOR}/{version} (Noema)"),
                "codex user agent",
            )?,
        );
        headers.insert(
            "version",
            super::responses::header_value(version, "codex client version")?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        if let Some(account_id) = chatgpt_account_id_from_access_token(access_token) {
            headers.insert(
                "ChatGPT-Account-ID",
                super::responses::header_value(&account_id, "ChatGPT account id")?,
            );
        }
        if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
            headers.insert(
                "session-id",
                super::responses::header_value(session_id, "Codex session id")?,
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
            native_tools: true,
            parallel_tool_calls: true,
            tool_choice: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: true,
            encrypted_reasoning: codex_encrypted_reasoning_supported(),
            fallback_mode: ProviderToolFallbackMode::NativeRequired,
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
mod tests {
    use super::*;
    use crate::SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE;
    use crate::provider::adapters::codex_oauth::CodexOAuthTokens;
    use crate::provider::adapters::test_support::spawn_server;
    use crate::provider::{
        GenerateInput, GenerateOptions, GenerateResponseStatus, NoemaToolChoice,
        NoemaToolExecution, NoemaToolSpec, ProviderToolFallbackMode, ProviderToolSchemaDialect,
    };
    use serde_json::Value;
    use tempfile::TempDir;

    #[test]
    fn rejects_missing_account_home() {
        let error = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: None,
            ..CodexProviderConfig::default()
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn accepts_noema_owned_token_store() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: Some(account_home.clone()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        provider
            .token_store()
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("write token");

        assert!(provider.token_store().has_usable_tokens());
    }

    #[test]
    fn default_tool_classification_model_is_gpt_5_4_mini() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: Some(account_home),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        assert_eq!(
            provider.default_tool_classification_model().as_deref(),
            Some(DEFAULT_TOOL_CLASSIFICATION_MODEL)
        );
    }

    #[test]
    fn configured_tool_classification_model_overrides_provider_default() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: Some(account_home),
            tool_classification_model: Some("custom-tool-classifier".to_string()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        assert_eq!(
            provider.default_tool_classification_model().as_deref(),
            Some("custom-tool-classifier")
        );
    }

    #[test]
    fn advertises_codex_responses_native_tool_capabilities() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: Some(account_home),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        let capabilities = provider.tool_capabilities(Some("gpt-test"));

        assert!(capabilities.native_tools);
        assert!(capabilities.parallel_tool_calls);
        assert!(capabilities.tool_choice);
        assert!(capabilities.native_tool_results);
        assert!(capabilities.prompt_cache_key);
        assert!(!capabilities.prompt_cache_retention);
        assert_eq!(
            capabilities.encrypted_reasoning,
            codex_encrypted_reasoning_supported()
        );
        assert_eq!(
            capabilities.schema_dialect,
            ProviderToolSchemaDialect::OpenAiResponses
        );
        assert_eq!(
            capabilities.fallback_mode,
            ProviderToolFallbackMode::NativeRequired
        );
    }

    #[tokio::test]
    async fn sends_codex_input_as_response_message_list() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3}}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest::text("Hello?"))
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/responses");
        assert_eq!(
            captured.headers.get("authorization"),
            Some(&format!("Bearer {}", test_access_token()))
        );
        assert_eq!(
            captured
                .headers
                .get("chatgpt-account-id")
                .map(String::as_str),
            Some("workspace-test")
        );
        assert_eq!(
            captured.headers.get("originator").map(String::as_str),
            Some(CODEX_ORIGINATOR)
        );
        assert_eq!(
            captured.headers.get("version").map(String::as_str),
            Some("0.144.0")
        );
        assert_eq!(
            captured.headers.get("user-agent").map(String::as_str),
            Some("codex_cli_rs/0.144.0 (Noema)")
        );
        assert_eq!(
            captured.headers.get("accept").map(String::as_str),
            Some("text/event-stream")
        );
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "Hello?");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("parallel_tool_calls").is_none());

        assert_eq!(response.assistant_text(), "Hello");
    }

    #[tokio::test]
    async fn codex_request_sends_native_tool_specs_with_provider_safe_names() {
        let response_body = "event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":[{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"mcp_x2e_docs_x3a_read\",\"arguments\":\"{\\\"document_id\\\":\\\"doc_1\\\"}\"}]}}\n\
             \n";
        let (base_url, request_rx) = spawn_server(200, response_body).await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest {
                conversation_id: None,
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Read it".to_string()),
                instructions: None,
                options: GenerateOptions {
                    require_noema_response: true,
                    ..GenerateOptions::default()
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
        assert_eq!(response.tool_calls[0].name, "mcp.docs:read");
    }

    #[tokio::test]
    async fn codex_sse_mixed_streamed_text_and_function_call_returns_needs_tools() {
        let response_body = "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"response_status\\\":\\\"needs_tools\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"commentary\\\",\\\"text\\\":\\\"Checking.\\\"}],\\\"tool_calls\\\":[]}\"}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":null}}\n\
             \n";
        let (base_url, _request_rx) = spawn_server(200, response_body).await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest {
                options: GenerateOptions {
                    require_noema_response: true,
                    ..GenerateOptions::default()
                },
                tools: vec![search_memory_tool()],
                ..GenerateRequest::text("Search memory")
            })
            .await
            .expect("response");

        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert_eq!(response.assistant_text(), "Checking.");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("item_1"));
        assert_eq!(
            response.tool_calls[0].provider_call_id.as_deref(),
            Some("call_1")
        );
        assert_eq!(response.tool_calls[0].name, "search_memory");
        assert_eq!(response.tool_calls[0].payload["query"], "trains");
    }

    #[tokio::test]
    async fn codex_parses_encrypted_reasoning_items_when_returned() {
        let (base_url, _request_rx) = spawn_server(
            200,
            "event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":[{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"opaque-codex-reasoning\"},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Done\"}]}]}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest::text("Hello?"))
            .await
            .expect("response");

        assert_eq!(response.assistant_text(), "Done");
        assert_eq!(response.reasoning_items.len(), 1);
        assert_eq!(
            response.reasoning_items[0].encrypted_content.as_deref(),
            Some("opaque-codex-reasoning")
        );
    }

    #[tokio::test]
    async fn forwards_codex_streaming_text_deltas() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let mut events = Vec::new();
        let response = provider
            .generate_streaming(GenerateRequest::text("Hello?"), &mut |event| {
                events.push(event);
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert!(body.get("text").is_none());

        assert_eq!(response.assistant_text(), "Hello");
        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "lo".to_string()
                }
            ]
        );
    }

    #[tokio::test]
    async fn generate_streaming_required_noema_response_emits_only_assistant_text_deltas() {
        let response_body = format!(
            "{}{}{}",
            sse_delta(
                r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hel"#
            ),
            sse_delta(r#"lo"}],"tool_calls":[]}"#),
            sse_completed(),
        );
        let (base_url, request_rx) = spawn_server(200, response_body).await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let request = GenerateRequest {
            options: GenerateOptions {
                require_noema_response: true,
                ..GenerateOptions::default()
            },
            ..GenerateRequest::text("Hello?")
        };
        let mut events = Vec::new();
        let response = provider
            .generate_streaming(request, &mut |event| {
                events.push(event);
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
        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "lo".to_string()
                }
            ]
        );
    }

    #[tokio::test]
    async fn logs_required_noema_response_parse_failure() {
        let response_body = "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"output\\\":[]}\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_bad\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n";
        let (base_url, _request_rx) = spawn_server(200, response_body).await;
        let dir = TempDir::new().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
        let (mut provider, _tokens_dir) = provider_with_tokens(base_url);
        provider.system_errors = Some(logger.clone());

        let error = provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:test".to_string()),
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("hello".to_string()),
                instructions: None,
                options: GenerateOptions {
                    require_noema_response: true,
                    ..GenerateOptions::default()
                },
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect_err("malformed response");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        let events = crate::system_errors::read_system_error_events(logger.path()).expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]["category"],
            SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE
        );
        assert_eq!(events[0]["context"]["conversation_id"], "conversation:test");
        assert_eq!(events[0]["raw"]["provider_text"], "{\"output\":[]}");
    }

    fn provider_with_tokens(base_url: String) -> (CodexResponsesProvider, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            base_url,
            default_model: Some("gpt-test".to_string()),
            client_version: Some("0.144.0".to_string()),
            account_home: Some(account_home.clone()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");
        provider
            .token_store()
            .write(&CodexOAuthTokens {
                access_token: test_access_token(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("write token");
        (provider, dir)
    }

    fn test_access_token() -> String {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "workspace-test"
                },
                "exp": 4_102_444_800_u64
            })
            .to_string(),
        );
        format!("header.{claims}.signature")
    }

    fn sse_delta(delta: &str) -> String {
        format!(
            "event: response.output_text.delta\n\
             data: {}\n\
             \n",
            serde_json::json!({
                "type": "response.output_text.delta",
                "delta": delta,
            })
        )
    }

    fn sse_completed() -> String {
        "event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
        \n"
        .to_string()
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
}
