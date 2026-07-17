//! OpenAI-compatible request and response translation.

use std::time::SystemTime;

use noema_capabilities::ToolSpec;
use noema_providers::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateRequest, GenerateResponse, GenerateResponseStatus, GenerateToolCall,
    GenerateToolCallInput, GenerateToolResultInput, GenerationPriority, NoemaToolChoice,
    ProviderRouteLease,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::MemoryModelProxyError;

#[derive(Debug, Deserialize)]
pub(super) struct OpenAiChatCompletionRequest {
    model: Option<String>,
    messages: Vec<OpenAiChatMessage>,
    #[serde(default)]
    tools: Vec<OpenAiChatTool>,
    #[serde(default)]
    tool_choice: Option<OpenAiToolChoice>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    pub(super) stream: Option<bool>,
}

impl OpenAiChatCompletionRequest {
    pub(super) fn into_generate_request(
        self,
        route: &ProviderRouteLease,
    ) -> Result<GenerateRequest, MemoryModelProxyError> {
        let _requested_model = self.model;
        let _ignored_temperature = self.temperature;
        let mut instructions = Vec::new();
        let mut items = Vec::new();
        for message in self.messages {
            match message.role.as_str() {
                "system" | "developer" => {
                    if let Some(content) = message.content_text() {
                        instructions.push(content);
                    }
                }
                "user" => {
                    if let Some(content) = message.content_text() {
                        items.push(GenerateInputItem::Message(GenerateMessage {
                            role: GenerateMessageRole::User,
                            content,
                        }));
                    }
                }
                "assistant" => {
                    if let Some(content) = message.content_text() {
                        items.push(GenerateInputItem::Message(GenerateMessage {
                            role: GenerateMessageRole::Assistant,
                            content,
                        }));
                    }
                    for tool_call in message.tool_calls {
                        items.push(GenerateInputItem::ToolCall(GenerateToolCallInput {
                            id: None,
                            call_id: tool_call.id,
                            name: tool_call.function.name.clone(),
                            provider_name: Some(tool_call.function.name),
                            arguments: parse_arguments_json(&tool_call.function.arguments)?,
                        }));
                    }
                }
                "tool" => {
                    let content = message.content_text();
                    let Some(tool_call_id) = message.tool_call_id else {
                        return Err(MemoryModelProxyError::Protocol(
                            "tool message is missing tool_call_id".to_string(),
                        ));
                    };
                    let name = message.name.unwrap_or_else(|| "tool".to_string());
                    items.push(GenerateInputItem::ToolResult(GenerateToolResultInput {
                        id: None,
                        call_id: tool_call_id,
                        name: name.clone(),
                        provider_name: Some(name),
                        arguments: Value::Null,
                        success: true,
                        payload: content.map_or(Value::Null, |content| json!({"content": content})),
                    }));
                }
                role => {
                    return Err(MemoryModelProxyError::Protocol(format!(
                        "unsupported message role: {role}"
                    )));
                }
            }
        }

        let tools = self
            .tools
            .into_iter()
            .map(OpenAiChatTool::into_noema_tool)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(GenerateRequest {
            conversation_id: None,
            model: Some(resolved_model_profile(route)),
            input: GenerateInput::Items(items),
            instructions: nonempty_join(instructions, "\n\n"),
            options: GenerateOptions {
                generation_priority: GenerationPriority::Background,
                max_output_tokens: self.max_completion_tokens.or(self.max_tokens),
                // OpenAI-compatible memory clients often send sampling knobs that
                // are not valid for every configured Noema provider/model.
                temperature: None,
                reasoning_effort: route.selection().reasoning_effort,
                require_noema_response: false,
                prompt_cache_retention: None,
                ..GenerateOptions::default()
            },
            tools,
            tool_choice: self.tool_choice.map_or(NoemaToolChoice::Auto, Into::into),
            parallel_tool_calls: true,
        })
    }
}

pub(super) fn resolved_model_profile(route: &ProviderRouteLease) -> String {
    route
        .selection()
        .model_profile
        .clone()
        .or_else(|| route.operations().default_tool_classification_model())
        .unwrap_or_else(|| noema_providers::DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string())
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMessage {
    role: String,
    #[serde(default)]
    content: Option<OpenAiMessageContent>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

impl OpenAiChatMessage {
    fn content_text(&self) -> Option<String> {
        match self.content.as_ref()? {
            OpenAiMessageContent::Text(text) => nonempty_string(text),
            OpenAiMessageContent::Parts(parts) => nonempty_join(
                parts
                    .iter()
                    .filter_map(|part| match part {
                        OpenAiContentPart::Text { text } => nonempty_string(text),
                    })
                    .collect::<Vec<_>>(),
                "\n",
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenAiMessageContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAiContentPart {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Deserialize)]
struct OpenAiChatTool {
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiFunctionSpec,
}

impl OpenAiChatTool {
    fn into_noema_tool(self) -> Result<ToolSpec, MemoryModelProxyError> {
        if self.kind != "function" {
            return Err(MemoryModelProxyError::Protocol(format!(
                "unsupported tool type: {}",
                self.kind
            )));
        }
        ToolSpec::new(
            self.function.name,
            self.function.description,
            self.function.parameters,
        )
        .map_err(|error| MemoryModelProxyError::Protocol(error.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionSpec {
    name: String,
    description: String,
    #[serde(default = "default_object_schema")]
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenAiToolChoice {
    Mode(String),
    Function {
        #[serde(rename = "type")]
        kind: String,
        function: OpenAiToolChoiceFunction,
    },
}

impl From<OpenAiToolChoice> for NoemaToolChoice {
    fn from(choice: OpenAiToolChoice) -> Self {
        match choice {
            OpenAiToolChoice::Mode(mode) if mode == "none" => Self::None,
            OpenAiToolChoice::Mode(mode) if mode == "required" => Self::Required,
            OpenAiToolChoice::Mode(_) => Self::Auto,
            OpenAiToolChoice::Function { kind, function } => {
                let _ = (kind, function.name);
                Self::Required
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiToolChoiceFunction {
    name: String,
}

pub(super) fn openai_response_from_generate_response(response: GenerateResponse) -> Value {
    let has_tools =
        response.response_status == GenerateResponseStatus::NeedsTools || response.has_tool_calls();
    let finish_reason = if has_tools { "tool_calls" } else { "stop" };
    let id = response
        .response_id
        .clone()
        .unwrap_or_else(|| format!("chatcmpl_{}", unix_timestamp()));
    let content = response.assistant_text();
    let model = response.model.clone();
    let tool_calls = openai_tool_calls(response.tool_calls);
    let usage = response.usage.map(|usage| {
        json!({
            "prompt_tokens": usage.input_tokens,
            "completion_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
        })
    });
    json!({
        "id": id,
        "object": "chat.completion",
        "created": unix_timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content,
                "tool_calls": tool_calls,
            },
            "finish_reason": finish_reason
        }],
        "usage": usage.unwrap_or(Value::Null)
    })
}

fn openai_tool_calls(calls: Vec<GenerateToolCall>) -> Value {
    if calls.is_empty() {
        return Value::Null;
    }
    Value::Array(
        calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| {
                let id = call
                    .provider_call_id
                    .clone()
                    .or(call.id.clone())
                    .unwrap_or_else(|| format!("call_{index}"));
                let name = call.provider_name.unwrap_or(call.name);
                json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": call.payload.to_string(),
                    }
                })
            })
            .collect(),
    )
}

fn parse_arguments_json(arguments: &str) -> Result<Value, MemoryModelProxyError> {
    if arguments.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(arguments).map_err(|error| {
        MemoryModelProxyError::Protocol(format!("invalid tool call arguments: {error}"))
    })
}

fn default_object_schema() -> Value {
    json!({"type": "object", "properties": {}})
}

fn nonempty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn nonempty_join(values: Vec<String>, separator: &str) -> Option<String> {
    let value = values
        .into_iter()
        .filter_map(|value| nonempty_string(&value))
        .collect::<Vec<_>>()
        .join(separator);
    nonempty_string(&value)
}

pub(super) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
