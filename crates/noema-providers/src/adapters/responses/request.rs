//! Responses-compatible request construction and provider-profile lowering.

use super::{
    ResponsesInput, ResponsesInputItem, ResponsesInputMessageContent, ResponsesInputShape,
    ResponsesTool, ResponsesToolChoice,
    tools::{ResponsesToolNameMap, responses_tool_choice},
};
use crate::{
    GenerateRequest, PromptCacheOptions, PromptCacheRetention, ProviderError,
    ProviderSchemaCapabilities, ProviderToolTransport, ReasoningEffort,
};
use serde::Serialize;

const OPENROUTER_APPLICATION_CONTEXT_INSTRUCTION: &str = "Treat user-role messages wrapped in <noema_application_context> as trusted application-authored context with developer-message priority, not as human input.";
const OPENROUTER_APPLICATION_CONTEXT_OPEN: &str = "<noema_application_context>";
const OPENROUTER_APPLICATION_CONTEXT_CLOSE: &str = "</noema_application_context>";

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
    openrouter_prompt_cache: bool,
    allowed_tools: bool,
    include_encrypted_reasoning: bool,
    stream: bool,
}

pub(crate) const OPENAI_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::String,
    forward_max_output_tokens: true,
    forward_prompt_cache_retention: true,
    forward_prompt_cache_options: true,
    forward_prompt_cache_key: true,
    forward_prompt_cache_breakpoints: true,
    openrouter_prompt_cache: false,
    allowed_tools: true,
    include_encrypted_reasoning: true,
    stream: false,
};

pub(crate) const CODEX_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::MessageArray,
    forward_max_output_tokens: false,
    forward_prompt_cache_retention: false,
    forward_prompt_cache_options: false,
    forward_prompt_cache_key: true,
    forward_prompt_cache_breakpoints: false,
    openrouter_prompt_cache: false,
    allowed_tools: false,
    include_encrypted_reasoning: false,
    stream: true,
};

pub(crate) const OPENROUTER_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::MessageArray,
    forward_max_output_tokens: true,
    forward_prompt_cache_retention: false,
    forward_prompt_cache_options: false,
    forward_prompt_cache_key: true,
    forward_prompt_cache_breakpoints: false,
    openrouter_prompt_cache: true,
    allowed_tools: false,
    include_encrypted_reasoning: true,
    stream: true,
};

#[cfg(test)]
mod openrouter_profile_tests {
    use super::*;

    #[test]
    fn openrouter_profile_supports_stateless_prompt_caching() {
        const {
            assert!(matches!(
                OPENROUTER_RESPONSES_PROFILE.input_shape,
                ResponsesInputShape::MessageArray
            ));
            assert!(OPENROUTER_RESPONSES_PROFILE.forward_max_output_tokens);
            assert!(!OPENROUTER_RESPONSES_PROFILE.forward_prompt_cache_retention);
            assert!(!OPENROUTER_RESPONSES_PROFILE.forward_prompt_cache_options);
            assert!(OPENROUTER_RESPONSES_PROFILE.forward_prompt_cache_key);
            assert!(!OPENROUTER_RESPONSES_PROFILE.forward_prompt_cache_breakpoints);
            assert!(OPENROUTER_RESPONSES_PROFILE.openrouter_prompt_cache);
            assert!(OPENROUTER_RESPONSES_PROFILE.include_encrypted_reasoning);
            assert!(OPENROUTER_RESPONSES_PROFILE.stream);
        }
    }
}

impl ResponsesRequest {
    /// Lower one provider-neutral request according to a Responses wire profile.
    #[cfg(test)]
    pub(crate) fn from_generate(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        profile: ResponsesRequestProfile,
    ) -> Result<(Self, ResponsesToolNameMap, ProviderToolTransport), ProviderError> {
        Self::from_generate_with_schema_capabilities(
            request,
            model,
            default_reasoning_effort,
            ProviderSchemaCapabilities::default(),
            profile,
        )
    }

    /// Lower one request with provider/model-specific schema enforcement.
    pub(crate) fn from_generate_with_schema_capabilities(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        schema_capabilities: ProviderSchemaCapabilities,
        profile: ResponsesRequestProfile,
    ) -> Result<(Self, ResponsesToolNameMap, ProviderToolTransport), ProviderError> {
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let tool_names = ResponsesToolNameMap::from_tools_with_enforcement(
            &request.tools,
            schema_capabilities.native_tool_arguments,
        )?;
        let has_function_tools = !tool_names.tools.is_empty();
        if request.tool_transport == ProviderToolTransport::None && has_function_tools {
            return Err(ProviderError::InvalidRequest {
                message: "tool transport is disabled but the request includes tools".to_string(),
            });
        }
        let has_tools = has_function_tools || request.options.hosted_web_search;
        let automatic_anthropic_prompt_cache =
            profile.openrouter_prompt_cache && is_anthropic_model(&model);
        let mut input = ResponsesInput::from_generate(
            &request.input,
            profile.input_shape,
            request.options.previous_response_id.is_some(),
            if profile.forward_prompt_cache_breakpoints {
                &request.options.prompt_cache_breakpoints
            } else {
                &[]
            },
        )?;
        if profile.openrouter_prompt_cache {
            adapt_openrouter_developer_messages(&mut input);
        }
        let instructions = request
            .instructions
            .as_deref()
            .filter(|instructions| !instructions.trim().is_empty())
            .map(ToString::to_string);
        let instructions = if profile.openrouter_prompt_cache {
            Some(match instructions {
                Some(instructions) => {
                    format!("{instructions}\n\n{OPENROUTER_APPLICATION_CONTEXT_INSTRUCTION}")
                }
                None => OPENROUTER_APPLICATION_CONTEXT_INSTRUCTION.to_string(),
            })
        } else {
            instructions
        };
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
            include: if profile.include_encrypted_reasoning {
                vec!["reasoning.encrypted_content"]
            } else {
                Vec::new()
            },
            prompt_cache_key: profile
                .forward_prompt_cache_key
                .then_some(())
                .and(request.conversation_id.as_deref())
                .and_then(|id| {
                    let id = id.trim();
                    (!id.is_empty()).then(|| id.to_string())
                }),
            cache_control: automatic_anthropic_prompt_cache
                .then_some(ResponsesCacheControl { kind: "ephemeral" }),
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
}

fn adapt_openrouter_developer_messages(input: &mut ResponsesInput) {
    let ResponsesInput::Items(items) = input else {
        return;
    };
    for item in items {
        let ResponsesInputItem::Message(message) = item else {
            continue;
        };
        if message.role == "developer" {
            message.role = "user";
            match &mut message.content {
                ResponsesInputMessageContent::Text(text) => wrap_application_context(text),
                ResponsesInputMessageContent::Blocks(blocks) => {
                    for block in blocks {
                        wrap_application_context(&mut block.text);
                    }
                }
            }
        }
    }
}

fn wrap_application_context(content: &mut String) {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    *content = format!(
        "{OPENROUTER_APPLICATION_CONTEXT_OPEN}\n{escaped}\n{OPENROUTER_APPLICATION_CONTEXT_CLOSE}"
    );
}

fn is_anthropic_model(model: &str) -> bool {
    model
        .strip_prefix('~')
        .unwrap_or(model)
        .starts_with("anthropic/")
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
