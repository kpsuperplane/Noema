use serde::Serialize;
use serde_json::Value;

use crate::response_support::tool_names::OpenAiToolDefinition;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessageRole, GenerateRequest, GenerateToolCallInput,
    GenerateToolResultInput, NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice,
    ProviderError, ProviderSchemaRequest, ProviderTool, chat_completions::OpenAiToolNameMap,
};

#[derive(Debug, Serialize)]
pub(super) struct ChatCompletionRequest {
    model: String,
    pub(super) messages: Vec<ChatMessage>,
    stream: bool,
    stream_options: ChatStreamOptions,
    pub(super) cache_prompt: bool,
    pub(super) chat_template_kwargs: ChatTemplateKwargs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_choice: Option<ChatToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) parallel_tool_calls: Option<bool>,
}

impl ChatCompletionRequest {
    pub(super) fn from_generate(
        request: &GenerateRequest,
        model: String,
    ) -> Result<(Self, OpenAiToolNameMap), ProviderError> {
        let mut messages = Vec::new();
        if let Some(instructions) = request
            .instructions
            .as_deref()
            .filter(|instructions| !instructions.trim().is_empty())
        {
            push_chat_message(&mut messages, "system", instructions);
        }
        append_generate_input(&mut messages, &request.input);
        if messages.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "local model request must contain model-visible input".to_string(),
            });
        }
        let selected_tools = selected_local_tools(request)?;
        let tool_names = OpenAiToolNameMap::from_tools_with_request(
            &selected_tools
                .iter()
                .map(|tool| (*tool).clone())
                .collect::<Vec<_>>(),
            ProviderSchemaRequest::Send,
        )?;
        let tools = (!tool_names.definitions.is_empty()).then(|| {
            tool_names
                .definitions
                .iter()
                .map(ChatTool::from_definition)
                .collect::<Vec<_>>()
        });
        let tool_choice = tools
            .as_ref()
            .map(|_| chat_tool_choice(&request.tool_choice));
        Ok((
            Self {
                model,
                messages,
                stream: true,
                stream_options: ChatStreamOptions {
                    include_usage: true,
                },
                cache_prompt: true,
                chat_template_kwargs: ChatTemplateKwargs {
                    enable_thinking: false,
                },
                max_tokens: request.options.max_output_tokens,
                temperature: request.options.temperature,
                parallel_tool_calls: tools.as_ref().map(|_| request.parallel_tool_calls),
                tools,
                tool_choice,
            },
            tool_names,
        ))
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ChatTemplateKwargs {
    pub(super) enable_thinking: bool,
}

fn selected_local_tools(request: &GenerateRequest) -> Result<Vec<&ProviderTool>, ProviderError> {
    match &request.tool_choice {
        NoemaToolChoice::None => Ok(Vec::new()),
        NoemaToolChoice::Required if request.tools.is_empty() => {
            Err(ProviderError::InvalidRequest {
                message: "required tool choice needs a non-empty tool catalog".to_string(),
            })
        }
        NoemaToolChoice::Auto | NoemaToolChoice::Required => Ok(request.tools.iter().collect()),
        NoemaToolChoice::Allowed(allowed) => {
            if allowed.tools.is_empty() {
                return Err(ProviderError::InvalidRequest {
                    message: "allowed tools cannot be empty".to_string(),
                });
            }
            let mut seen = std::collections::HashSet::with_capacity(allowed.tools.len());
            let mut selected = Vec::with_capacity(allowed.tools.len());
            for allowed_name in &allowed.tools {
                if !seen.insert(allowed_name.as_str()) {
                    return Err(ProviderError::InvalidRequest {
                        message: format!("allowed tool {allowed_name} is duplicated"),
                    });
                }
                let Some(tool) = request
                    .tools
                    .iter()
                    .find(|tool| &tool.canonical_spec().name == allowed_name)
                else {
                    return Err(ProviderError::InvalidRequest {
                        message: format!(
                            "allowed tool {allowed_name} is not present in the request tool catalog"
                        ),
                    });
                };
                selected.push(tool);
            }
            Ok(selected)
        }
    }
}

fn chat_tool_choice(choice: &NoemaToolChoice) -> ChatToolChoice {
    match choice {
        NoemaToolChoice::None => ChatToolChoice::Mode("none"),
        NoemaToolChoice::Auto
        | NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Auto,
            ..
        }) => ChatToolChoice::Mode("auto"),
        NoemaToolChoice::Required
        | NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Required,
            ..
        }) => ChatToolChoice::Mode("required"),
    }
}

fn normalize_llama_cpp_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            // llama.cpp lowers JSON Schema string constraints into grammar
            // productions. Large length bounds make the generated grammar too
            // large, while otherwise-valid expressions such as `\S` can make
            // b10015 reject the grammar entirely. Runtime tool handlers still
            // enforce the canonical schema after generation.
            object.remove("minLength");
            object.remove("maxLength");
            object.remove("pattern");
            for child in object.values_mut() {
                normalize_llama_cpp_schema(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize_llama_cpp_schema(child);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Serialize)]
struct ChatStreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct ChatMessage {
    pub(super) role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_call_id: Option<String>,
}

impl ChatMessage {
    fn new(role: &'static str, content: impl Into<String>) -> Self {
        Self {
            role,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn assistant_tool_calls(tool_calls: Vec<ChatToolCall>) -> Self {
        Self {
            role: "assistant",
            content: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    fn tool_result(result: &GenerateToolResultInput) -> Self {
        Self {
            role: "tool",
            content: Some(tool_result_content(result)),
            tool_calls: None,
            tool_call_id: Some(result.call_id.clone()),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ChatToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: ChatFunctionCall,
}

impl From<&GenerateToolCallInput> for ChatToolCall {
    fn from(call: &GenerateToolCallInput) -> Self {
        Self {
            id: call.call_id.clone(),
            kind: "function",
            function: ChatFunctionCall {
                name: call
                    .provider_name
                    .clone()
                    .unwrap_or_else(|| call.name.clone()),
                arguments: call.arguments.to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ChatTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: ChatFunction,
}

impl ChatTool {
    fn from_definition(definition: &OpenAiToolDefinition) -> Self {
        let mut parameters = definition.parameters.clone();
        normalize_llama_cpp_schema(&mut parameters);
        Self {
            kind: "function",
            function: ChatFunction {
                name: definition.name.clone(),
                description: definition.description.clone(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum ChatToolChoice {
    Mode(&'static str),
}

fn append_generate_input(messages: &mut Vec<ChatMessage>, input: &GenerateInput) {
    match input {
        GenerateInput::Text(text) => push_chat_message(messages, "user", text),
        GenerateInput::Messages(input_messages) => {
            for message in input_messages {
                push_chat_message(
                    messages,
                    match message.role {
                        GenerateMessageRole::System | GenerateMessageRole::Developer => "system",
                        GenerateMessageRole::User => "user",
                        GenerateMessageRole::Assistant => "assistant",
                    },
                    &message.content,
                );
            }
        }
        GenerateInput::Items(items) => {
            let mut pending_tool_calls = Vec::new();
            for item in items {
                match item {
                    GenerateInputItem::Message(message) => {
                        flush_tool_calls(messages, &mut pending_tool_calls);
                        push_chat_message(
                            messages,
                            match message.role {
                                GenerateMessageRole::System | GenerateMessageRole::Developer => {
                                    "system"
                                }
                                GenerateMessageRole::User => "user",
                                GenerateMessageRole::Assistant => "assistant",
                            },
                            &message.content,
                        );
                    }
                    GenerateInputItem::AssistantText(message) => {
                        flush_tool_calls(messages, &mut pending_tool_calls);
                        push_chat_message(messages, "assistant", &message.content);
                    }
                    GenerateInputItem::Reasoning(_) => {
                        flush_tool_calls(messages, &mut pending_tool_calls);
                        push_chat_message(messages, "assistant", item.render_for_token_count());
                    }
                    GenerateInputItem::ToolCall(call) => {
                        pending_tool_calls.push(ChatToolCall::from(call));
                    }
                    GenerateInputItem::ToolResult(result) => {
                        flush_tool_calls(messages, &mut pending_tool_calls);
                        messages.push(ChatMessage::tool_result(result));
                    }
                    GenerateInputItem::HostedWebSearch(_) => {
                        flush_tool_calls(messages, &mut pending_tool_calls);
                        push_chat_message(messages, "assistant", item.render_for_token_count());
                    }
                }
            }
            flush_tool_calls(messages, &mut pending_tool_calls);
        }
        GenerateInput::NativeToolResults(results) => {
            let mut calls = results
                .iter()
                .map(|result| ChatToolCall {
                    id: result.call_id.clone(),
                    kind: "function",
                    function: ChatFunctionCall {
                        name: result
                            .provider_name
                            .clone()
                            .unwrap_or_else(|| result.name.clone()),
                        arguments: result.arguments.to_string(),
                    },
                })
                .collect();
            flush_tool_calls(messages, &mut calls);
            for result in results {
                messages.push(ChatMessage::tool_result(result));
            }
        }
    }
}

fn flush_tool_calls(messages: &mut Vec<ChatMessage>, tool_calls: &mut Vec<ChatToolCall>) {
    if !tool_calls.is_empty() {
        messages.push(ChatMessage::assistant_tool_calls(std::mem::take(
            tool_calls,
        )));
    }
}

fn tool_result_content(result: &GenerateToolResultInput) -> String {
    serde_json::json!({
        "call_id": result.call_id,
        "name": result.name,
        "provider_name": result.provider_name,
        "success": result.success,
        "payload": result.payload,
    })
    .to_string()
}

fn push_chat_message(
    messages: &mut Vec<ChatMessage>,
    role: &'static str,
    content: impl Into<String>,
) {
    let content = content.into();
    if role == "system"
        && let Some(previous) = messages.last_mut()
        && previous.role == "system"
        && let Some(previous_content) = previous.content.as_mut()
    {
        previous_content.push_str("\n\n");
        previous_content.push_str(&content);
        return;
    }
    messages.push(ChatMessage::new(role, content));
}
