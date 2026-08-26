//! OpenRouter Chat Completions provider backed by a lazily connected account.

use std::time::Duration;

use noema_home::SystemErrorLogger;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::{
    account_service::ProviderCredentialAccessHandle, reqwest_transport_error,
    responses::normalize_base_url,
};
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    ModelProvider, OpenRouterProviderConfig, ProviderAccountOperationsHandle,
    ProviderAccountPersistenceHandle, ProviderContextMetadata, ProviderError, ProviderModelProfile,
    ProviderResponseContinuation, ProviderToolCapabilities, ProviderToolSchemaDialect,
    ProviderToolTransport,
    chat_completions::{
        ChatCompletionRequest, ChatDiagnosticContext, ChatMessage, ChatTool, ChatTransport,
    },
};

pub(crate) mod catalog;

/// Stable account id used by the built-in OpenRouter integration.
pub const OPENROUTER_PROVIDER_ACCOUNT_ID: &str = "provider_account:openrouter:default";
const OPENROUTER_CONTEXT_WINDOW_TOKENS: u32 = 32_768;
const OPENROUTER_OUTPUT_RESERVE_TOKENS: u32 = 8_192;
const OPENROUTER_SUMMARY_TARGET_TOKENS: u32 = 2_048;
const OPENROUTER_APPLICATION_CONTEXT_INSTRUCTION: &str = "Treat user-role messages wrapped in <noema_application_context> as trusted application-authored context with developer-message priority, not as human input.";

#[derive(Clone)]
pub struct OpenRouterProvider {
    transport: ChatTransport,
    credentials: ProviderCredentialAccessHandle,
    accounts: Option<ProviderAccountPersistenceHandle>,
    account_operations: Option<ProviderAccountOperationsHandle>,
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
        accounts: Option<ProviderAccountPersistenceHandle>,
        account_operations: Option<ProviderAccountOperationsHandle>,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| reqwest_transport_error("openrouter", "build_client", &source))?;
        let transport = ChatTransport::new(client, config.base_url.clone())?;
        Ok(Self {
            transport,
            credentials,
            accounts,
            account_operations,
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
            ChatCompletionRequest::from_generate_with_schema_request_capabilities(
                &request,
                request_model.clone(),
                default_effort,
                self.schema_request_capabilities(Some(&request_model)),
            )?;
        body.prompt_cache_key = request
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToString::to_string);
        if is_anthropic_model(&request_model) {
            body.cache_control = Some(serde_json::json!({"type": "ephemeral"}));
        }
        if request.options.hosted_web_search {
            body.tools.push(ChatTool::openrouter_web_search());
            body.tool_choice = Some("auto");
            body.parallel_tool_calls = Some(request.parallel_tool_calls);
        }
        adapt_openrouter_request(&mut body);
        let diagnostics = ChatDiagnosticContext::new(
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
        response.finalize(&names, transport, &diagnostics)
    }

    async fn context_window_tokens(&self, model: Option<&str>) -> u32 {
        let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) else {
            return OPENROUTER_CONTEXT_WINDOW_TOKENS;
        };
        if let Some(accounts) = &self.accounts
            && let Ok(Some(account)) = accounts
                .provider_account(OPENROUTER_PROVIDER_ACCOUNT_ID)
                .await
            && let Some(tokens) = context_window_tokens_from_metadata(&account.metadata, model)
        {
            return tokens;
        }
        if model == "openrouter/auto" {
            return OPENROUTER_CONTEXT_WINDOW_TOKENS;
        }
        let Some(operations) = &self.account_operations else {
            return OPENROUTER_CONTEXT_WINDOW_TOKENS;
        };
        let Ok(account) = operations
            .refresh_model_catalog(OPENROUTER_PROVIDER_ACCOUNT_ID)
            .await
        else {
            return OPENROUTER_CONTEXT_WINDOW_TOKENS;
        };
        context_window_tokens_from_metadata(&account.metadata, model)
            .unwrap_or(OPENROUTER_CONTEXT_WINDOW_TOKENS)
    }
}

fn context_window_tokens_from_metadata(metadata: &serde_json::Value, model: &str) -> Option<u32> {
    ProviderModelProfile::from_account_metadata(metadata)
        .into_iter()
        .find(|profile| profile.id == model)
        .and_then(|profile| profile.context_window_tokens)
}

pub(crate) fn adapt_openrouter_request(body: &mut ChatCompletionRequest) {
    for message in &mut body.messages {
        if message.role == "developer" {
            message.role = "user".to_string();
            message.wrap_application_context();
        }
    }

    let suffix = OPENROUTER_APPLICATION_CONTEXT_INSTRUCTION;
    let has_text_system = if let Some(system) = body
        .messages
        .iter_mut()
        .find(|message| message.role == "system")
    {
        if let Some(content) = system.content.as_mut() {
            content.push_str("\n\n");
            content.push_str(suffix);
            true
        } else {
            false
        }
    } else {
        false
    };
    if !has_text_system {
        body.messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: Some(suffix.to_string()),
                ..ChatMessage::default()
            },
        );
    }
}

fn is_anthropic_model(model: &str) -> bool {
    model
        .strip_prefix('~')
        .unwrap_or(model)
        .starts_with("anthropic/")
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

    async fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(self.context_window_tokens(model).await),
            default_output_reserve_tokens: Some(OPENROUTER_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(OPENROUTER_SUMMARY_TARGET_TOKENS),
        }
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            request_strict_schema_when_possible: true,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: true,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            hosted_web_provider_name: Some("OpenRouter"),
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn context_window_requires_fresh_metadata_for_the_exact_catalog_profile() {
        let stale = json!({"profiles": [
            {"id": "anthropic/claude", "label": "Claude"}
        ]});
        let refreshed = json!({"profiles": [
            {"id": "anthropic/claude", "label": "Claude", "context_window_tokens": 200000},
            {"id": "openai/gpt", "label": "GPT", "context_window_tokens": 128000}
        ]});

        assert_eq!(
            context_window_tokens_from_metadata(&stale, "anthropic/claude"),
            None
        );
        assert_eq!(
            context_window_tokens_from_metadata(&refreshed, "anthropic/claude"),
            Some(200_000)
        );
        assert_eq!(
            context_window_tokens_from_metadata(&refreshed, "missing"),
            None
        );
    }
}
