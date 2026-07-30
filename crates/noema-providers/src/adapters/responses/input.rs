//! Responses-compatible request input wire types.

use super::tools::provider_safe_tool_name;
use crate::{
    GenerateInput, GenerateInputItem, GenerateReasoningInput, GenerateToolCallInput,
    GenerateToolResultInput, ProviderError,
};
use serde::Serialize;

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
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when explicit prompt-cache
    /// breakpoint indices are duplicated, exceed provider limits, or do not
    /// identify a message in the final filtered input.
    pub fn from_generate(
        value: &GenerateInput,
        shape: ResponsesInputShape,
        continuing_response: bool,
        prompt_cache_breakpoints: &[usize],
    ) -> Result<Self, ProviderError> {
        if let (GenerateInput::Text(text), ResponsesInputShape::MessageArray) = (value, shape) {
            let mut input = Self::Items(vec![ResponsesInputItem::Message(ResponsesInputMessage {
                role: "user",
                content: text.clone().into(),
            })]);
            input.apply_prompt_cache_breakpoints(prompt_cache_breakpoints)?;
            return Ok(input);
        }
        if let (GenerateInput::NativeToolResults(results), true) = (value, continuing_response) {
            let mut input = Self::Items(results.iter().map(ResponsesInputItem::from).collect());
            input.apply_prompt_cache_breakpoints(prompt_cache_breakpoints)?;
            return Ok(input);
        }
        let mut input = Self::from(value);
        input.apply_prompt_cache_breakpoints(prompt_cache_breakpoints)?;
        Ok(input)
    }

    fn apply_prompt_cache_breakpoints(
        &mut self,
        prompt_cache_breakpoints: &[usize],
    ) -> Result<(), ProviderError> {
        if prompt_cache_breakpoints.is_empty() {
            return Ok(());
        }
        if prompt_cache_breakpoints.len() > 4 {
            return Err(ProviderError::InvalidRequest {
                message: "Responses requests support at most four prompt-cache breakpoints"
                    .to_string(),
            });
        }

        let mut requested =
            std::collections::HashSet::with_capacity(prompt_cache_breakpoints.len());
        for index in prompt_cache_breakpoints {
            if !requested.insert(*index) {
                return Err(ProviderError::InvalidRequest {
                    message: format!("prompt-cache breakpoint message index {index} is duplicated"),
                });
            }
        }

        match self {
            Self::Text(text) => {
                if requested.len() != 1 || !requested.contains(&0) {
                    return Err(prompt_cache_breakpoint_index_error(
                        prompt_cache_breakpoints,
                        1,
                    ));
                }
                let mut message = ResponsesInputMessage {
                    role: "user",
                    content: std::mem::take(text).into(),
                };
                message.add_prompt_cache_breakpoint();
                *self = Self::Items(vec![ResponsesInputItem::Message(message)]);
            }
            Self::Items(items) => {
                let mut message_index = 0;
                for item in items {
                    let ResponsesInputItem::Message(message) = item else {
                        continue;
                    };
                    if requested.contains(&message_index) {
                        message.add_prompt_cache_breakpoint();
                    }
                    message_index += 1;
                }
                if requested.iter().any(|index| *index >= message_index) {
                    return Err(prompt_cache_breakpoint_index_error(
                        prompt_cache_breakpoints,
                        message_index,
                    ));
                }
            }
        }
        Ok(())
    }
}

fn prompt_cache_breakpoint_index_error(indices: &[usize], message_count: usize) -> ProviderError {
    ProviderError::InvalidRequest {
        message: format!(
            "prompt-cache breakpoint message indices {indices:?} are out of range for {message_count} filtered messages"
        ),
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
                            role: message.role.as_str(),
                            content: message.content.clone().into(),
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
                role: message.role.as_str(),
                content: message.content.clone().into(),
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
    summary: Vec<serde_json::Value>,
    encrypted_content: String,
}

impl From<&GenerateReasoningInput> for ResponsesReasoningItem {
    fn from(reasoning: &GenerateReasoningInput) -> Self {
        Self {
            kind: "reasoning",
            id: reasoning.id.clone(),
            summary: Vec::new(),
            encrypted_content: reasoning.encrypted_content.clone(),
        }
    }
}

/// One Responses API input message.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesInputMessage {
    /// Provider role.
    pub role: &'static str,
    /// Message text or content blocks carrying provider controls.
    pub content: ResponsesInputMessageContent,
}

impl ResponsesInputMessage {
    fn add_prompt_cache_breakpoint(&mut self) {
        let text = match std::mem::replace(
            &mut self.content,
            ResponsesInputMessageContent::Blocks(Vec::new()),
        ) {
            ResponsesInputMessageContent::Text(text) => text,
            ResponsesInputMessageContent::Blocks(mut blocks) => {
                if let Some(block) = blocks.last_mut() {
                    block.prompt_cache_breakpoint =
                        Some(ResponsesPromptCacheBreakpoint { mode: "explicit" });
                }
                self.content = ResponsesInputMessageContent::Blocks(blocks);
                return;
            }
        };
        self.content = ResponsesInputMessageContent::Blocks(vec![ResponsesInputText {
            kind: "input_text",
            text,
            prompt_cache_breakpoint: Some(ResponsesPromptCacheBreakpoint { mode: "explicit" }),
        }]);
    }
}

/// Responses message content, retaining the string shorthand unless metadata is required.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResponsesInputMessageContent {
    /// Plain message text.
    Text(String),
    /// Structured input content blocks.
    Blocks(Vec<ResponsesInputText>),
}

impl From<String> for ResponsesInputMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

/// Responses API input-text block.
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesInputText {
    #[serde(rename = "type")]
    kind: &'static str,
    pub(super) text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_breakpoint: Option<ResponsesPromptCacheBreakpoint>,
}

/// Explicit cache marker attached to a supported Responses content block.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ResponsesPromptCacheBreakpoint {
    mode: &'static str,
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
