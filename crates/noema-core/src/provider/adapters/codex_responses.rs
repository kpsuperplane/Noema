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
        ResponsesDiagnosticContext, ResponsesReasoning, ResponsesTool, ResponsesToolNameMap,
        ResponsesTransport, noema_response_text_format, normalize_base_url,
        prompt_cache_key_from_conversation_id, provider_safe_tool_name, responses_tool_choice,
    },
};
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateInput, GenerateInputItem, GenerateMessageRole,
        GenerateOptions, GenerateReasoningInput, GenerateRequest, GenerateResponse,
        GenerateResponseStatus, GenerateStreamEvent, GenerateToolCallInput,
        GenerateToolResultInput, ModelProvider, ParsedNoemaResponse, ProviderError,
        ProviderToolCapabilities, ProviderToolFallbackMode, ProviderToolSchemaDialect,
        output_items_from_text, required_noema_response_from_text_with_native_tool_calls,
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
    /// Optional explicit reasoning effort used only when config supplies an explicit model.
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
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
            reasoning_effort: None,
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

fn codex_encrypted_reasoning_supported() -> bool {
    false
}

#[derive(Debug, Serialize)]
struct CodexResponsesRequest {
    model: String,
    input: Vec<CodexInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ResponsesTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ResponsesReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<String>,
    store: bool,
    stream: bool,
}

#[derive(Debug, Clone, Default)]
struct CodexResponsesToolFields {
    tools: Vec<ResponsesTool>,
    tool_choice: Option<&'static str>,
    parallel_tool_calls: Option<bool>,
}

impl CodexResponsesRequest {
    fn new(
        model: String,
        input: &GenerateInput,
        instructions: Option<String>,
        options: &GenerateOptions,
        reasoning: Option<crate::provider::ReasoningEffort>,
        prompt_cache_key: Option<String>,
        tool_fields: CodexResponsesToolFields,
    ) -> Self {
        Self {
            model,
            input: codex_input_items(input),
            instructions,
            max_output_tokens: options.max_output_tokens,
            temperature: options.temperature,
            text: options
                .require_noema_response
                .then(noema_response_text_format),
            tools: tool_fields.tools,
            tool_choice: tool_fields.tool_choice,
            parallel_tool_calls: tool_fields.parallel_tool_calls,
            reasoning: reasoning.map(|effort| ResponsesReasoning { effort }),
            prompt_cache_key,
            store: false,
            stream: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CodexInputItem {
    Message(CodexInputMessage),
    Reasoning(CodexReasoningItem),
    FunctionCall(CodexFunctionCall),
    FunctionCallOutput(CodexFunctionCallOutput),
}

impl From<&GenerateToolResultInput> for CodexInputItem {
    fn from(value: &GenerateToolResultInput) -> Self {
        Self::FunctionCallOutput(CodexFunctionCallOutput::from(value))
    }
}

impl From<&GenerateInputItem> for CodexInputItem {
    fn from(value: &GenerateInputItem) -> Self {
        match value {
            GenerateInputItem::Message(message) => Self::Message(CodexInputMessage {
                role: match message.role {
                    GenerateMessageRole::User => "user",
                    GenerateMessageRole::Assistant => "assistant",
                },
                content: message.content.clone(),
            }),
            GenerateInputItem::Reasoning(reasoning) => {
                Self::Reasoning(CodexReasoningItem::from(reasoning))
            }
            GenerateInputItem::ToolCall(call) => Self::FunctionCall(CodexFunctionCall::from(call)),
            GenerateInputItem::ToolResult(result) => {
                Self::FunctionCallOutput(CodexFunctionCallOutput::from(result))
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct CodexReasoningItem {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    encrypted_content: String,
}

impl From<&GenerateReasoningInput> for CodexReasoningItem {
    fn from(value: &GenerateReasoningInput) -> Self {
        Self {
            kind: "reasoning",
            id: value.id.clone(),
            encrypted_content: value.encrypted_content.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CodexInputMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct CodexFunctionCall {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    call_id: String,
    name: String,
    arguments: String,
}

impl From<&GenerateToolResultInput> for CodexFunctionCall {
    fn from(value: &GenerateToolResultInput) -> Self {
        Self {
            kind: "function_call",
            id: value.id.clone(),
            call_id: value.call_id.clone(),
            name: value
                .provider_name
                .clone()
                .unwrap_or_else(|| provider_safe_tool_name(&value.name)),
            arguments: value.arguments.to_string(),
        }
    }
}

impl From<&GenerateToolCallInput> for CodexFunctionCall {
    fn from(value: &GenerateToolCallInput) -> Self {
        Self {
            kind: "function_call",
            id: value.id.clone(),
            call_id: value.call_id.clone(),
            name: value
                .provider_name
                .clone()
                .unwrap_or_else(|| provider_safe_tool_name(&value.name)),
            arguments: value.arguments.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CodexFunctionCallOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    call_id: String,
    output: String,
}

impl From<&GenerateToolResultInput> for CodexFunctionCallOutput {
    fn from(value: &GenerateToolResultInput) -> Self {
        Self {
            kind: "function_call_output",
            call_id: value.call_id.clone(),
            output: value.output_json_string(),
        }
    }
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

        let request_model = request
            .model
            .as_ref()
            .filter(|model| !model.trim().is_empty())
            .map(|model| model.trim().to_string());
        let using_config_default_model = request_model.is_none();
        let model = self.model_for_request(request_model)?;
        let reasoning_effort = request.options.reasoning_effort.or_else(|| {
            using_config_default_model
                .then_some(self.config.reasoning_effort)
                .flatten()
        });
        let instructions = request
            .instructions
            .clone()
            .filter(|instructions| !instructions.trim().is_empty());
        let tool_names = ResponsesToolNameMap::from_tools(&request.tools)?;
        let has_tools = !tool_names.tools.is_empty();
        let tool_fields = CodexResponsesToolFields {
            tools: tool_names.tools.clone(),
            tool_choice: responses_tool_choice(request.tool_choice, has_tools),
            parallel_tool_calls: has_tools.then_some(request.parallel_tool_calls),
        };
        let prompt_cache_key =
            prompt_cache_key_from_conversation_id(request.conversation_id.as_deref());
        let body = CodexResponsesRequest::new(
            model.clone(),
            &request.input,
            instructions.clone(),
            &request.options,
            reasoning_effort,
            prompt_cache_key.clone(),
            tool_fields.clone(),
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
                    &request.options,
                    reasoning_effort,
                    prompt_cache_key,
                    tool_fields,
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
                        "codex",
                        response.model.clone().unwrap_or(model),
                        response.id.clone(),
                        response.usage.clone().map(Into::into),
                    )
                    .with_reasoning_items(response.reasoning_items()));
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

        let parsed = if require_noema_response {
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
            "codex",
            response.model.clone().unwrap_or(model),
            response.id.clone(),
            response.usage.clone().map(Into::into),
        )
        .with_reasoning_items(response.reasoning_items()))
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

fn codex_input_items(input: &GenerateInput) -> Vec<CodexInputItem> {
    match input {
        GenerateInput::Text(text) => vec![CodexInputItem::Message(CodexInputMessage {
            role: "user",
            content: text.clone(),
        })],
        GenerateInput::Messages(messages) => messages
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .map(|message| {
                CodexInputItem::Message(CodexInputMessage {
                    role: match message.role {
                        GenerateMessageRole::User => "user",
                        GenerateMessageRole::Assistant => "assistant",
                    },
                    content: message.content.clone(),
                })
            })
            .collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter(|item| !item.is_empty())
            .map(CodexInputItem::from)
            .collect(),
        GenerateInput::NativeToolResults(results) => {
            let mut items = Vec::with_capacity(results.len().saturating_mul(2));
            for result in results {
                items.push(CodexInputItem::FunctionCall(CodexFunctionCall::from(
                    result,
                )));
                items.push(CodexInputItem::FunctionCallOutput(
                    CodexFunctionCallOutput::from(result),
                ));
            }
            items
        }
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
    use crate::provider::adapters::codex_oauth::CodexOAuthTokens;
    use crate::provider::adapters::test_support::spawn_server;
    use crate::provider::{
        GenerateMessage, GenerateMessageRole, GenerateOptions, NoemaToolChoice, NoemaToolExecution,
        NoemaToolSpec, PromptCacheRetention, ProviderToolFallbackMode, ProviderToolSchemaDialect,
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
            captured.headers.get("authorization").map(String::as_str),
            Some("Bearer access")
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
    async fn sends_codex_native_tool_result_input_with_call_context() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Done\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3}}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest {
                input: GenerateInput::NativeToolResults(vec![
                    crate::provider::GenerateToolResultInput {
                        id: Some("item_1".to_string()),
                        call_id: "call_1".to_string(),
                        name: "mcp.docs:read".to_string(),
                        provider_name: Some("mcp_x2e_docs_x3a_read".to_string()),
                        arguments: serde_json::json!({"document_id": "doc_1"}),
                        success: true,
                        payload: serde_json::json!({"title": "Docs"}),
                    },
                ]),
                ..GenerateRequest::text("ignored")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["input"][0]["type"], "function_call");
        assert_eq!(body["input"][0]["id"], "item_1");
        assert_eq!(body["input"][0]["call_id"], "call_1");
        assert_eq!(body["input"][0]["name"], "mcp_x2e_docs_x3a_read");
        assert_eq!(body["input"][1]["type"], "function_call_output");
        assert_eq!(body["input"][1]["call_id"], "call_1");
        assert_eq!(response.assistant_text(), "Done");
    }

    #[tokio::test]
    async fn sends_codex_typed_history_items_as_native_response_items() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Done\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        provider
            .generate(GenerateRequest {
                input: GenerateInput::Items(vec![
                    crate::provider::GenerateInputItem::Message(crate::GenerateMessage {
                        role: crate::GenerateMessageRole::User,
                        content: "Rename yourself to Momo".to_string(),
                    }),
                    crate::provider::GenerateInputItem::ToolCall(
                        crate::provider::GenerateToolCallInput {
                            id: Some("item_1".to_string()),
                            call_id: "call_1".to_string(),
                            name: "update_own_name".to_string(),
                            provider_name: None,
                            arguments: serde_json::json!({"name": "Momo"}),
                        },
                    ),
                    crate::provider::GenerateInputItem::ToolResult(
                        crate::provider::GenerateToolResultInput {
                            id: Some("item_1".to_string()),
                            call_id: "call_1".to_string(),
                            name: "update_own_name".to_string(),
                            provider_name: None,
                            arguments: Value::Null,
                            success: true,
                            payload: serde_json::json!({"display_name": "Momo"}),
                        },
                    ),
                ]),
                ..GenerateRequest::text("ignored")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][1]["type"], "function_call");
        assert_eq!(body["input"][1]["call_id"], "call_1");
        assert_eq!(body["input"][1]["name"], "update_own_name");
        assert_eq!(body["input"][2]["type"], "function_call_output");
        assert_eq!(body["input"][2]["call_id"], "call_1");
        assert!(
            body["input"][2]["output"]
                .as_str()
                .is_some_and(|output| output.contains("Momo"))
        );
    }

    #[tokio::test]
    async fn sends_codex_typed_history_with_provider_safe_fallback_names() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Done\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        provider
            .generate(GenerateRequest {
                input: GenerateInput::Items(vec![crate::provider::GenerateInputItem::ToolCall(
                    crate::provider::GenerateToolCallInput {
                        id: None,
                        call_id: "call_1".to_string(),
                        name: "mcp.dex:search contacts".to_string(),
                        provider_name: None,
                        arguments: serde_json::json!({"query": "Gautam"}),
                    },
                )]),
                ..GenerateRequest::text("ignored")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["input"][0]["type"], "function_call");
        assert_eq!(
            body["input"][0]["name"],
            "mcp_x2e_dex_x3a_search_x20_contacts"
        );
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
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"response_status\\\":\\\"needs_tools\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"commentary\\\",\\\"text\\\":\\\"Checking.\\\"}],\\\"tool_calls\\\":[],\\\"memory_proposals\\\":[]}\"}\n\
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
    async fn omits_codex_prompt_cache_retention_even_when_requested() {
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
    async fn sends_codex_prompt_cache_key_for_conversation_requests() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        let response = provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:cacheable".to_string()),
                ..GenerateRequest::text("Hello?")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["prompt_cache_key"], "conversation:cacheable");
        assert_eq!(response.assistant_text(), "Hello");
    }

    #[tokio::test]
    async fn codex_omits_encrypted_reasoning_include_until_verified() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:codex-reasoning".to_string()),
                ..GenerateRequest::text("Hello?")
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert!(body.get("include").is_none());
    }

    #[tokio::test]
    async fn codex_sends_reasoning_effort_when_configured() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"response_status\\\":\\\"final\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"final_answer\\\",\\\"text\\\":\\\"Hello\\\"}],\\\"tool_calls\\\":[],\\\"memory_proposals\\\":[]}\"}]}]}}\n\
             \n",
        )
        .await;
        let (provider, _dir) = provider_with_tokens(base_url);

        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:test".to_string()),
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Hello?".to_string()),
                instructions: Some("Reply in contract.".to_string()),
                options: GenerateOptions {
                    require_noema_response: true,
                    reasoning_effort: Some(crate::provider::ReasoningEffort::High),
                    ..GenerateOptions::default()
                },
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["reasoning"]["effort"], "high");
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
