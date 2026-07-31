use crate::response_support::tool_names::OpenAiToolDefinition;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateReasoningInput,
    GenerateRequest, GenerateToolCallInput, GenerateToolResultInput, NoemaToolChoice,
    ProviderError, ProviderSchemaCapabilities, ProviderToolTransport, ReasoningEffort,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) use crate::response_support::tool_names::OpenAiToolNameMap;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChatCompletionRequest {
    pub(crate) model: String,
    pub(crate) messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<ChatTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Value>,
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream_options: Option<ChatStreamOptions>,
}

impl ChatCompletionRequest {
    pub(crate) fn from_generate_with_schema_capabilities(
        request: &GenerateRequest,
        model: String,
        default_reasoning_effort: Option<ReasoningEffort>,
        schema_capabilities: ProviderSchemaCapabilities,
    ) -> Result<(Self, OpenAiToolNameMap, ProviderToolTransport), ProviderError> {
        if request.input.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let tool_names = OpenAiToolNameMap::from_tools_with_enforcement(
            &request.tools,
            schema_capabilities.native_tool_arguments,
        )?;
        if request.tool_transport == ProviderToolTransport::None
            && !tool_names.definitions.is_empty()
        {
            return Err(ProviderError::InvalidRequest {
                message: "tool transport is disabled but the request includes tools".to_string(),
            });
        }

        let has_function_tools = !tool_names.definitions.is_empty();
        let body = Self {
            model,
            messages: messages_from_generate(&request.input, request.instructions.as_deref()),
            max_completion_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
            reasoning: request
                .options
                .reasoning_effort
                .or(default_reasoning_effort)
                .map(|effort| serde_json::json!({"effort": effort})),
            tools: tool_names
                .definitions
                .iter()
                .map(ChatTool::function)
                .collect(),
            tool_choice: chat_tool_choice(&request.tool_choice, &tool_names)?,
            parallel_tool_calls: has_function_tools.then_some(request.parallel_tool_calls),
            prompt_cache_key: None,
            cache_control: None,
            stream: true,
            stream_options: Some(ChatStreamOptions {
                include_usage: true,
            }),
        };
        Ok((body, tool_names, request.tool_transport))
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ChatMessage {
    pub(crate) role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<ChatMessageContent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_calls: Vec<ChatToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) reasoning_details: Vec<Value>,
}

impl ChatMessage {
    fn text(role: GenerateMessageRole, content: impl Into<String>) -> Self {
        Self {
            role: role.as_str().to_string(),
            content: Some(ChatMessageContent::Text(content.into())),
            ..Self::default()
        }
    }

    fn assistant_tool_call(call: &GenerateToolCallInput) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            tool_calls: vec![ChatToolCall::from_input(call)],
            ..Self::default()
        }
    }

    fn tool_result(result: &GenerateToolResultInput) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(ChatMessageContent::Text(result.output_json_string())),
            tool_call_id: Some(result.call_id.clone()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ChatMessageContent {
    Text(String),
    #[allow(dead_code)]
    Blocks(Vec<ChatTextBlock>),
}

impl ChatMessageContent {
    pub(crate) fn wrap_application_context(&mut self) {
        match self {
            Self::Text(text) => wrap_application_context_text(text),
            Self::Blocks(blocks) => {
                for block in blocks {
                    wrap_application_context_text(&mut block.text);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChatTextBlock {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChatToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    pub(crate) function: Value,
}

impl ChatToolCall {
    fn from_input(call: &GenerateToolCallInput) -> Self {
        Self {
            id: call.call_id.clone(),
            kind: "function",
            function: serde_json::json!({
                "name": call
                    .provider_name
                    .clone()
                    .unwrap_or_else(|| crate::tools::provider_safe_tool_name(&call.name)),
                "arguments": call.arguments.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChatFunctionTool {
    #[serde(rename = "type")]
    kind: &'static str,
    pub(crate) function: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ChatTool {
    Function(ChatFunctionTool),
    OpenRouterWebSearch(ChatHostedWebSearchTool),
}

impl ChatTool {
    pub(crate) fn function(definition: &OpenAiToolDefinition) -> Self {
        let mut function = serde_json::json!({
            "name": definition.name,
            "description": definition.description,
            "parameters": definition.parameters,
        });
        if let Some(strict) = definition.strict {
            function["strict"] = Value::Bool(strict);
        }
        Self::Function(ChatFunctionTool {
            kind: "function",
            function,
        })
    }

    pub(crate) fn openrouter_web_search() -> Self {
        Self::OpenRouterWebSearch(ChatHostedWebSearchTool {
            kind: "openrouter:web_search",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChatHostedWebSearchTool {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct ChatStreamOptions {
    pub(crate) include_usage: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatUsage {
    #[serde(default, alias = "input_tokens")]
    pub(crate) prompt_tokens: u64,
    #[serde(default, alias = "output_tokens")]
    pub(crate) completion_tokens: u64,
    #[serde(default)]
    pub(crate) total_tokens: u64,
    #[serde(default)]
    pub(crate) prompt_tokens_details: Option<ChatPromptTokenDetails>,
    #[serde(default)]
    pub(crate) server_tool_use: Option<ChatServerToolUse>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatPromptTokenDetails {
    #[serde(default)]
    pub(crate) cached_tokens: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ChatServerToolUse {
    #[serde(default)]
    pub(crate) web_search_requests: u64,
}

fn messages_from_generate(input: &GenerateInput, instructions: Option<&str>) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
        messages.push(ChatMessage::text(GenerateMessageRole::System, instructions));
    }

    match input {
        GenerateInput::Text(text) => {
            if !text.trim().is_empty() {
                messages.push(ChatMessage::text(GenerateMessageRole::User, text));
            }
        }
        GenerateInput::Messages(input) => {
            messages.extend(input.iter().filter_map(message_from_generate));
        }
        GenerateInput::Items(items) => append_items(&mut messages, items),
        GenerateInput::NativeToolResults(results) => {
            for result in results {
                let call = GenerateToolCallInput {
                    id: result.id.clone(),
                    call_id: result.call_id.clone(),
                    name: result.name.clone(),
                    provider_name: result.provider_name.clone(),
                    arguments: result.arguments.clone(),
                };
                messages.push(ChatMessage::assistant_tool_call(&call));
                messages.push(ChatMessage::tool_result(result));
            }
        }
    }
    messages
}

fn message_from_generate(message: &GenerateMessage) -> Option<ChatMessage> {
    (!message.content.trim().is_empty())
        .then(|| ChatMessage::text(message.role, message.content.clone()))
}

fn append_items(messages: &mut Vec<ChatMessage>, items: &[GenerateInputItem]) {
    let mut pending_reasoning = Vec::new();
    for item in items {
        match item {
            GenerateInputItem::Reasoning(reasoning) => {
                pending_reasoning.extend(reasoning_details(reasoning));
            }
            GenerateInputItem::Message(message) => {
                if message.role != GenerateMessageRole::Assistant && !pending_reasoning.is_empty() {
                    push_reasoning_message(messages, &mut pending_reasoning);
                }
                if let Some(mut chat_message) = message_from_generate(message) {
                    if message.role == GenerateMessageRole::Assistant {
                        chat_message.reasoning_details = std::mem::take(&mut pending_reasoning);
                    }
                    messages.push(chat_message);
                }
            }
            GenerateInputItem::ToolCall(call) => {
                if !pending_reasoning.is_empty() {
                    push_reasoning_message(messages, &mut pending_reasoning);
                }
                if let Some(last) = messages.last_mut()
                    && last.role == "assistant"
                {
                    last.tool_calls.push(ChatToolCall::from_input(call));
                } else {
                    messages.push(ChatMessage::assistant_tool_call(call));
                }
            }
            GenerateInputItem::ToolResult(result) => {
                if !pending_reasoning.is_empty() {
                    push_reasoning_message(messages, &mut pending_reasoning);
                }
                messages.push(ChatMessage::tool_result(result));
            }
        }
    }
    if !pending_reasoning.is_empty() {
        push_reasoning_message(messages, &mut pending_reasoning);
    }
}

fn push_reasoning_message(messages: &mut Vec<ChatMessage>, details: &mut Vec<Value>) {
    messages.push(ChatMessage {
        role: "assistant".to_string(),
        reasoning_details: std::mem::take(details),
        ..ChatMessage::default()
    });
}

fn reasoning_details(reasoning: &GenerateReasoningInput) -> Vec<Value> {
    if let Some(details) = reasoning
        .provider_details
        .as_ref()
        .filter(|details| !details.is_empty())
    {
        return details.clone();
    }
    if reasoning.encrypted_content.trim().is_empty() {
        return Vec::new();
    }
    let mut detail = serde_json::json!({
        "type": "reasoning.encrypted",
        "data": reasoning.encrypted_content,
    });
    if let Some(id) = reasoning.id.as_ref().filter(|id| !id.trim().is_empty()) {
        detail["id"] = Value::String(id.clone());
    }
    vec![detail]
}

fn chat_tool_choice(
    choice: &NoemaToolChoice,
    tool_names: &OpenAiToolNameMap,
) -> Result<Option<&'static str>, ProviderError> {
    if tool_names.definitions.is_empty() {
        return match choice {
            NoemaToolChoice::Allowed(_) => Err(ProviderError::InvalidRequest {
                message: "allowed tools require a non-empty tool catalog".to_string(),
            }),
            _ => Ok(None),
        };
    }
    match choice {
        NoemaToolChoice::Auto => Ok(Some("auto")),
        NoemaToolChoice::None => Ok(Some("none")),
        NoemaToolChoice::Required => Ok(Some("required")),
        NoemaToolChoice::Allowed(_) => Err(ProviderError::InvalidRequest {
            message: "allowed tools are not supported by Chat Completions".to_string(),
        }),
    }
}

fn wrap_application_context_text(content: &mut String) {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    *content = format!("<noema_application_context>\n{escaped}\n</noema_application_context>");
}
