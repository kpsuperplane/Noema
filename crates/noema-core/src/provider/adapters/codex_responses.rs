//! Provider adapter for Codex direct Responses API calls.

use std::{path::PathBuf, time::Duration};

use reqwest::header::HeaderMap;
use serde::Serialize;
use serde_json::Value;

use super::{
    codex_oauth::{
        CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthClient, CodexOAuthConfig,
        CodexTokenStore, DEFAULT_CODEX_BASE_URL,
    },
    noema_response_stream::NoemaAssistantTextDeltaExtractor,
    responses::{
        ResponsesDiagnosticContext, ResponsesTransport, noema_response_text_format,
        normalize_base_url,
    },
};
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateInput, GenerateMessageRole, GenerateRequest,
        GenerateResponse, GenerateResponseStatus, GenerateStreamEvent, ModelProvider,
        ParsedNoemaResponse, ProviderError, output_items_from_text,
        required_noema_response_from_text,
    },
};

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
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
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
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
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
        let transport = ResponsesTransport::new(client, config.base_url.clone())?;
        let token_store = CodexTokenStore::new(account_home);
        let oauth_client = CodexOAuthClient::new(config.oauth.clone())?;
        Ok(Self {
            transport,
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
    Ok(config)
}

#[derive(Debug, Serialize)]
struct CodexResponsesRequest {
    model: String,
    input: Vec<CodexInputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<serde_json::Value>,
    store: bool,
    stream: bool,
}

impl CodexResponsesRequest {
    fn new(
        model: String,
        input: &GenerateInput,
        instructions: Option<String>,
        max_output_tokens: Option<u32>,
        temperature: Option<f32>,
        require_noema_response: bool,
    ) -> Self {
        Self {
            model,
            input: codex_input_messages(input),
            instructions,
            max_output_tokens,
            temperature,
            text: require_noema_response.then(noema_response_text_format),
            store: false,
            stream: true,
        }
    }
}

#[derive(Debug, Serialize)]
struct CodexInputMessage {
    role: &'static str,
    content: String,
}

impl CodexResponsesProvider {
    async fn generate_with_events(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let require_noema_response = request.options.require_noema_response;
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let model = self.model_for_request(request.model)?;
        let instructions = request
            .instructions
            .clone()
            .filter(|instructions| !instructions.trim().is_empty());
        let max_output_tokens = request.options.max_output_tokens;
        let temperature = request.options.temperature;
        let body = CodexResponsesRequest::new(
            model.clone(),
            &request.input,
            instructions.clone(),
            max_output_tokens,
            temperature,
            require_noema_response,
        );
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
        let mut noema_delta_extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut forward_event = |event| {
            if require_noema_response {
                if let GenerateStreamEvent::AssistantTextDelta { delta } = event {
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
                body,
                HeaderMap::new(),
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
                let retry_body = CodexResponsesRequest::new(
                    model.clone(),
                    &request.input,
                    instructions,
                    max_output_tokens,
                    temperature,
                    require_noema_response,
                );
                self.transport
                    .send_streaming(
                        &refreshed,
                        retry_body,
                        HeaderMap::new(),
                        diagnostics,
                        &mut forward_event,
                    )
                    .await?
            }
            Err(error) => return Err(error),
        };
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

        let parsed = if require_noema_response {
            match required_noema_response_from_text(text) {
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
                tool_calls: Vec::new(),
                memory_proposals: Vec::new(),
                response_status: GenerateResponseStatus::Final,
            }
        };

        Ok(GenerateResponse::from_parsed(
            parsed,
            "codex",
            response.model.unwrap_or(model),
            response.id,
            response.usage.map(Into::into),
        ))
    }

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
                    "provider_kind": "codex",
                    "model": model,
                    "conversation_id": conversation_id,
                    "request_id": request_id,
                }))
                .with_error_chain([error.to_string()])
                .with_raw(raw),
        );
    }
}

fn codex_input_messages(input: &GenerateInput) -> Vec<CodexInputMessage> {
    match input {
        GenerateInput::Text(text) => vec![CodexInputMessage {
            role: "user",
            content: text.clone(),
        }],
        GenerateInput::Messages(messages) => messages
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .map(|message| CodexInputMessage {
                role: match message.role {
                    GenerateMessageRole::User => "user",
                    GenerateMessageRole::Assistant => "assistant",
                },
                content: message.content.clone(),
            })
            .collect(),
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
    use crate::provider::adapters::codex_oauth::CodexOAuthTokens;
    use crate::provider::adapters::test_support::spawn_server;
    use crate::provider::{
        GenerateMessage, GenerateMessageRole, GenerateOptions, PromptCacheRetention,
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
            captured.headers.get("authorization").map(String::as_str),
            Some("Bearer access")
        );
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "Hello?");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);

        assert_eq!(response.assistant_text(), "Hello");
    }

    #[tokio::test]
    async fn sends_codex_transcript_messages_as_response_input_items_without_cache_retention() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello again\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest {
                input: GenerateInput::Messages(vec![
                    GenerateMessage {
                        role: GenerateMessageRole::User,
                        content: "first durable question".to_string(),
                    },
                    GenerateMessage {
                        role: GenerateMessageRole::Assistant,
                        content: "first durable answer".to_string(),
                    },
                    GenerateMessage {
                        role: GenerateMessageRole::User,
                        content: "second durable question".to_string(),
                    },
                ]),
                options: GenerateOptions {
                    prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                    ..GenerateOptions::default()
                },
                ..GenerateRequest::text("unused")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "first durable question");
        assert_eq!(body["input"][1]["role"], "assistant");
        assert_eq!(body["input"][1]["content"], "first durable answer");
        assert_eq!(body["input"][2]["role"], "user");
        assert_eq!(body["input"][2]["content"], "second durable question");
        assert!(body.get("prompt_cache_retention").is_none());

        assert_eq!(response.assistant_text(), "Hello again");
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
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
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
            sse_delta(r#"lo"}],"tool_calls":[],"memory_proposals":[]}"#),
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
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
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
        (provider, dir)
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
}
