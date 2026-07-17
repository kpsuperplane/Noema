use serde::Serialize;
use serde_json::Value;

use crate::{
    GenerateInput, GenerateInputItem, GenerateMessageRole, GenerateRequest, NoemaAllowedTools,
    NoemaAllowedToolsMode, NoemaToolChoice, ProviderError,
    response_support::noema_response_text_format,
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
    pub(super) response_format: Option<Value>,
}

impl ChatCompletionRequest {
    pub(super) fn from_generate(
        request: &GenerateRequest,
        model: String,
    ) -> Result<Self, ProviderError> {
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
        let response_format = request
            .options
            .require_noema_response
            .then(|| chat_noema_response_format(request))
            .transpose()?;
        Ok(Self {
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
            response_format,
        })
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ChatTemplateKwargs {
    pub(super) enable_thinking: bool,
}

fn chat_noema_response_format(request: &GenerateRequest) -> Result<Value, ProviderError> {
    let format = noema_response_text_format()
        .get("format")
        .cloned()
        .expect("shared Noema response format");
    let mut response_format = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": format["name"],
            "strict": format["strict"],
            "schema": format["schema"]
        }
    });
    let selected_tools = selected_local_tools(request)?;
    let tools_allowed = !selected_tools.is_empty();
    let schema = &mut response_format["json_schema"]["schema"];
    if tools_allowed {
        let tool_schemas = selected_tools
            .into_iter()
            .map(|tool| {
                let mut input_schema = tool.input_schema.as_value().clone();
                normalize_llama_cpp_schema(&mut input_schema);
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": ["string", "null"]},
                        "name": {"type": "string", "enum": [tool.name.as_str()]},
                        "payload": input_schema
                    },
                    "required": ["name", "payload"],
                    "additionalProperties": false
                })
            })
            .collect::<Vec<_>>();
        schema["properties"]["tool_calls"]["items"] = serde_json::json!({"oneOf": tool_schemas});
    }
    if !tools_allowed {
        schema["properties"]["response_status"]["enum"] = serde_json::json!(["final"]);
        schema["properties"]["responses"]["minItems"] = serde_json::json!(1);
        schema["properties"]["tool_calls"]["maxItems"] = serde_json::json!(0);
    }
    let tool_call_required = matches!(
        &request.tool_choice,
        NoemaToolChoice::Required
            | NoemaToolChoice::Allowed(NoemaAllowedTools {
                mode: NoemaAllowedToolsMode::Required,
                ..
            })
    );
    if tool_call_required {
        schema["properties"]["response_status"]["enum"] = serde_json::json!(["needs_tools"]);
        schema["properties"]["responses"]["maxItems"] = serde_json::json!(0);
        schema["properties"]["tool_calls"]["minItems"] = serde_json::json!(1);
    }
    if tools_allowed && !request.parallel_tool_calls {
        schema["properties"]["tool_calls"]["maxItems"] = serde_json::json!(1);
    }
    Ok(response_format)
}

fn selected_local_tools(
    request: &GenerateRequest,
) -> Result<Vec<&noema_capabilities::ToolSpec>, ProviderError> {
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
                let Some(tool) = request.tools.iter().find(|tool| &tool.name == allowed_name)
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
    pub(super) content: String,
}

impl ChatMessage {
    fn new(role: &'static str, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
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
            for item in items {
                match item {
                    GenerateInputItem::Message(message) => push_chat_message(
                        messages,
                        match message.role {
                            GenerateMessageRole::System | GenerateMessageRole::Developer => {
                                "system"
                            }
                            GenerateMessageRole::User => "user",
                            GenerateMessageRole::Assistant => "assistant",
                        },
                        &message.content,
                    ),
                    GenerateInputItem::Reasoning(_) | GenerateInputItem::ToolCall(_) => {
                        push_chat_message(messages, "assistant", item.render_for_token_count());
                    }
                    GenerateInputItem::ToolResult(_) => {
                        push_chat_message(messages, "user", item.render_for_token_count());
                    }
                }
            }
        }
        GenerateInput::NativeToolResults(_) => {
            push_chat_message(messages, "user", input.render_for_token_count());
        }
    }
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
    {
        previous.content.push_str("\n\n");
        previous.content.push_str(&content);
        return;
    }
    messages.push(ChatMessage::new(role, content));
}
