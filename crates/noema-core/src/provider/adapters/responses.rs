//! Shared transport and parser for OpenAI-compatible Responses API calls.

use super::sse::SseAccumulator;
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    provider::{
        GenerateInput, GenerateInputItem, GenerateMessageRole, GenerateReasoningInput,
        GenerateReasoningItem, GenerateRequest, GenerateResponse, GenerateResponseStatus,
        GenerateStreamEvent, GenerateToolCallInput, GenerateToolResultInput, ParsedNoemaResponse,
        PromptCacheRetention, ProviderError, ReasoningEffort, TokenUsage, output_items_from_text,
        required_noema_response_from_text_with_native_tool_calls,
    },
};
use futures_util::StreamExt;
use reqwest::{
    StatusCode,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
    pub tool_choice: Option<&'static str>,
    /// Whether parallel independent tool calls are allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Additional provider output fields to include in responses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<&'static str>,
    /// Provider prompt-cache key used to bind reusable prefixes to a conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
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
    include_encrypted_reasoning: bool,
    stream: bool,
}

pub(crate) const OPENAI_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::String,
    forward_max_output_tokens: true,
    forward_prompt_cache_retention: true,
    include_encrypted_reasoning: true,
    stream: false,
};

pub(crate) const CODEX_RESPONSES_PROFILE: ResponsesRequestProfile = ResponsesRequestProfile {
    input_shape: ResponsesInputShape::MessageArray,
    forward_max_output_tokens: false,
    forward_prompt_cache_retention: false,
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
            input: ResponsesInput::from_generate(&request.input, profile.input_shape),
            instructions: request
                .instructions
                .as_deref()
                .filter(|instructions| !instructions.trim().is_empty())
                .map(ToString::to_string),
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
            tool_choice: responses_tool_choice(request.tool_choice, has_tools),
            parallel_tool_calls: has_tools.then_some(request.parallel_tool_calls),
            include: if profile.include_encrypted_reasoning {
                vec!["reasoning.encrypted_content"]
            } else {
                Vec::new()
            },
            prompt_cache_key: prompt_cache_key_from_conversation_id(
                request.conversation_id.as_deref(),
            ),
            store: false,
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

/// Native Responses API tool definition.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
    description: String,
    parameters: Value,
}

impl ResponsesTool {
    /// Build a native function tool definition.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Request-local provider-safe tool names for OpenAI-compatible adapters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponsesToolNameMap {
    pub(crate) tools: Vec<ResponsesTool>,
    provider_to_canonical: HashMap<String, String>,
}

impl ResponsesToolNameMap {
    /// Lower canonical Noema tool names into provider-safe Responses tools.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when two canonical names map
    /// to the same provider-safe name.
    pub(crate) fn from_tools(
        tools: &[crate::provider::NoemaToolSpec],
    ) -> Result<Self, ProviderError> {
        let mut responses_tools = Vec::with_capacity(tools.len());
        let mut provider_to_canonical = HashMap::with_capacity(tools.len());

        for tool in tools {
            let canonical = tool.name.as_str();
            let provider_safe = provider_safe_tool_name(canonical);
            if let Some(existing) = provider_to_canonical.get(&provider_safe) {
                let message = if existing == canonical {
                    format!("duplicate tool name {canonical}")
                } else {
                    format!(
                        "provider-safe tool name collision: {existing} and {canonical} both map to {provider_safe}"
                    )
                };
                return Err(ProviderError::InvalidRequest { message });
            }

            provider_to_canonical.insert(provider_safe.clone(), canonical.to_string());
            responses_tools.push(ResponsesTool::function(
                provider_safe,
                tool.description.clone(),
                tool.input_schema.as_value().clone(),
            ));
        }

        Ok(Self {
            tools: responses_tools,
            provider_to_canonical,
        })
    }

    fn canonical_name<'a>(&'a self, provider_name: &'a str) -> &'a str {
        self.provider_to_canonical
            .get(provider_name)
            .map(String::as_str)
            .unwrap_or(provider_name)
    }
}

pub(crate) fn responses_tool_choice(
    tool_choice: crate::provider::NoemaToolChoice,
    has_tools: bool,
) -> Option<&'static str> {
    has_tools.then_some(match tool_choice {
        crate::provider::NoemaToolChoice::Auto => "auto",
        crate::provider::NoemaToolChoice::None => "none",
        crate::provider::NoemaToolChoice::Required => "required",
    })
}

pub(crate) fn provider_safe_tool_name(canonical: &str) -> String {
    let mut encoded = String::with_capacity(canonical.len());
    for byte in canonical.bytes() {
        let character = byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            encoded.push(character);
        } else {
            encoded.push_str(&format!("_x{byte:02x}_"));
        }
    }

    const OPENAI_FUNCTION_NAME_MAX: usize = 64;
    if encoded.len() <= OPENAI_FUNCTION_NAME_MAX {
        return encoded;
    }

    const HASH_SUFFIX_LEN: usize = 18;
    let mut prefix = encoded;
    prefix.truncate(OPENAI_FUNCTION_NAME_MAX - HASH_SUFFIX_LEN);
    format!("{prefix}_h{:016x}", fnv1a64(canonical.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(super) fn noema_response_text_format() -> Value {
    serde_json::json!({
        "format": {
            "type": "json_schema",
            "name": "noema_response",
            "strict": false,
            "schema": {
                "type": "object",
                "properties": {
                    "response_status": {
                        "type": "string",
                        "enum": ["needs_tools", "final"]
                    },
                    "responses": {
                        "type": "array",
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["text"]
                                        },
                                        "phase": {
                                            "type": "string",
                                            "enum": ["commentary", "final_answer"]
                                        },
                                        "text": {
                                            "type": "string"
                                        }
                                    },
                                    "required": ["kind", "phase", "text"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["multiple_choice"]
                                        },
                                        "phase": {
                                            "type": "string",
                                            "enum": ["commentary", "final_answer"]
                                        },
                                        "prompt": {
                                            "type": "string"
                                        },
                                        "selection_mode": {
                                            "type": "string",
                                            "enum": ["pick_one", "pick_many"]
                                        },
                                        "options": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "id": {"type": "string"},
                                                    "label": {"type": "string"}
                                                },
                                                "required": ["id", "label"],
                                                "additionalProperties": false
                                            }
                                        }
                                    },
                                    "required": ["kind", "phase", "prompt", "selection_mode", "options"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "type": "string",
                                            "enum": ["structured"]
                                        },
                                        "schema": {
                                            "type": "string"
                                        },
                                        "payload": {
                                            "type": "object"
                                        }
                                    },
                                    "required": ["kind", "schema", "payload"],
                                    "additionalProperties": false
                                }
                            ]
                        }
                    },
                    "tool_calls": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": ["string", "null"]},
                                "name": {"type": "string"},
                                "payload": {"type": "object"}
                            },
                            "required": ["name", "payload"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["response_status", "responses", "tool_calls"],
                "additionalProperties": false
            }
        }
    })
}

/// Responses API input shape.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResponsesInput {
    /// Plain text input.
    Text(String),
    /// Structured Responses input items.
    Items(Vec<ResponsesInputItem>),
}

/// Provider-specific wire shape for otherwise shared Responses input items.
#[derive(Debug, Clone, Copy)]
pub enum ResponsesInputShape {
    /// Keep plain text as the Responses API string shorthand.
    String,
    /// Lower plain text to a user message in the structured item array.
    MessageArray,
}

impl ResponsesInput {
    /// Lower provider-neutral input to the requested Responses wire shape.
    #[must_use]
    pub fn from_generate(value: &GenerateInput, shape: ResponsesInputShape) -> Self {
        if let (GenerateInput::Text(text), ResponsesInputShape::MessageArray) = (value, shape) {
            return Self::Items(vec![ResponsesInputItem::Message(ResponsesInputMessage {
                role: "user",
                content: text.clone(),
            })]);
        }
        Self::from(value)
    }
}

impl From<&GenerateInput> for ResponsesInput {
    fn from(value: &GenerateInput) -> Self {
        match value {
            GenerateInput::Text(text) => Self::Text(text.clone()),
            GenerateInput::Messages(messages) => Self::Items(
                messages
                    .iter()
                    .filter(|message| !message.content.trim().is_empty())
                    .map(|message| {
                        ResponsesInputItem::Message(ResponsesInputMessage {
                            role: match message.role {
                                GenerateMessageRole::User => "user",
                                GenerateMessageRole::Assistant => "assistant",
                            },
                            content: message.content.clone(),
                        })
                    })
                    .collect(),
            ),
            GenerateInput::Items(items) => Self::Items(
                items
                    .iter()
                    .filter(|item| !item.is_empty())
                    .map(ResponsesInputItem::from)
                    .collect(),
            ),
            GenerateInput::NativeToolResults(results) => {
                let mut items = Vec::with_capacity(results.len().saturating_mul(2));
                for result in results {
                    items.push(ResponsesInputItem::FunctionCall(
                        ResponsesFunctionCall::from(result),
                    ));
                    items.push(ResponsesInputItem::FunctionCallOutput(
                        ResponsesFunctionCallOutput::from(result),
                    ));
                }
                Self::Items(items)
            }
        }
    }
}

/// One Responses API input item.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResponsesInputItem {
    /// Provider role message.
    Message(ResponsesInputMessage),
    /// Provider-encrypted reasoning context.
    Reasoning(ResponsesReasoningItem),
    /// Prior native function call context.
    FunctionCall(ResponsesFunctionCall),
    /// Native function-call output.
    FunctionCallOutput(ResponsesFunctionCallOutput),
}

impl From<&GenerateToolResultInput> for ResponsesInputItem {
    fn from(value: &GenerateToolResultInput) -> Self {
        Self::FunctionCallOutput(ResponsesFunctionCallOutput::from(value))
    }
}

impl From<&GenerateInputItem> for ResponsesInputItem {
    fn from(value: &GenerateInputItem) -> Self {
        match value {
            GenerateInputItem::Message(message) => Self::Message(ResponsesInputMessage {
                role: match message.role {
                    GenerateMessageRole::User => "user",
                    GenerateMessageRole::Assistant => "assistant",
                },
                content: message.content.clone(),
            }),
            GenerateInputItem::Reasoning(reasoning) => {
                Self::Reasoning(ResponsesReasoningItem::from(reasoning))
            }
            GenerateInputItem::ToolCall(call) => {
                Self::FunctionCall(ResponsesFunctionCall::from(call))
            }
            GenerateInputItem::ToolResult(result) => {
                Self::FunctionCallOutput(ResponsesFunctionCallOutput::from(result))
            }
        }
    }
}

/// One Responses API encrypted reasoning input item.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesReasoningItem {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    encrypted_content: String,
}

impl From<&GenerateReasoningInput> for ResponsesReasoningItem {
    fn from(reasoning: &GenerateReasoningInput) -> Self {
        Self {
            kind: "reasoning",
            id: reasoning.id.clone(),
            encrypted_content: reasoning.encrypted_content.clone(),
        }
    }
}

/// One Responses API input message.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesInputMessage {
    /// Provider role.
    pub role: &'static str,
    /// Message text.
    pub content: String,
}

/// One Responses API native function-call context input item.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesFunctionCall {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    call_id: String,
    name: String,
    arguments: String,
}

impl From<&GenerateToolResultInput> for ResponsesFunctionCall {
    fn from(result: &GenerateToolResultInput) -> Self {
        Self {
            kind: "function_call",
            id: result.id.clone(),
            call_id: result.call_id.clone(),
            name: result
                .provider_name
                .clone()
                .unwrap_or_else(|| provider_safe_tool_name(&result.name)),
            arguments: result.arguments.to_string(),
        }
    }
}

impl From<&GenerateToolCallInput> for ResponsesFunctionCall {
    fn from(call: &GenerateToolCallInput) -> Self {
        Self {
            kind: "function_call",
            id: call.id.clone(),
            call_id: call.call_id.clone(),
            name: call
                .provider_name
                .clone()
                .unwrap_or_else(|| provider_safe_tool_name(&call.name)),
            arguments: call.arguments.to_string(),
        }
    }
}

/// One Responses API native function-call output input item.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesFunctionCallOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    call_id: String,
    output: String,
}

impl ResponsesFunctionCallOutput {
    fn new(result: &GenerateToolResultInput) -> Self {
        Self {
            kind: "function_call_output",
            call_id: result.call_id.clone(),
            output: result.output_json_string(),
        }
    }
}

impl From<&GenerateToolResultInput> for ResponsesFunctionCallOutput {
    fn from(result: &GenerateToolResultInput) -> Self {
        Self::new(result)
    }
}

/// Parsed Responses-compatible API response.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResponsesResponse {
    /// Provider response id.
    pub id: Option<String>,
    /// Model reported by the provider.
    pub model: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
    /// Token usage reported by the provider.
    pub usage: Option<ResponsesUsage>,
    #[serde(default, skip)]
    raw: Option<Value>,
}

impl ResponsesResponse {
    /// Finalize one shared Responses result into Noema's provider-neutral response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when native tool calls or assistant output are malformed.
    pub(crate) fn finalize(
        self,
        tool_names: &ResponsesToolNameMap,
        require_noema_response: bool,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> Result<GenerateResponse, ProviderError> {
        let native_tool_calls = self.native_tool_calls_with_names(tool_names)?;
        let text = match self.output_text() {
            Ok(text) => text,
            Err(ProviderError::MalformedResponse { .. }) if !native_tool_calls.is_empty() => {
                let parsed = ParsedNoemaResponse {
                    responses: Vec::new(),
                    tool_calls: native_tool_calls,
                    response_status: GenerateResponseStatus::NeedsTools,
                };
                return Ok(self.generate_response(parsed, diagnostics));
            }
            Err(error @ ProviderError::MalformedResponse { .. }) => {
                diagnostics.log_malformed_error(&error, self.id.as_deref(), self.raw_payload());
                return Err(error);
            }
            Err(error) => return Err(error),
        };

        let parsed = if require_noema_response {
            required_noema_response_from_text_with_native_tool_calls(
                text.clone(),
                native_tool_calls,
            )
            .inspect_err(|error| {
                diagnostics.log_malformed_error(
                    error,
                    self.id.as_deref(),
                    serde_json::json!({ "provider_text": text }),
                );
            })?
        } else {
            let response_status = if native_tool_calls.is_empty() {
                GenerateResponseStatus::Final
            } else {
                GenerateResponseStatus::NeedsTools
            };
            ParsedNoemaResponse {
                responses: output_items_from_text(text)?,
                tool_calls: native_tool_calls,
                response_status,
            }
        };

        Ok(self.generate_response(parsed, diagnostics))
    }

    fn generate_response(
        self,
        parsed: ParsedNoemaResponse,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> GenerateResponse {
        let reasoning_items = self.reasoning_items();
        GenerateResponse::from_parsed(
            parsed,
            diagnostics.provider_kind.clone(),
            self.model.unwrap_or_else(|| diagnostics.model.clone()),
            self.id,
            self.usage.map(Into::into),
        )
        .with_reasoning_items(reasoning_items)
    }

    /// Return the raw provider payload preserved for developer diagnostics.
    #[must_use]
    pub fn raw_payload(&self) -> Value {
        self.raw.clone().unwrap_or_else(|| {
            serde_json::json!({
                "id": self.id.clone(),
                "model": self.model.clone(),
                "output": self.output.clone(),
                "usage": self.usage.clone(),
            })
        })
    }

    /// Collect assistant output text in provider order.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MalformedResponse`] when the response contains
    /// no text, or [`ProviderError::ApiError`] when the only textual payload is
    /// a refusal.
    pub fn output_text(&self) -> Result<String, ProviderError> {
        let mut output = String::new();
        let mut refusals = Vec::new();

        for item in &self.output {
            let ResponsesOutputItem::Message { content } = item else {
                continue;
            };

            for content_item in content {
                match content_item {
                    ResponsesContent::OutputText { text } => output.push_str(text),
                    ResponsesContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                    ResponsesContent::Other => {}
                }
            }
        }

        if !output.is_empty() {
            return Ok(output);
        }

        if !refusals.is_empty() {
            return Err(ProviderError::ApiError {
                status: 200,
                message: refusals.join("\n"),
                request_id: self.id.clone(),
            });
        }

        Err(ProviderError::MalformedResponse {
            message: "response did not contain output_text".to_string(),
        })
    }

    /// Collect provider-native function-call output items.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MalformedResponse`] when a function-call item
    /// contains invalid JSON arguments.
    pub fn native_tool_calls(
        &self,
    ) -> Result<Vec<crate::provider::GenerateToolCall>, ProviderError> {
        self.native_tool_calls_with_names(&ResponsesToolNameMap::default())
    }

    pub(crate) fn native_tool_calls_with_names(
        &self,
        tool_names: &ResponsesToolNameMap,
    ) -> Result<Vec<crate::provider::GenerateToolCall>, ProviderError> {
        let mut calls = Vec::new();
        for item in &self.output {
            let ResponsesOutputItem::FunctionCall {
                id,
                call_id,
                name,
                arguments,
            } = item
            else {
                continue;
            };
            let Some(call_id) = call_id.as_ref().filter(|value| !value.trim().is_empty()) else {
                return Err(ProviderError::MalformedResponse {
                    message: format!("native tool call {name} is missing call_id"),
                });
            };
            let payload: Value = serde_json::from_str(arguments).map_err(|source| {
                ProviderError::MalformedResponse {
                    message: format!(
                        "failed to parse native tool call arguments for {name}: {source}"
                    ),
                }
            })?;
            if !payload.is_object() {
                return Err(ProviderError::MalformedResponse {
                    message: format!("native tool call arguments for {name} must be a JSON object"),
                });
            }
            calls.push(crate::provider::GenerateToolCall {
                id: id.clone(),
                provider_call_id: Some(call_id.clone()),
                provider_name: Some(name.clone()),
                name: tool_names.canonical_name(name).to_string(),
                payload,
            });
        }
        Ok(calls)
    }

    /// Collect encrypted reasoning output items for stateless replay.
    #[must_use]
    pub fn reasoning_items(&self) -> Vec<GenerateReasoningItem> {
        self.output
            .iter()
            .filter_map(|item| match item {
                ResponsesOutputItem::Reasoning {
                    id,
                    encrypted_content,
                } => encrypted_content
                    .as_ref()
                    .map(|encrypted_content| GenerateReasoningItem {
                        id: id.clone(),
                        encrypted_content: Some(encrypted_content.clone()),
                    }),
                _ => None,
            })
            .collect()
    }

    pub(super) fn from_stream_parts(
        id: Option<String>,
        model: Option<String>,
        output_values: Vec<Value>,
        usage: Option<ResponsesUsage>,
    ) -> Result<Self, ProviderError> {
        let raw_output_values = output_values.clone();
        let output = output_values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
                    message: format!("failed to parse SSE output item: {source}"),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let raw = serde_json::json!({
            "id": id.clone(),
            "model": model.clone(),
            "output": raw_output_values,
            "usage": usage.clone(),
        });
        Ok(Self {
            id,
            model,
            output,
            usage,
            raw: Some(raw),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
    #[serde(rename = "function_call")]
    FunctionCall {
        id: Option<String>,
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        id: Option<String>,
        encrypted_content: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

/// Token usage reported by a Responses-compatible API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "total_tokens")]
    total: u64,
    #[serde(default, rename = "input_tokens_details")]
    input_details: Option<ResponsesInputTokenDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponsesInputTokenDetails {
    #[serde(default)]
    cached_tokens: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
            cached_input_tokens: value.input_details.map(|details| details.cached_tokens),
        }
    }
}

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

    fn test_tool() -> crate::provider::NoemaToolSpec {
        crate::provider::NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("tool")
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
            crate::provider::NoemaToolSpec::new(
                "mcp.docs:read",
                "Read docs.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
                crate::provider::NoemaToolExecution::LocalBuiltin,
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
            .native_tool_calls()
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
            .native_tool_calls()
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
            .native_tool_calls()
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
            crate::provider::NoemaToolSpec::new(
                "mcp.docs:read",
                "Read docs.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
                crate::provider::NoemaToolExecution::LocalBuiltin,
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
        let first = crate::provider::NoemaToolSpec::new(
            "mcp.docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
        )
        .expect("first tool");
        let second = crate::provider::NoemaToolSpec::new(
            "mcp_x2e_docs",
            "Read docs.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            crate::provider::NoemaToolExecution::LocalBuiltin,
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
