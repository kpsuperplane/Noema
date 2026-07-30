use std::collections::{BTreeMap, HashSet};

use crate::response_support::sse::{SseEvent, next_sse_event_boundary, parse_sse_event_bytes};
use crate::{GenerateStreamEvent, ProviderError};
use serde_json::Value;

use super::output::{
    ChatChoice, ChatCompletionResponse, ChatOutputFunction, ChatOutputMessage, ChatOutputToolCall,
};
use super::request::ChatUsage;

pub(crate) struct ChatSseAccumulator {
    pending: Vec<u8>,
    response_id: Option<String>,
    model: Option<String>,
    choice: AccumChoice,
    usage: Option<ChatUsage>,
    terminal_error: Option<Value>,
    started_hosted_searches: HashSet<String>,
}

#[derive(Default)]
struct AccumChoice {
    text: String,
    tool_calls: BTreeMap<usize, AccumToolCall>,
    reasoning_details: Vec<Value>,
    annotations: Vec<Value>,
}

#[derive(Default)]
struct AccumToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    started: bool,
}

impl ChatSseAccumulator {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::new(),
            response_id: None,
            model: None,
            choice: AccumChoice::default(),
            usage: None,
            terminal_error: None,
            started_hosted_searches: HashSet::new(),
        }
    }

    pub(crate) fn push_bytes(
        &mut self,
        chunk: &[u8],
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        self.pending.extend_from_slice(chunk);
        while let Some((index, delimiter_len)) = next_sse_event_boundary(&self.pending) {
            let raw = self.pending[..index].to_vec();
            self.pending.drain(..index + delimiter_len);
            let event = parse_sse_event_bytes(&raw)?;
            self.handle_event(event, on_event)?;
        }
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ChatCompletionResponse, ProviderError> {
        if !self.pending.is_empty() {
            let raw = std::mem::take(&mut self.pending);
            let event = parse_sse_event_bytes(&raw)?;
            self.handle_event(event, on_event)?;
        }
        if let Some(error) = self.terminal_error {
            return Err(ProviderError::ApiError {
                status: 200,
                message: response_stream_error_message(&error),
                request_id: self.response_id,
            });
        }

        let choice = self.choice;
        let choices = vec![ChatChoice {
            message: ChatOutputMessage {
                content: (!choice.text.is_empty()).then_some(choice.text),
                tool_calls: choice
                    .tool_calls
                    .into_values()
                    .map(|call| ChatOutputToolCall {
                        id: call.id,
                        function: ChatOutputFunction {
                            name: call.name,
                            arguments: (!call.arguments.is_empty()).then_some(call.arguments),
                        },
                    })
                    .collect(),
                reasoning_details: choice.reasoning_details,
                annotations: choice.annotations,
            },
        }];
        Ok(ChatCompletionResponse {
            id: self.response_id,
            model: self.model,
            choices,
            usage: self.usage,
        })
    }

    fn handle_event(
        &mut self,
        event: SseEvent,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        let Some(data) = event.data else {
            return Ok(());
        };
        if data == "[DONE]" {
            return Ok(());
        }
        let value: Value = serde_json::from_str(&data).map_err(|source| {
            let message = format!("failed to parse Chat SSE JSON: {source}");
            ProviderError::MalformedResponse { message }
        })?;
        if value.get("error").is_some()
            || event.event.as_deref() == Some("error")
            || value.get("type").and_then(Value::as_str) == Some("error")
        {
            self.terminal_error = Some(value);
            return Ok(());
        }
        self.response_id.get_or_insert_with(|| {
            value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        });
        if self.response_id.as_deref() == Some("") {
            self.response_id = None;
        }
        self.model.get_or_insert_with(|| {
            value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        });
        if self.model.as_deref() == Some("") {
            self.model = None;
        }
        if let Some(usage) = value.get("usage").cloned() {
            self.usage = serde_json::from_value(usage).ok();
        }
        self.capture_response(&value, on_event)
    }

    fn capture_response(
        &mut self,
        value: &Value,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        let Some(choices) = value.get("choices").and_then(Value::as_array) else {
            if let Some(details) = value.get("reasoning_details") {
                self.handle_reasoning_details(details, on_event);
            }
            return Ok(());
        };
        for choice in choices {
            if choice
                .get("index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index != 0)
            {
                continue;
            }
            if let Some(delta) = choice.get("delta") {
                self.handle_message_delta(delta, on_event)?;
            }
            if let Some(message) = choice.get("message") {
                self.handle_message_delta(message, on_event)?;
            }
        }
        Ok(())
    }

    fn handle_message_delta(
        &mut self,
        delta: &Value,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            self.choice.text.push_str(content);
            on_event(GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: content.to_string(),
            });
        }
        if let Some(details) = delta.get("reasoning_details") {
            self.handle_reasoning_details(details, on_event);
            if let Some(details) = details.as_array() {
                self.append_reasoning_details(details);
            }
        }
        if let Some(annotations) = delta.get("annotations") {
            if let Some(annotations) = annotations.as_array() {
                self.choice.annotations.extend(annotations.iter().cloned());
            } else {
                self.choice.annotations.push(annotations.clone());
            }
        }
        let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else {
            return Ok(());
        };
        for (position, fragment) in tool_calls.iter().enumerate() {
            let index = fragment
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or(position);
            let started_name = {
                let call = self.choice.tool_calls.entry(index).or_default();
                if let Some(id) = fragment.get("id").and_then(Value::as_str) {
                    call.id.get_or_insert_with(|| id.to_string());
                }
                if let Some(function) = fragment.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        call.name.get_or_insert_with(|| name.to_string());
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        call.arguments.push_str(arguments);
                    }
                }
                if !call.started {
                    call.name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                        .map(|name| {
                            call.started = true;
                            name.to_string()
                        })
                } else {
                    None
                }
            };
            if let Some(name) = started_name {
                on_event(GenerateStreamEvent::ToolCallStarted {
                    output_index: index,
                    name,
                });
            }
        }
        Ok(())
    }

    fn handle_reasoning_details(
        &mut self,
        value: &Value,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        for detail in value.as_array().into_iter().flatten() {
            let Some(object) = detail.as_object() else {
                continue;
            };
            if object.get("type").and_then(Value::as_str) != Some("reasoning.server_tool_call") {
                continue;
            }
            let key = object
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("0:{}", self.started_hosted_searches.len()));
            if self.started_hosted_searches.insert(key.clone()) {
                on_event(GenerateStreamEvent::HostedWebSearchStarted {
                    output_index: 0,
                    id: Some(key),
                });
            }
        }
    }

    fn append_reasoning_details(&mut self, fragments: &[Value]) {
        for fragment in fragments {
            let Some(fragment_object) = fragment.as_object() else {
                self.choice.reasoning_details.push(fragment.clone());
                continue;
            };
            let fragment_type = fragment_object.get("type").and_then(Value::as_str);
            let mergeable = matches!(
                fragment_type,
                Some("reasoning.text" | "reasoning.encrypted" | "reasoning.summary")
            );
            let fragment_index = fragment_object.get("index").and_then(Value::as_u64);
            let fragment_id = fragment_object.get("id").and_then(Value::as_str);
            let existing = mergeable.then(|| {
                self.choice.reasoning_details.iter_mut().find(|detail| {
                    let Some(object) = detail.as_object() else {
                        return false;
                    };
                    object.get("type").and_then(Value::as_str) == fragment_type
                        && ((fragment_index.is_some()
                            && object.get("index").and_then(Value::as_u64) == fragment_index)
                            || (fragment_id.is_some()
                                && object.get("id").and_then(Value::as_str) == fragment_id))
                })
            });
            if let Some(existing) = existing.flatten() {
                merge_reasoning_fragment(existing, fragment_object);
            } else {
                self.choice.reasoning_details.push(fragment.clone());
            }
        }
    }
}

fn response_stream_error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.pointer("/error/message").and_then(Value::as_str))
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string())
}

fn merge_reasoning_fragment(target: &mut Value, fragment: &serde_json::Map<String, Value>) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    for key in ["text", "data", "summary"] {
        let Some(Value::String(fragment)) = fragment.get(key) else {
            continue;
        };
        match target.get_mut(key) {
            Some(Value::String(existing)) => existing.push_str(fragment),
            _ => {
                target.insert(key.to_string(), Value::String(fragment.clone()));
            }
        }
    }
    for (key, value) in fragment {
        target.entry(key.clone()).or_insert_with(|| value.clone());
    }
}
