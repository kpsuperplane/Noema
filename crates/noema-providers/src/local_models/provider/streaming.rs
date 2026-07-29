use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{GenerateToolCall, ProviderError, TokenUsage};

#[derive(Debug, Default)]
pub(super) struct ChatSseAccumulator {
    pending: Vec<u8>,
    text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<TokenUsage>,
    tool_calls: BTreeMap<usize, PartialToolCall>,
}

#[derive(Debug)]
pub(super) struct ChatStreamOutput {
    pub(super) text: String,
    pub(super) tool_calls: Vec<GenerateToolCall>,
    pub(super) response_id: Option<String>,
    pub(super) model: Option<String>,
    pub(super) usage: Option<TokenUsage>,
}

#[derive(Debug)]
pub(super) enum ChatStreamEvent {
    AssistantTextDelta(String),
    ToolCallStarted { output_index: usize, name: String },
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
    started: bool,
}

impl ChatSseAccumulator {
    pub(super) fn push_bytes(
        &mut self,
        bytes: &[u8],
        mut on_event: impl FnMut(ChatStreamEvent),
    ) -> Result<(), ProviderError> {
        self.pending.extend_from_slice(bytes);
        while let Some((index, delimiter_len)) = next_event_boundary(&self.pending) {
            let raw = self.pending[..index].to_vec();
            self.pending.drain(..index + delimiter_len);
            self.handle_event(&raw, &mut on_event)?;
        }
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        mut on_event: impl FnMut(ChatStreamEvent),
    ) -> Result<ChatStreamOutput, ProviderError> {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.handle_event(&pending, &mut on_event)?;
        }
        if self.text.trim().is_empty() && self.tool_calls.is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "local model produced empty output".to_string(),
            });
        }
        let mut provider_call_ids = HashSet::new();
        let mut tool_calls = Vec::with_capacity(self.tool_calls.len());
        for (output_index, call) in self.tool_calls {
            let call = call.finish(output_index)?;
            let Some(provider_call_id) = call.provider_call_id.as_deref() else {
                return Err(ProviderError::MalformedResponse {
                    message: format!(
                        "local native tool call {output_index} is missing provider_call_id"
                    ),
                });
            };
            if !provider_call_ids.insert(provider_call_id.to_string()) {
                return Err(ProviderError::MalformedResponse {
                    message: format!(
                        "local model returned duplicate provider_call_id `{provider_call_id}` in native tool calls"
                    ),
                });
            }
            tool_calls.push(call);
        }
        Ok(ChatStreamOutput {
            text: self.text,
            tool_calls,
            response_id: self.response_id,
            model: self.model,
            usage: self.usage,
        })
    }

    fn handle_event(
        &mut self,
        raw: &[u8],
        on_event: &mut impl FnMut(ChatStreamEvent),
    ) -> Result<(), ProviderError> {
        let raw = std::str::from_utf8(raw).map_err(|error| ProviderError::MalformedResponse {
            message: format!("local model stream was not UTF-8: {error}"),
        })?;
        let data = raw
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            return Ok(());
        }
        let chunk: ChatCompletionChunk =
            serde_json::from_str(&data).map_err(|error| ProviderError::MalformedResponse {
                message: format!("invalid local model stream event: {error}"),
            })?;
        if self.response_id.is_none() {
            self.response_id = chunk.id;
        }
        if self.model.is_none() {
            self.model = chunk.model;
        }
        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.into());
        }
        for choice in chunk.choices {
            let ChatDelta {
                content,
                tool_calls,
            } = choice.delta;
            if let Some(delta) = content.filter(|delta| !delta.is_empty()) {
                self.text.push_str(&delta);
                on_event(ChatStreamEvent::AssistantTextDelta(delta));
            }
            for tool_call in tool_calls {
                self.append_tool_call(tool_call, on_event);
            }
        }
        Ok(())
    }

    fn append_tool_call(
        &mut self,
        delta: ChatToolCallDelta,
        on_event: &mut impl FnMut(ChatStreamEvent),
    ) {
        let output_index = delta.index;
        let partial = self.tool_calls.entry(output_index).or_default();
        if let Some(id) = delta.id.filter(|id| !id.is_empty()) {
            partial.id = Some(id);
        }
        if let Some(function) = delta.function {
            if let Some(name) = function.name.filter(|name| !name.is_empty()) {
                partial.name.push_str(&name);
            }
            if let Some(arguments) = function.arguments {
                partial.arguments.push_str(&arguments);
            }
        }
        if !partial.started && !partial.name.is_empty() {
            partial.started = true;
            on_event(ChatStreamEvent::ToolCallStarted {
                output_index,
                name: partial.name.clone(),
            });
        }
    }
}

impl PartialToolCall {
    fn finish(self, output_index: usize) -> Result<GenerateToolCall, ProviderError> {
        let id = self.id.filter(|id| !id.trim().is_empty()).ok_or_else(|| {
            ProviderError::MalformedResponse {
                message: format!("local native tool call {output_index} is missing an id"),
            }
        })?;
        if self.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: format!("local native tool call {output_index} is missing a name"),
            });
        }
        let arguments = if self.arguments.trim().is_empty() {
            "{}"
        } else {
            self.arguments.as_str()
        };
        let payload: Value =
            serde_json::from_str(arguments).map_err(|error| ProviderError::MalformedResponse {
                message: format!(
                    "failed to parse local native tool call arguments for {}: {error}",
                    self.name
                ),
            })?;
        if !payload.is_object() {
            return Err(ProviderError::MalformedResponse {
                message: format!(
                    "local native tool call arguments for {} must be a JSON object",
                    self.name
                ),
            });
        }
        Ok(GenerateToolCall {
            id: Some(id.clone()),
            provider_call_id: Some(id),
            provider_name: Some(self.name.clone()),
            name: self.name,
            payload,
        })
    }
}

fn next_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2))
        .or_else(|| {
            bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| (index, 4))
        })
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCallDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChatFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    prompt_tokens_details: Option<ChatPromptTokensDetails>,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ChatPromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

impl From<ChatUsage> for TokenUsage {
    fn from(usage: ChatUsage) -> Self {
        Self {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            cached_input_tokens: usage
                .prompt_tokens_details
                .and_then(|details| details.cached_tokens),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenizeResponse {
    #[serde(default)]
    pub(super) tokens: Option<Vec<Value>>,
    #[serde(default)]
    pub(super) count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_sse_accumulator_streams_text_and_usage_across_chunks() {
        let mut accumulator = ChatSseAccumulator::default();
        let mut events = Vec::new();
        accumulator
            .push_bytes(
                b"data: {\"id\":\"chat-1\",\"model\":\"local-8b\",\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
                |event| events.push(event),
            )
            .expect("first event");
        accumulator
            .push_bytes(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}],\"usage\":{\"prompt_tokens\":3,\"prompt_tokens_details\":{\"cached_tokens\":2},\"completion_tokens\":2,\"total_tokens\":5}}\n\ndata: [DONE]\n\n",
                |event| events.push(event),
            )
            .expect("remaining events");

        let output = accumulator
            .finish(|event| events.push(event))
            .expect("stream output");

        assert!(
            matches!(events[0], ChatStreamEvent::AssistantTextDelta(ref delta) if delta == "hel")
        );
        assert!(
            matches!(events[1], ChatStreamEvent::AssistantTextDelta(ref delta) if delta == "lo")
        );
        assert_eq!(output.text, "hello");
        assert_eq!(output.response_id.as_deref(), Some("chat-1"));
        assert_eq!(output.model.as_deref(), Some("local-8b"));
        assert_eq!(
            output.usage,
            Some(TokenUsage {
                input_tokens: 3,
                output_tokens: 2,
                total_tokens: 5,
                cached_input_tokens: Some(2),
            })
        );
    }

    #[test]
    fn chat_sse_accumulator_reassembles_native_tool_call_arguments() {
        let mut accumulator = ChatSseAccumulator::default();
        let mut events = Vec::new();
        accumulator
            .push_bytes(
                br#"data: {"id":"chat-1","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"search_memory","arguments":"{\"query\":\"tra"}}]}}]}

"#,
                |event| events.push(event),
            )
            .expect("first tool chunk");
        accumulator
            .push_bytes(
                br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ins\"}"}}]}}]}

data: [DONE]

"#,
                |event| events.push(event),
            )
            .expect("remaining tool chunks");

        let output = accumulator
            .finish(|event| events.push(event))
            .expect("tool output");

        assert!(events.iter().any(|event| {
            matches!(event, ChatStreamEvent::ToolCallStarted { output_index: 0, name } if name == "search_memory")
        }));
        assert_eq!(output.tool_calls.len(), 1);
        assert_eq!(
            output.tool_calls[0].provider_call_id.as_deref(),
            Some("call-1")
        );
        assert_eq!(output.tool_calls[0].name, "search_memory");
        assert_eq!(output.tool_calls[0].payload["query"], "trains");
    }

    #[test]
    fn chat_sse_accumulator_rejects_duplicate_parallel_provider_call_ids() {
        let mut accumulator = ChatSseAccumulator::default();
        accumulator
            .push_bytes(
                br#"data: {"id":"chat-1","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-duplicate","type":"function","function":{"name":"search_memory","arguments":"{}"}}]}},{"delta":{"tool_calls":[{"index":1,"id":"call-duplicate","type":"function","function":{"name":"search_memory","arguments":"{}"}}]}}]}

"#,
                |_| {},
            )
            .expect("parallel tool event");

        let error = accumulator
            .finish(|_| {})
            .expect_err("duplicate provider call ids must fail closed");
        assert!(error.to_string().contains("duplicate provider_call_id"));
    }

    #[test]
    fn chat_usage_without_prompt_token_details_leaves_cache_usage_unknown() {
        let usage = serde_json::from_value::<ChatUsage>(serde_json::json!({
            "prompt_tokens": 3,
            "completion_tokens": 2,
            "total_tokens": 5
        }))
        .expect("usage");

        assert_eq!(
            TokenUsage::from(usage),
            TokenUsage {
                input_tokens: 3,
                output_tokens: 2,
                total_tokens: 5,
                cached_input_tokens: None,
            }
        );
    }

    #[test]
    fn chat_sse_accumulator_rejects_empty_output() {
        let mut accumulator = ChatSseAccumulator::default();
        accumulator
            .push_bytes(b"data: [DONE]\n\n", |_| {})
            .expect("done event");

        assert!(matches!(
            accumulator.finish(|_| {}),
            Err(ProviderError::MalformedResponse { .. })
        ));
    }
}
