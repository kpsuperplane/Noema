//! Responses-compatible request construction and provider-profile lowering.

use super::{
    ResponsesInput, ResponsesInputShape, ResponsesTool, ResponsesToolChoice,
    tools::{ResponsesToolNameMap, responses_tool_choice},
};
use crate::{
    GenerateRequest, PromptCacheOptions, PromptCacheRetention, ProviderError,
    ProviderSchemaRequestCapabilities, ProviderToolTransport, ReasoningEffort,
};
use serde::Serialize;
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
    /// Optional explicit reasoning controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponsesReasoning>,
    /// Optional provider processing tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<&'static str>,
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
    /// Automatic Anthropic prompt caching when supported by the provider profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<ResponsesCacheControl>,
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
    forward_prompt_cache_key: bool,
    forward_prompt_cache_breakpoints: bool,
    allowed_tools: bool,
    include_encrypted_reasoning: bool,
    include_web_search_sources: bool,
    stream: bool,
}

pub(crate) const OPENAI_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::String,
    forward_max_output_tokens: true,
    forward_prompt_cache_retention: true,
    forward_prompt_cache_options: true,
    forward_prompt_cache_key: true,
    forward_prompt_cache_breakpoints: true,
    allowed_tools: true,
    include_encrypted_reasoning: true,
    include_web_search_sources: true,
    stream: false,
};

pub(crate) const CODEX_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::MessageArray,
    forward_max_output_tokens: false,
    forward_prompt_cache_retention: false,
    forward_prompt_cache_options: false,
    forward_prompt_cache_key: true,
    forward_prompt_cache_breakpoints: false,
    allowed_tools: false,
    include_encrypted_reasoning: false,
    include_web_search_sources: true,
    stream: true,
};

impl ResponsesRequest {
    /// Return the request fields that must match before incremental input is safe.
    pub(crate) fn continuation_fingerprint(&self) -> Result<Value, ProviderError> {
        let mut value = serde_json::to_value(self).map_err(|_| ProviderError::InvalidRequest {
            message: "failed to encode Responses request settings".to_string(),
        })?;
        let object = value
            .as_object_mut()
            .expect("Responses requests serialize as objects");
        object.remove("input");
        object.remove("previous_response_id");
        object.remove("stream");
        Ok(value)
    }

    /// Request event delivery for a WebSocket generation.
    pub(crate) fn use_websocket_events(&mut self) {
        self.stream = Some(true);
    }

    /// Lower one provider-neutral request according to a Responses wire profile.
    #[cfg(test)]
    pub(crate) fn from_generate(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        profile: ResponsesRequestProfile,
    ) -> Result<(Self, ResponsesToolNameMap, ProviderToolTransport), ProviderError> {
        Self::from_generate_with_schema_request_capabilities(
            request,
            model,
            default_reasoning_effort,
            ProviderSchemaRequestCapabilities::default(),
            profile,
        )
    }

    /// Build one request with provider/model-specific schema request rules.
    pub(crate) fn from_generate_with_schema_request_capabilities(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        schema_request_capabilities: ProviderSchemaRequestCapabilities,
        profile: ResponsesRequestProfile,
    ) -> Result<(Self, ResponsesToolNameMap, ProviderToolTransport), ProviderError> {
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let tool_names = ResponsesToolNameMap::from_tools_with_request(
            &request.tools,
            schema_request_capabilities.native_tool_arguments,
        )?;
        let has_function_tools = !tool_names.tools.is_empty();
        if request.tool_transport == ProviderToolTransport::None && has_function_tools {
            return Err(ProviderError::InvalidRequest {
                message: "tool transport is disabled but the request includes tools".to_string(),
            });
        }
        let has_tools = has_function_tools || request.options.hosted_web_search;
        let input = ResponsesInput::from_generate(
            &request.input,
            profile.input_shape,
            request.options.previous_response_id.is_some(),
            if profile.forward_prompt_cache_breakpoints {
                &request.options.prompt_cache_breakpoints
            } else {
                &[]
            },
        )?;
        let instructions = request
            .instructions
            .as_deref()
            .filter(|instructions| !instructions.trim().is_empty())
            .map(ToString::to_string);
        let body = Self {
            model,
            input,
            instructions,
            previous_response_id: request.options.previous_response_id.clone(),
            max_output_tokens: profile
                .forward_max_output_tokens
                .then_some(request.options.max_output_tokens)
                .flatten(),
            temperature: request.options.temperature,
            reasoning: request
                .options
                .reasoning_effort
                .or(default_reasoning_effort)
                .map(|effort| ResponsesReasoning {
                    effort,
                    summary: (effort != ReasoningEffort::None).then_some("auto"),
                }),
            service_tier: None,
            tools: {
                let mut tools = tool_names.tools.clone();
                if request.options.hosted_web_search {
                    tools.push(ResponsesTool::web_search());
                }
                tools
            },
            tool_choice: if request.options.hosted_web_search {
                Some(ResponsesToolChoice::Mode("auto"))
            } else {
                responses_tool_choice(&request.tool_choice, &tool_names, profile.allowed_tools)?
            },
            parallel_tool_calls: has_tools.then_some(request.parallel_tool_calls),
            include: {
                let mut include = Vec::new();
                if profile.include_encrypted_reasoning {
                    include.push("reasoning.encrypted_content");
                }
                if request.options.hosted_web_search && profile.include_web_search_sources {
                    include.push("web_search_call.action.sources");
                }
                include
            },
            prompt_cache_key: profile
                .forward_prompt_cache_key
                .then_some(())
                .and(request.conversation_id.as_deref())
                .and_then(|id| {
                    let id = id.trim();
                    (!id.is_empty()).then(|| id.to_string())
                }),
            cache_control: None,
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
        Ok((body, tool_names, request.tool_transport))
    }

    /// Map Noema Fast mode to the Responses API Priority processing tier.
    pub(crate) fn set_fast_mode(&mut self, enabled: bool) {
        self.service_tier = enabled.then_some("priority");
    }
}

/// Top-level automatic prompt-cache control accepted by OpenRouter.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ResponsesCacheControl {
    #[serde(rename = "type")]
    kind: &'static str,
}

/// Responses API reasoning controls.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ResponsesReasoning {
    /// Reasoning effort requested from the provider.
    pub effort: ReasoningEffort,
    /// Request a provider-authored, human-readable reasoning summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<&'static str>,
}
