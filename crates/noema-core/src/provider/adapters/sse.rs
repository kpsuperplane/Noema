//! Server-Sent Events parser for Responses-compatible streams.

use super::responses::{ResponsesDiagnosticContext, ResponsesResponse, ResponsesUsage};
use crate::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SystemErrorEvent,
    provider::{GenerateStreamEvent, ProviderError},
};
use serde_json::Value;

pub(crate) struct SseAccumulator {
    diagnostics: ResponsesDiagnosticContext,
    pending: Vec<u8>,
    output_values: Vec<Value>,
    output_text: String,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<ResponsesUsage>,
    terminal_error: Option<Value>,
}

impl SseAccumulator {
    pub(crate) fn new(diagnostics: ResponsesDiagnosticContext) -> Self {
        Self {
            diagnostics,
            pending: Vec::new(),
            output_values: Vec::new(),
            output_text: String::new(),
            response_id: None,
            model: None,
            usage: None,
            terminal_error: None,
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
            let event = parse_sse_event_bytes(&raw).inspect_err(|error| {
                self.log_malformed(
                    error.to_string(),
                    serde_json::json!({
                        "raw_event_bytes_utf8_lossy": String::from_utf8_lossy(&raw).to_string(),
                    }),
                );
            })?;
            self.handle_event(event, on_event)?;
        }

        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<ResponsesResponse, ProviderError> {
        if !self.pending.is_empty() {
            let event = parse_sse_event_bytes(&self.pending).inspect_err(|error| {
                self.log_malformed(
                    error.to_string(),
                    serde_json::json!({
                        "raw_event_bytes_utf8_lossy": String::from_utf8_lossy(&self.pending)
                            .to_string(),
                    }),
                );
            })?;
            self.pending.clear();
            self.handle_event(event, on_event)?;
        }

        if let Some(error) = self.terminal_error {
            return Err(ProviderError::ApiError {
                status: 200,
                message: response_stream_error_message(&error),
                request_id: self.response_id,
            });
        }

        if !self.output_text.is_empty() {
            self.output_values.retain(|item| {
                item.get("type")
                    .and_then(Value::as_str)
                    .is_none_or(|kind| kind != "message")
            });
            self.output_values.push(serde_json::json!({
                "type": "message",
                "content": [{"type": "output_text", "text": self.output_text}]
            }));
        }

        let response_id = self.response_id.clone();
        let model = self.model.clone();
        let output_values = self.output_values.clone();
        let response = ResponsesResponse::from_stream_parts(
            self.response_id.clone(),
            self.model.clone(),
            self.output_values.clone(),
            self.usage.clone(),
        );
        if let Err(error) = &response {
            self.log_malformed(
                error.to_string(),
                serde_json::json!({
                    "response_id": response_id,
                    "model": model,
                    "output_values": output_values,
                }),
            );
        }
        response
    }

    #[cfg(test)]
    fn push_chunk(
        &mut self,
        chunk: &str,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<(), ProviderError> {
        self.push_bytes(chunk.as_bytes(), on_event)
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
            let message = format!("failed to parse SSE JSON: {source}");
            self.log_malformed(
                message.clone(),
                serde_json::json!({
                    "event": event.event,
                    "data": data,
                }),
            );
            ProviderError::MalformedResponse { message }
        })?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or_default();

        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    self.output_text.push_str(delta);
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        delta: delta.to_string(),
                    });
                }
            }
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    self.output_values.push(item.clone());
                }
            }
            "response.completed" | "response.incomplete" => {
                if let Some(response) = value.get("response") {
                    collect_terminal_response_metadata(
                        response,
                        &mut self.response_id,
                        &mut self.model,
                        &mut self.usage,
                    );
                    if self.output_values.is_empty()
                        && let Some(items) = response.get("output").and_then(Value::as_array)
                    {
                        self.output_values.extend(items.iter().cloned());
                    }
                }
            }
            "response.failed" => {
                if let Some(response) = value.get("response") {
                    collect_terminal_response_metadata(
                        response,
                        &mut self.response_id,
                        &mut self.model,
                        &mut self.usage,
                    );
                    self.terminal_error = response.get("error").cloned();
                }
            }
            "error" => {
                self.terminal_error = Some(value);
            }
            _ => {}
        }

        Ok(())
    }

    fn log_malformed(&self, message: impl Into<String>, raw: serde_json::Value) {
        let Some(logger) = &self.diagnostics.logger else {
            return;
        };
        let message = message.into();
        logger.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
                .with_context(self.diagnostics.context_json(self.response_id.as_deref()))
                .with_error_chain([message])
                .with_raw(raw),
        );
    }
}

pub(crate) fn collect_terminal_response_metadata(
    response: &Value,
    response_id: &mut Option<String>,
    model: &mut Option<String>,
    usage: &mut Option<ResponsesUsage>,
) {
    if response_id.is_none() {
        *response_id = response
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if model.is_none() {
        *model = response
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if usage.is_none() {
        *usage = response
            .get("usage")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok());
    }
}

fn response_stream_error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            error
                .get("error")
                .and_then(|body| body.get("message"))
                .and_then(Value::as_str)
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string())
}

pub(crate) struct SseEvent {
    event: Option<String>,
    data: Option<String>,
}

pub(crate) fn parse_sse_event(raw: &str) -> SseEvent {
    let mut event = None;
    let mut data = Vec::new();
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start().to_string());
        }
    }

    SseEvent {
        event,
        data: (!data.is_empty()).then(|| data.join("\n")),
    }
}

pub(crate) fn parse_sse_event_bytes(raw: &[u8]) -> Result<SseEvent, ProviderError> {
    let raw = std::str::from_utf8(raw).map_err(|source| ProviderError::MalformedResponse {
        message: format!("failed to decode SSE event as UTF-8: {source}"),
    })?;
    Ok(parse_sse_event(raw))
}

pub(crate) fn next_sse_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    [(b"\n\n".as_slice(), 2), (b"\r\n\r\n".as_slice(), 4)]
        .into_iter()
        .filter_map(|(delimiter, len)| {
            bytes
                .windows(delimiter.len())
                .position(|window| window == delimiter)
                .map(|index| (index, len))
        })
        .min_by_key(|(index, _)| *index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_diagnostics() -> ResponsesDiagnosticContext {
        ResponsesDiagnosticContext::new(None, "test", "test-model", None)
    }

    #[test]
    fn incremental_sse_parser_emits_deltas_before_terminal_response() {
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator
            .push_chunk(
                "event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\n",
                &mut |event| events.push(event),
            )
            .expect("first chunk");
        accumulator
            .push_chunk(
                "event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n\
                 event: response.completed\n\
                 data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\n",
                &mut |event| events.push(event),
            )
            .expect("second chunk");

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "lo".to_string()
                }
            ]
        );

        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "Hello");
    }

    #[test]
    fn response_from_sse_prefers_output_item_done_over_terminal_output() {
        let response = response_from_sse(
            "event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"from item\"}]}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
        )
        .expect("sse response");

        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "from item");
    }

    #[test]
    fn response_from_sse_prefers_streamed_text_over_output_item_done_text() {
        let response = response_from_sse(
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"response_status\\\":\\\"needs_tools\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"commentary\\\",\\\"text\\\":\\\"Searching memory.\\\"}],\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"\\\"tool_calls\\\":[{\\\"id\\\":\\\"call_memory_1\\\",\\\"name\\\":\\\"search_memory\\\",\\\"payload\\\":{\\\"scope_ids\\\":[\\\"human:local\\\"],\\\"query\\\":\\\"\\\",\\\"purpose\\\":\\\"answer_human_question\\\",\\\"limit\\\":8}}],\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"\\\"memory_proposals\\\":[]}\"}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Searching memory.\"}]}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
        )
        .expect("sse response");

        assert_eq!(
            response.output_text().expect("output text"),
            "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Searching memory.\"}],\"tool_calls\":[{\"id\":\"call_memory_1\",\"name\":\"search_memory\",\"payload\":{\"scope_ids\":[\"human:local\"],\"query\":\"\",\"purpose\":\"answer_human_question\",\"limit\":8}}],\"memory_proposals\":[]}"
        );
    }

    #[test]
    fn response_from_sse_preserves_function_call_items_with_streamed_text() {
        let response = response_from_sse(
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"response_status\\\":\\\"needs_tools\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"commentary\\\",\\\"text\\\":\\\"Checking.\\\"}],\\\"tool_calls\\\":[],\\\"memory_proposals\\\":[]}\"}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
        )
        .expect("sse response");

        assert_eq!(
            response.output_text().expect("output text"),
            "{\"response_status\":\"needs_tools\",\"responses\":[{\"kind\":\"text\",\"phase\":\"commentary\",\"text\":\"Checking.\"}],\"tool_calls\":[],\"memory_proposals\":[]}"
        );
        let calls = response.native_tool_calls().expect("native calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(calls[0].name, "search_memory");
        assert_eq!(calls[0].payload["query"], "trains");
    }

    #[test]
    fn incremental_sse_parser_buffers_utf8_split_across_byte_chunks() {
        let accent = "\u{00e9}";
        let payload = format!(
            "event: response.output_text.delta\n\
             data: {{\"type\":\"response.output_text.delta\",\"delta\":\"caf{accent}\"}}\n\n\
             event: response.completed\n\
             data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp_test\",\"status\":\"completed\"}}}}\n\n"
        );
        let split_at = payload.find(accent).expect("accent byte offset") + 1;
        let bytes = payload.as_bytes();
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());

        accumulator
            .push_bytes(&bytes[..split_at], &mut |event| events.push(event))
            .expect("first partial byte chunk");
        accumulator
            .push_bytes(&bytes[split_at..], &mut |event| events.push(event))
            .expect("second partial byte chunk");

        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: format!("caf{accent}")
            }]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(
            response.output_text().expect("output text"),
            format!("caf{accent}")
        );
    }

    #[test]
    fn incremental_sse_parser_buffers_delimiter_split_across_byte_chunks() {
        let payload = b"event: response.output_text.delta\r\n\
                        data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\r\n\
                        \r\n\
                        event: response.completed\r\n\
                        data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\r\n\
                        \r\n";
        let first_delimiter = payload
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("delimiter");
        let split_at = first_delimiter + 2;
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());

        accumulator
            .push_bytes(&payload[..split_at], &mut |event| events.push(event))
            .expect("first delimiter chunk");
        accumulator
            .push_bytes(&payload[split_at..], &mut |event| events.push(event))
            .expect("second delimiter chunk");

        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: "Hi".to_string()
            }]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), "Hi");
    }

    fn response_from_sse(text: &str) -> Result<ResponsesResponse, ProviderError> {
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator.push_chunk(text, &mut |_| {})?;
        accumulator.finish(&mut |_| {})
    }
}
