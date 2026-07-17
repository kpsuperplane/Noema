use serde::Deserialize;
use serde_json::Value;

use crate::{ProviderError, TokenUsage};

#[derive(Debug, Default)]
pub(super) struct ChatSseAccumulator {
    pending: Vec<u8>,
    text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<TokenUsage>,
}

#[derive(Debug)]
pub(super) struct ChatStreamOutput {
    pub(super) text: String,
    pub(super) response_id: Option<String>,
    pub(super) model: Option<String>,
    pub(super) usage: Option<TokenUsage>,
}

impl ChatSseAccumulator {
    pub(super) fn push_bytes(
        &mut self,
        bytes: &[u8],
        mut on_delta: impl FnMut(String),
    ) -> Result<(), ProviderError> {
        self.pending.extend_from_slice(bytes);
        while let Some((index, delimiter_len)) = next_event_boundary(&self.pending) {
            let raw = self.pending[..index].to_vec();
            self.pending.drain(..index + delimiter_len);
            self.handle_event(&raw, &mut on_delta)?;
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<ChatStreamOutput, ProviderError> {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.handle_event(&pending, &mut |_| {})?;
        }
        if self.text.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "local model produced empty output".to_string(),
            });
        }
        Ok(ChatStreamOutput {
            text: self.text,
            response_id: self.response_id,
            model: self.model,
            usage: self.usage,
        })
    }

    fn handle_event(
        &mut self,
        raw: &[u8],
        on_delta: &mut impl FnMut(String),
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
            if let Some(delta) = choice.delta.content.filter(|delta| !delta.is_empty()) {
                self.text.push_str(&delta);
                on_delta(delta);
            }
        }
        Ok(())
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
    fn chat_sse_accumulator_streams_deltas_and_usage_across_chunks() {
        let mut accumulator = ChatSseAccumulator::default();
        let mut deltas = Vec::new();
        accumulator
            .push_bytes(
                b"data: {\"id\":\"chat-1\",\"model\":\"local-8b\",\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
                |delta| deltas.push(delta),
            )
            .expect("first event");
        accumulator
            .push_bytes(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}],\"usage\":{\"prompt_tokens\":3,\"prompt_tokens_details\":{\"cached_tokens\":2},\"completion_tokens\":2,\"total_tokens\":5}}\n\ndata: [DONE]\n\n",
                |delta| deltas.push(delta),
            )
            .expect("remaining events");

        let output = accumulator.finish().expect("stream output");

        assert_eq!(deltas, vec!["hel", "lo"]);
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
            accumulator.finish(),
            Err(ProviderError::MalformedResponse { .. })
        ));
    }
}
