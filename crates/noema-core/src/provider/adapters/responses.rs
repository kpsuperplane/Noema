//! Shared transport and parser for OpenAI-compatible Responses API calls.

use super::sse::SseAccumulator;
use crate::provider::{
    GenerateRequest, GenerateStreamEvent, PromptCacheOptions, PromptCacheRetention, ProviderError,
    ReasoningEffort, SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
};
use futures_util::StreamExt;
use noema_home::{SystemErrorEvent, SystemErrorLogger};
use reqwest::{
    StatusCode,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON request body sent to a Responses-compatible endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesRequest {
    /// Model identifier to use for the response.
    pub model: String,
    /// User-visible input.
    pub input: ResponsesInput,
    /// Optional system/developer instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Opaque id of the response whose provider-side context should be reused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    /// Optional maximum output token budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Optional Responses text controls such as JSON schema output format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Value>,
    /// Optional explicit reasoning controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponsesReasoning>,
    /// Native Responses API tool definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ResponsesTool>,
    /// Responses API tool-choice policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ResponsesToolChoice>,
    /// Whether parallel independent tool calls are allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Additional provider output fields to include in responses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<&'static str>,
    /// Provider prompt-cache key used to bind reusable prefixes to a conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    /// Request-wide prompt-cache controls when supported by the profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_options: Option<PromptCacheOptions>,
    /// Whether the upstream should store this response.
    pub store: bool,
    /// Provider prompt-cache retention request when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<PromptCacheRetention>,
    /// Whether the provider should return an SSE stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Provider-specific wire capabilities for a shared Responses request.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResponsesRequestProfile {
    input_shape: ResponsesInputShape,
    forward_max_output_tokens: bool,
    forward_prompt_cache_retention: bool,
    forward_prompt_cache_options: bool,
    forward_prompt_cache_breakpoints: bool,
    allowed_tools: bool,
    include_encrypted_reasoning: bool,
    stream: bool,
}

pub(crate) const OPENAI_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::String,
    forward_max_output_tokens: true,
    forward_prompt_cache_retention: true,
    forward_prompt_cache_options: true,
    forward_prompt_cache_breakpoints: true,
    allowed_tools: true,
    include_encrypted_reasoning: true,
    stream: false,
};

pub(crate) const CODEX_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::MessageArray,
    forward_max_output_tokens: false,
    forward_prompt_cache_retention: false,
    forward_prompt_cache_options: false,
    forward_prompt_cache_breakpoints: false,
    allowed_tools: false,
    include_encrypted_reasoning: false,
    stream: true,
};

impl ResponsesRequest {
    /// Lower one provider-neutral request according to a Responses wire profile.
    pub(crate) fn from_generate(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        profile: ResponsesRequestProfile,
    ) -> Result<(Self, ResponsesToolNameMap), ProviderError> {
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let tool_names = ResponsesToolNameMap::from_tools(&request.tools)?;
        let has_tools = !tool_names.tools.is_empty();
        let body = Self {
            model,
            input: ResponsesInput::from_generate(
                &request.input,
                profile.input_shape,
                request.options.previous_response_id.is_some(),
                if profile.forward_prompt_cache_breakpoints {
                    &request.options.prompt_cache_breakpoints
                } else {
                    &[]
                },
            )?,
            instructions: request
                .instructions
                .as_deref()
                .filter(|instructions| !instructions.trim().is_empty())
                .map(ToString::to_string),
            previous_response_id: request.options.previous_response_id.clone(),
            max_output_tokens: profile
                .forward_max_output_tokens
                .then_some(request.options.max_output_tokens)
                .flatten(),
            temperature: request.options.temperature,
            text: request
                .options
                .require_noema_response
                .then(noema_response_text_format),
            reasoning: request
                .options
                .reasoning_effort
                .or(default_reasoning_effort)
                .map(|effort| ResponsesReasoning { effort }),
            tools: tool_names.tools.clone(),
            tool_choice: responses_tool_choice(
                &request.tool_choice,
                &tool_names,
                profile.allowed_tools,
            )?,
            parallel_tool_calls: has_tools.then_some(request.parallel_tool_calls),
            include: if profile.include_encrypted_reasoning {
                vec!["reasoning.encrypted_content"]
            } else {
                Vec::new()
            },
            prompt_cache_key: prompt_cache_key_from_conversation_id(
                request.conversation_id.as_deref(),
            ),
            prompt_cache_options: profile
                .forward_prompt_cache_options
                .then_some(request.options.prompt_cache_options)
                .flatten(),
            store: request.options.store_response,
            prompt_cache_retention: profile
                .forward_prompt_cache_retention
                .then_some(request.options.prompt_cache_retention)
                .flatten(),
            stream: profile.stream.then_some(true),
        };
        Ok((body, tool_names))
    }
}

/// Responses API reasoning controls.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ResponsesReasoning {
    /// Reasoning effort requested from the provider.
    pub effort: ReasoningEffort,
}

pub(crate) fn prompt_cache_key_from_conversation_id(
    conversation_id: Option<&str>,
) -> Option<String> {
    let conversation_id = conversation_id?.trim();
    (!conversation_id.is_empty()).then(|| conversation_id.to_string())
}

pub(super) use super::responses_format::noema_response_text_format;
pub use super::responses_input::*;
pub use super::responses_output::{ResponsesResponse, ResponsesUsage};
pub use super::responses_tools::{
    ResponsesAllowedTool, ResponsesAllowedTools, ResponsesTool, ResponsesToolChoice,
};
pub(crate) use super::responses_tools::{ResponsesToolNameMap, responses_tool_choice};
/// HTTP transport for a Responses-compatible endpoint.
#[derive(Debug, Clone)]
pub struct ResponsesTransport {
    client: reqwest::Client,
    responses_url: String,
}

/// Diagnostic context for Responses-compatible provider calls.
#[derive(Debug, Clone)]
pub struct ResponsesDiagnosticContext {
    /// Developer diagnostic logger.
    pub logger: Option<SystemErrorLogger>,
    /// Provider kind.
    pub provider_kind: String,
    /// Model requested by Noema.
    pub model: String,
    /// Noema conversation id, when available.
    pub conversation_id: Option<String>,
}

impl ResponsesDiagnosticContext {
    /// Build a provider diagnostic context.
    #[must_use]
    pub fn new(
        logger: Option<SystemErrorLogger>,
        provider_kind: impl Into<String>,
        model: impl Into<String>,
        conversation_id: Option<String>,
    ) -> Self {
        Self {
            logger,
            provider_kind: provider_kind.into(),
            model: model.into(),
            conversation_id,
        }
    }

    pub(crate) fn context_json(&self, request_id: Option<&str>) -> Value {
        serde_json::json!({
            "provider_kind": self.provider_kind,
            "model": self.model,
            "conversation_id": self.conversation_id,
            "request_id": request_id,
        })
    }

    pub(crate) fn log_malformed(&self, message: impl Into<String>, raw: Value) {
        if let Some(logger) = &self.logger {
            let message = message.into();
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
                    .with_context(self.context_json(None))
                    .with_error_chain([message])
                    .with_raw(raw),
            );
        }
    }

    pub(crate) fn log_malformed_error(
        &self,
        error: &ProviderError,
        request_id: Option<&str>,
        raw: Value,
    ) {
        if let Some(logger) = &self.logger {
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, error.to_string())
                    .with_context(self.context_json(request_id))
                    .with_error_chain([error.to_string()])
                    .with_raw(raw),
            );
        }
    }
}

impl ResponsesTransport {
    /// Build a transport from a reqwest client and base API URL.
    ///
    /// The base URL should not include `/responses`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when the base URL is empty or
    /// not absolute.
    pub fn new(
        client: reqwest::Client,
        base_url: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let base_url = normalize_base_url(base_url.into(), "responses base URL")?;
        Ok(Self {
            client,
            responses_url: format!("{base_url}/responses"),
        })
    }

    /// Send one Responses request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] for HTTP transport failures, API errors, or
    /// malformed JSON responses.
    pub async fn send<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
        diagnostics: ResponsesDiagnosticContext,
    ) -> Result<ResponsesResponse, ProviderError>
    where
        T: Serialize,
    {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "responses".to_string(),
                credential: "bearer_token".to_string(),
            });
        }

        let mut builder = self
            .client
            .post(&self.responses_url)
            .bearer_auth(bearer_token);
        for (name, value) in &extra_headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());
        let body_text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;

        if !status.is_success() {
            return Err(error_from_status(status, request_id, &body_text));
        }

        let value = serde_json::from_str::<Value>(&body_text).map_err(|source| {
            let message = format!("failed to parse JSON: {source}");
            diagnostics.log_malformed(
                message.clone(),
                serde_json::json!({
                    "http_status": status.as_u16(),
                    "body_text": body_text,
                }),
            );
            ProviderError::MalformedResponse { message }
        })?;
        let mut response =
            serde_json::from_value::<ResponsesResponse>(value.clone()).map_err(|source| {
                let message = format!("failed to parse JSON: {source}");
                diagnostics.log_malformed(
                    message.clone(),
                    serde_json::json!({
                        "http_status": status.as_u16(),
                        "body_text": body_text,
                    }),
                );
                ProviderError::MalformedResponse { message }
            })?;
        response.raw = Some(value);
        Ok(response)
    }

    /// Send one streaming Responses request, emitting incremental assistant text
    /// events as SSE chunks arrive, and collect the terminal response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] for HTTP transport failures, API errors, or
    /// malformed Server-Sent Events.
    pub async fn send_streaming<T>(
        &self,
        bearer_token: &str,
        body: T,
        extra_headers: HeaderMap,
        diagnostics: ResponsesDiagnosticContext,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ResponsesResponse, ProviderError>
    where
        T: Serialize,
    {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "responses".to_string(),
                credential: "bearer_token".to_string(),
            });
        }

        let mut builder = self
            .client
            .post(&self.responses_url)
            .bearer_auth(bearer_token);
        for (name, value) in &extra_headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());

        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .map_err(|source| ProviderError::HttpFailure { source })?;
            return Err(error_from_status(status, request_id, &body_text));
        }

        let mut accumulator = SseAccumulator::new(diagnostics);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| ProviderError::HttpFailure { source })?;
            accumulator.push_bytes(&chunk, on_event)?;
        }

        accumulator.finish(on_event)
    }
}

/// Normalize and validate a Responses-compatible base URL.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidRequest`] when the URL is empty or invalid.
pub fn normalize_base_url(value: String, label: &str) -> Result<String, ProviderError> {
    let base_url = value.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} cannot be empty"),
        });
    }

    if reqwest::Url::parse(&base_url).is_err() {
        return Err(ProviderError::InvalidRequest {
            message: format!("{label} must be an absolute URL"),
        });
    }

    Ok(base_url)
}

/// Convert a non-empty string to an HTTP header value.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidRequest`] when the value is not a legal
/// header value.
pub fn header_value(value: &str, label: &str) -> Result<HeaderValue, ProviderError> {
    HeaderValue::from_str(value).map_err(|_| ProviderError::InvalidRequest {
        message: format!("{label} contains invalid header characters"),
    })
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorResponse {
    error: Option<ResponsesErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorBody {
    message: Option<String>,
}

fn error_from_status(
    status: StatusCode,
    request_id: Option<String>,
    body_text: &str,
) -> ProviderError {
    let message = serde_json::from_str::<ResponsesErrorResponse>(body_text)
        .ok()
        .and_then(|body| body.error)
        .and_then(|error| error.message)
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| body_text.trim().to_string())
        .if_empty_then(|| status.canonical_reason().unwrap_or("API error").to_string());

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthenticationFailure {
            message,
            request_id,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimit {
            message,
            request_id,
        },
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id,
        },
    }
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

trait EmptyStringExt {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.is_empty() { fallback() } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        GenerateInput, GenerateInputItem, GenerateReasoningInput, NoemaAllowedTools,
        NoemaAllowedToolsMode, NoemaToolChoice,
    };

    #[test]
    fn responses_request_profiles_preserve_provider_wire_differences() {
        let request = GenerateRequest {
            conversation_id: Some(" conversation:cacheable ".to_string()),
            instructions: Some("Be brief.".to_string()),
            options: crate::provider::GenerateOptions {
                max_output_tokens: Some(32),
                prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![test_tool()],
            tool_choice: crate::provider::NoemaToolChoice::Required,
            parallel_tool_calls: true,
            ..GenerateRequest::text("hi")
        };

        let (openai, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-openai".to_string(),
            Some(ReasoningEffort::Low),
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("OpenAI request");
        let (codex, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-codex".to_string(),
            Some(ReasoningEffort::High),
            CODEX_RESPONSES_PROFILE,
        )
        .expect("Codex request");
        let openai = serde_json::to_value(openai).expect("OpenAI JSON");
        let codex = serde_json::to_value(codex).expect("Codex JSON");

        assert_eq!(openai["input"], "hi");
        assert_eq!(openai["max_output_tokens"], 32);
        assert_eq!(openai["prompt_cache_retention"], "24h");
        assert_eq!(openai["include"][0], "reasoning.encrypted_content");
        assert!(openai.get("stream").is_none());
        assert_eq!(openai["reasoning"]["effort"], "low");

        assert_eq!(codex["input"][0]["role"], "user");
        assert_eq!(codex["input"][0]["content"], "hi");
        assert!(codex.get("max_output_tokens").is_none());
        assert!(codex.get("prompt_cache_retention").is_none());
        assert!(codex.get("include").is_none());
        assert_eq!(codex["stream"], true);
        assert_eq!(codex["reasoning"]["effort"], "high");

        for value in [&openai, &codex] {
            assert_eq!(value["instructions"], "Be brief.");
            assert_eq!(value["text"]["format"]["name"], "noema_response");
            assert_eq!(value["tools"][0]["name"], "search_memory");
            assert_eq!(value["tool_choice"], "required");
            assert_eq!(value["parallel_tool_calls"], true);
            assert_eq!(value["prompt_cache_key"], "conversation:cacheable");
            assert_eq!(value["store"], false);
        }
    }

    #[test]
    fn openai_profile_serializes_allowed_tools_with_provider_safe_names() {
        let request = GenerateRequest {
            tools: vec![test_tool(), test_tool_named("mcp.docs:read")],
            tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
                mode: NoemaAllowedToolsMode::Required,
                tools: vec![noema_capabilities::ToolName::new("mcp.docs:read").expect("tool name")],
            }),
            ..GenerateRequest::text("hi")
        };

        let (body, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-openai".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("allowed tools request");
        let value = serde_json::to_value(body).expect("OpenAI JSON");

        assert_eq!(value["tools"].as_array().map(Vec::len), Some(2));
        assert_eq!(value["tool_choice"]["type"], "allowed_tools");
        assert_eq!(value["tool_choice"]["mode"], "required");
        assert_eq!(value["tool_choice"]["tools"][0]["type"], "function");
        assert_eq!(
            value["tool_choice"]["tools"][0]["name"],
            "mcp_x2e_docs_x3a_read"
        );
    }

    #[test]
    fn allowed_tools_are_profile_gated_and_must_reference_the_catalog() {
        let mut request = GenerateRequest {
            tools: vec![test_tool()],
            tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
                mode: NoemaAllowedToolsMode::Auto,
                tools: vec![noema_capabilities::ToolName::new("search_memory").expect("tool name")],
            }),
            ..GenerateRequest::text("hi")
        };

        let unsupported = ResponsesRequest::from_generate(
            &request,
            "gpt-codex".to_string(),
            None,
            CODEX_RESPONSES_PROFILE,
        )
        .expect_err("Codex profile rejects allowed tools");
        assert!(
            unsupported
                .to_string()
                .contains("not supported by this provider request profile")
        );

        request.tool_choice = NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Auto,
            tools: vec![noema_capabilities::ToolName::new("update_own_name").expect("tool name")],
        });
        let missing = ResponsesRequest::from_generate(
            &request,
            "gpt-openai".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect_err("unknown allowed tool rejected");
        assert!(
            missing
                .to_string()
                .contains("not present in the request tool catalog")
        );
    }

    #[test]
    fn openai_profile_serializes_cache_options_and_developer_message_breakpoints() {
        let request = GenerateRequest {
            input: GenerateInput::Messages(vec![
                crate::GenerateMessage {
                    role: crate::provider::contract::GenerateMessageRole::System,
                    content: "   ".to_string(),
                },
                crate::GenerateMessage {
                    role: crate::provider::contract::GenerateMessageRole::Developer,
                    content: "Environment revision 8".to_string(),
                },
                crate::GenerateMessage {
                    role: crate::provider::contract::GenerateMessageRole::User,
                    content: "What changed?".to_string(),
                },
            ]),
            options: crate::provider::GenerateOptions {
                prompt_cache_options: Some(crate::provider::contract::PromptCacheOptions {
                    mode: crate::provider::contract::PromptCacheMode::Explicit,
                    ttl: crate::provider::contract::PromptCacheTtl::ThirtyMinutes,
                }),
                prompt_cache_breakpoints: vec![0],
                ..crate::provider::GenerateOptions::default()
            },
            ..GenerateRequest::text("unused")
        };

        let (openai, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-openai".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("OpenAI request");
        let (codex, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-codex".to_string(),
            None,
            CODEX_RESPONSES_PROFILE,
        )
        .expect("Codex request");
        let openai = serde_json::to_value(openai).expect("OpenAI JSON");
        let codex = serde_json::to_value(codex).expect("Codex JSON");

        assert_eq!(openai["prompt_cache_options"]["mode"], "explicit");
        assert_eq!(openai["prompt_cache_options"]["ttl"], "30m");
        assert_eq!(openai["input"][0]["role"], "developer");
        assert_eq!(openai["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(
            openai["input"][0]["content"][0]["prompt_cache_breakpoint"]["mode"],
            "explicit"
        );
        assert_eq!(openai["input"][1]["content"], "What changed?");

        assert!(codex.get("prompt_cache_options").is_none());
        assert_eq!(codex["input"][0]["role"], "developer");
        assert_eq!(codex["input"][0]["content"], "Environment revision 8");
    }

    #[test]
    fn prompt_cache_breakpoints_reject_invalid_filtered_message_indices() {
        let request = GenerateRequest {
            input: GenerateInput::Messages(vec![crate::GenerateMessage {
                role: crate::provider::contract::GenerateMessageRole::Developer,
                content: "Environment revision 8".to_string(),
            }]),
            options: crate::provider::GenerateOptions {
                prompt_cache_breakpoints: vec![1],
                ..crate::provider::GenerateOptions::default()
            },
            ..GenerateRequest::text("unused")
        };

        let error = ResponsesRequest::from_generate(
            &request,
            "gpt-openai".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect_err("out-of-range breakpoint rejected");
        assert!(
            error
                .to_string()
                .contains("out of range for 1 filtered messages")
        );
    }

    #[test]
    fn responses_request_reasoning_precedence_and_input_validation_are_shared() {
        let mut request = GenerateRequest::text("hi");
        request.options.reasoning_effort = Some(ReasoningEffort::Medium);
        let (body, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-test".to_string(),
            Some(ReasoningEffort::Low),
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("request reasoning wins");
        assert!(matches!(
            body.reasoning,
            Some(ResponsesReasoning {
                effort: ReasoningEffort::Medium
            })
        ));

        let error = ResponsesRequest::from_generate(
            &GenerateRequest::text(""),
            "gpt-test".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect_err("empty input rejected");
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    fn test_tool() -> noema_capabilities::ToolSpec {
        test_tool_named("search_memory")
    }

    fn test_tool_named(name: &str) -> noema_capabilities::ToolSpec {
        noema_capabilities::ToolSpec::new(
            name,
            "Search governed Noema memory.",
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false
            }),
        )
        .expect("tool")
    }

    #[test]
    fn whole_capability_catalog_lowering_fixture_is_stable() {
        let tools = vec![
            noema_capabilities::web::search::tool_spec().expect("search spec"),
            noema_capabilities::web::fetch::tool_spec().expect("fetch spec"),
            noema_capabilities::ToolSpec::new(
                "mcp.mcp:docs.read",
                "Read a document.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
            )
            .expect("MCP spec"),
        ];
        let request = GenerateRequest {
            tools,
            tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
                mode: NoemaAllowedToolsMode::Required,
                tools: vec![
                    noema_capabilities::ToolName::new("mcp.mcp:docs.read").expect("tool name"),
                ],
            }),
            parallel_tool_calls: true,
            ..GenerateRequest::text("fixture input")
        };
        let (body, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-fixture".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("lowering");

        let expected_tools = serde_json::json!([
            {
                "type": "function",
                "name": "web_x2e_search",
                "description": "Search the public web using Noema's configured search provider.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "minLength": 1, "maxLength": 500, "description": "The exact internet search query to send to the configured search provider."},
                        "reason": {"type": "string", "maxLength": 500, "description": "Brief reason this search is useful for the current response."},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": 10, "description": "Maximum number of search results to return."}
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            },
            {
                "type": "function",
                "name": "web_x2e_fetch",
                "description": "Fetch and read a public web page using Noema's configured web fetch provider.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {"type": "string", "minLength": 1, "maxLength": 2048, "description": "The public http(s) URL to fetch and read."},
                        "reason": {"type": "string", "maxLength": 500, "description": "Brief reason this page is useful for the current response."},
                        "max_chars": {"type": "integer", "minimum": 1000, "maximum": 20000, "description": "Maximum characters to return after extraction and optional summarization."}
                    },
                    "required": ["url"],
                    "additionalProperties": false
                }
            },
            {
                "type": "function",
                "name": "mcp_x2e_mcp_x3a_docs_x2e_read",
                "description": "Read a document.",
                "parameters": {
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }
            }
        ]);
        assert_eq!(
            serde_json::to_value(body).expect("serialize"),
            serde_json::json!({
                "model": "gpt-fixture",
                "input": "fixture input",
                "tools": expected_tools,
                "tool_choice": {
                    "type": "allowed_tools",
                    "mode": "required",
                    "tools": [{
                        "type": "function",
                        "name": "mcp_x2e_mcp_x3a_docs_x2e_read"
                    }]
                },
                "parallel_tool_calls": true,
                "include": ["reasoning.encrypted_content"],
                "store": false
            })
        );
    }

    #[test]
    fn valid_unknown_provider_tool_name_is_rejected_without_fallback() {
        let tool_names = ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names");
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [{
                "type": "function_call",
                "id": "item_1",
                "call_id": "call_1",
                "name": "unadvertised_valid_name",
                "arguments": "{}"
            }]
        }))
        .expect("response");

        let error = response
            .native_tool_calls_with_names(&tool_names)
            .expect_err("unknown provider name rejected");
        assert!(matches!(
            error,
            ProviderError::MalformedResponse { ref message }
                if message == "provider returned an unadvertised tool name"
        ));
    }

    #[test]
    fn unknown_provider_tool_name_is_rejected_before_missing_call_id_validation() {
        assert_unknown_provider_tool_error(None, "{}");
    }

    #[test]
    fn unknown_provider_tool_name_is_rejected_before_malformed_arguments_validation() {
        assert_unknown_provider_tool_error(Some("call_1"), "{\"query\":");
    }

    #[test]
    fn unknown_provider_tool_name_is_rejected_before_non_object_arguments_validation() {
        assert_unknown_provider_tool_error(Some("call_1"), "[]");
    }

    fn assert_unknown_provider_tool_error(call_id: Option<&str>, arguments: &str) {
        let tool_names = ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names");
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [{
                "type": "function_call",
                "id": "item_1",
                "call_id": call_id,
                "name": "sensitive_x2e_provider_x3a_value",
                "arguments": arguments
            }]
        }))
        .expect("response");

        let error = response
            .native_tool_calls_with_names(&tool_names)
            .expect_err("unknown provider name rejected first");

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { ref message }
                if message == "provider returned an unadvertised tool name"
        ));
        assert!(!error.to_string().contains("sensitive"));
    }

    #[test]
    fn noema_response_text_format_requires_text_response_text() {
        let value = noema_response_text_format();
        let one_of = value["format"]["schema"]["properties"]["responses"]["items"]["oneOf"]
            .as_array()
            .expect("responses items oneOf");
        let text_schema = one_of
            .iter()
            .find(|schema| schema["properties"]["kind"]["enum"] == serde_json::json!(["text"]))
            .expect("text response schema");

        assert_eq!(
            text_schema["required"],
            serde_json::json!(["kind", "phase", "text"])
        );
        assert_eq!(text_schema["additionalProperties"], false);
    }

    #[test]
    fn noema_response_text_format_includes_multiple_choice_response() {
        let value = noema_response_text_format();
        let one_of = value["format"]["schema"]["properties"]["responses"]["items"]["oneOf"]
            .as_array()
            .expect("responses items oneOf");
        let multiple_choice_schema = one_of
            .iter()
            .find(|schema| {
                schema["properties"]["kind"]["enum"] == serde_json::json!(["multiple_choice"])
            })
            .expect("multiple choice response schema");

        assert_eq!(
            multiple_choice_schema["required"],
            serde_json::json!(["kind", "phase", "prompt", "selection_mode", "options"])
        );
        assert_eq!(
            multiple_choice_schema["properties"]["selection_mode"]["enum"],
            serde_json::json!(["pick_one", "pick_many"])
        );
        assert_eq!(multiple_choice_schema["additionalProperties"], false);
    }

    #[test]
    fn prompt_cache_key_uses_non_empty_conversation_id() {
        assert_eq!(
            prompt_cache_key_from_conversation_id(Some(" conversation:cacheable ")).as_deref(),
            Some("conversation:cacheable")
        );
        assert_eq!(prompt_cache_key_from_conversation_id(Some("  ")), None);
        assert_eq!(prompt_cache_key_from_conversation_id(None), None);
    }

    #[test]
    fn responses_response_parses_function_call_output_items() {
        let tools = vec![
            noema_capabilities::ToolSpec::new(
                "mcp.docs:read",
                "Read docs.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
            )
            .expect("tool"),
        ];
        let tool_names = ResponsesToolNameMap::from_tools(&tools).expect("tool names");
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "mcp_x2e_docs_x3a_read",
                    "arguments": "{\"document_id\":\"doc_1\"}"
                }
            ]
        }))
        .expect("response");

        let calls = response
            .native_tool_calls_with_names(&tool_names)
            .expect("tool calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id.as_deref(), Some("item_1"));
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("call_1"));
        assert_eq!(
            calls[0].provider_name.as_deref(),
            Some("mcp_x2e_docs_x3a_read")
        );
        assert_eq!(calls[0].name, "mcp.docs:read");
        assert_eq!(calls[0].payload["document_id"], "doc_1");
    }

    #[test]
    fn responses_response_rejects_invalid_function_call_arguments_json() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "search_memory",
                    "arguments": "{\"query\":"
                }
            ]
        }))
        .expect("response");

        let error = response
            .native_tool_calls_with_names(
                &ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names"),
            )
            .expect_err("invalid arguments rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("failed to parse native tool call arguments for search_memory")
        );
    }

    #[test]
    fn responses_response_rejects_non_object_function_call_arguments() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "search_memory",
                    "arguments": "[]"
                }
            ]
        }))
        .expect("response");

        let error = response
            .native_tool_calls_with_names(
                &ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names"),
            )
            .expect_err("non-object arguments rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("native tool call arguments for search_memory must be a JSON object")
        );
    }

    #[test]
    fn responses_response_rejects_missing_function_call_id() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "name": "search_memory",
                    "arguments": "{\"query\":\"trains\"}"
                }
            ]
        }))
        .expect("response");

        let error = response
            .native_tool_calls_with_names(
                &ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names"),
            )
            .expect_err("missing call_id rejected");

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(
            error
                .to_string()
                .contains("native tool call search_memory is missing call_id")
        );
    }

    #[test]
    fn responses_input_serializes_native_tool_results() {
        let input =
            GenerateInput::NativeToolResults(vec![crate::provider::GenerateToolResultInput {
                id: Some("item_1".to_string()),
                call_id: "call_1".to_string(),
                name: "mcp.docs:read".to_string(),
                provider_name: Some("mcp_x2e_docs_x3a_read".to_string()),
                arguments: serde_json::json!({"document_id": "doc_1"}),
                success: true,
                payload: serde_json::json!({"title": "Docs"}),
            }]);

        let value = serde_json::to_value(ResponsesInput::from(&input)).expect("serialize");

        assert_eq!(value[0]["type"], "function_call");
        assert_eq!(value[0]["id"], "item_1");
        assert_eq!(value[0]["call_id"], "call_1");
        assert_eq!(value[0]["name"], "mcp_x2e_docs_x3a_read");
        let arguments: Value =
            serde_json::from_str(value[0]["arguments"].as_str().expect("arguments string"))
                .expect("arguments json");
        assert_eq!(arguments["document_id"], "doc_1");
        assert_eq!(value[1]["type"], "function_call_output");
        assert_eq!(value[1]["call_id"], "call_1");
        let output: Value =
            serde_json::from_str(value[1]["output"].as_str().expect("output string"))
                .expect("output json");
        assert_eq!(output["call_id"], "call_1");
        assert_eq!(output["name"], "mcp.docs:read");
        assert_eq!(output["provider_name"], "mcp_x2e_docs_x3a_read");
        assert_eq!(output["success"], true);
        assert_eq!(output["payload"]["title"], "Docs");
    }

    #[test]
    fn chained_response_sends_only_new_tool_outputs() {
        let mut request = GenerateRequest {
            input: GenerateInput::NativeToolResults(vec![
                crate::provider::GenerateToolResultInput {
                    id: Some("item_1".to_string()),
                    call_id: "call_1".to_string(),
                    name: "search_memory".to_string(),
                    provider_name: None,
                    arguments: serde_json::json!({"query": "trains"}),
                    success: true,
                    payload: serde_json::json!({"matches": []}),
                },
            ]),
            ..GenerateRequest::text("unused")
        };
        request.options.previous_response_id = Some("resp_previous".to_string());
        request.options.store_response = true;

        let (body, _) = ResponsesRequest::from_generate(
            &request,
            "gpt-test".to_string(),
            None,
            OPENAI_RESPONSES_PROFILE,
        )
        .expect("chained request");
        let value = serde_json::to_value(body).expect("serialize chained request");

        assert_eq!(value["previous_response_id"], "resp_previous");
        assert_eq!(value["store"], true);
        assert_eq!(value["input"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["input"][0]["type"], "function_call_output");
        assert_eq!(value["input"][0]["call_id"], "call_1");
    }

    #[test]
    fn responses_input_serializes_typed_history_items() {
        let input = GenerateInput::Items(vec![
            crate::provider::GenerateInputItem::Message(crate::GenerateMessage {
                role: crate::GenerateMessageRole::User,
                content: "Rename yourself to Momo".to_string(),
            }),
            crate::provider::GenerateInputItem::ToolCall(crate::provider::GenerateToolCallInput {
                id: Some("item_1".to_string()),
                call_id: "call_1".to_string(),
                name: "update_own_name".to_string(),
                provider_name: None,
                arguments: serde_json::json!({"name": "Momo"}),
            }),
            crate::provider::GenerateInputItem::ToolResult(
                crate::provider::GenerateToolResultInput {
                    id: Some("item_1".to_string()),
                    call_id: "call_1".to_string(),
                    name: "update_own_name".to_string(),
                    provider_name: None,
                    arguments: serde_json::Value::Null,
                    success: true,
                    payload: serde_json::json!({"display_name": "Momo"}),
                },
            ),
        ]);

        let value = serde_json::to_value(ResponsesInput::from(&input)).expect("serialize");

        assert_eq!(value[0]["role"], "user");
        assert_eq!(value[1]["type"], "function_call");
        assert_eq!(value[1]["call_id"], "call_1");
        assert_eq!(value[1]["name"], "update_own_name");
        assert_eq!(value[2]["type"], "function_call_output");
        assert_eq!(value[2]["call_id"], "call_1");
        let output: Value =
            serde_json::from_str(value[2]["output"].as_str().expect("output string"))
                .expect("output json");
        assert_eq!(output["name"], "update_own_name");
        assert_eq!(output["payload"]["display_name"], "Momo");
    }

    #[test]
    fn serializes_reasoning_history_item_for_replay() {
        let input =
            GenerateInput::Items(vec![GenerateInputItem::Reasoning(GenerateReasoningInput {
                id: Some("rs_1".to_string()),
                encrypted_content: "opaque-openai-reasoning".to_string(),
            })]);

        let ResponsesInput::Items(items) = ResponsesInput::from(&input) else {
            panic!("expected items");
        };
        let value = serde_json::to_value(&items[0]).expect("json");
        assert_eq!(value["type"], "reasoning");
        assert_eq!(value["id"], "rs_1");
        assert_eq!(value["encrypted_content"], "opaque-openai-reasoning");
    }

    #[test]
    fn responses_input_encodes_unsafe_typed_history_tool_names() {
        let input = GenerateInput::Items(vec![crate::provider::GenerateInputItem::ToolCall(
            crate::provider::GenerateToolCallInput {
                id: None,
                call_id: "call_1".to_string(),
                name: "mcp.dex:search contacts".to_string(),
                provider_name: None,
                arguments: serde_json::json!({"query": "Gautam"}),
            },
        )]);

        let value = serde_json::to_value(ResponsesInput::from(&input)).expect("serialize");

        assert_eq!(value[0]["type"], "function_call");
        assert_eq!(value[0]["name"], "mcp_x2e_dex_x3a_search_x20_contacts");
    }

    #[test]
    fn responses_tool_name_map_uses_provider_safe_names_and_maps_back() {
        let tools = vec![
            noema_capabilities::ToolSpec::new(
                "mcp.docs:read",
                "Read docs.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
            )
            .expect("tool"),
        ];
        let tool_names = ResponsesToolNameMap::from_tools(&tools).expect("tool names");

        assert_eq!(tool_names.tools[0].name, "mcp_x2e_docs_x3a_read");

        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "resp_1",
            "model": "gpt-test",
            "output": [
                {
                    "type": "function_call",
                    "id": "item_1",
                    "call_id": "call_1",
                    "name": "mcp_x2e_docs_x3a_read",
                    "arguments": "{\"document_id\":\"doc_1\"}"
                }
            ]
        }))
        .expect("response");

        let calls = response
            .native_tool_calls_with_names(&tool_names)
            .expect("tool calls");

        assert_eq!(calls[0].name, "mcp.docs:read");
    }

    #[test]
    fn responses_tool_name_map_rejects_provider_safe_name_collisions() {
        let first = noema_capabilities::ToolSpec::new(
            "mcp.docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        )
        .expect("first tool");
        let second = noema_capabilities::ToolSpec::new(
            "mcp_x2e_docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        )
        .expect("second tool");

        let error =
            ResponsesToolNameMap::from_tools(&[first, second]).expect_err("collision rejected");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(
            error
                .to_string()
                .contains("provider-safe tool name collision")
        );
    }
}
