//! Server-Sent Events parser for Responses-compatible streams.

use super::{ResponsesDiagnosticContext, ResponsesResponse, ResponsesUsage};
use crate::response_support::sse::{SseEvent, next_sse_event_boundary, parse_sse_event_bytes};
use crate::{GenerateStreamEvent, ProviderError, ProviderTimingMilestone};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

pub(crate) struct SseAccumulator {
    diagnostics: ResponsesDiagnosticContext,
    pending: Vec<u8>,
    output_values: BTreeMap<usize, Value>,
    output_text: BTreeMap<usize, String>,
    response_id: Option<String>,
    model: Option<String>,
    usage: Option<ResponsesUsage>,
    terminal_error: Option<Value>,
    started_tool_calls: HashSet<String>,
    started_hosted_web_searches: HashSet<usize>,
    searching_hosted_web_searches: HashSet<usize>,
    completed_hosted_web_searches: HashSet<usize>,
}

impl SseAccumulator {
    pub(crate) fn new(diagnostics: ResponsesDiagnosticContext) -> Self {
        Self {
            diagnostics,
            pending: Vec::new(),
            output_values: BTreeMap::new(),
            output_text: BTreeMap::new(),
            response_id: None,
            model: None,
            usage: None,
            terminal_error: None,
            started_tool_calls: HashSet::new(),
            started_hosted_web_searches: HashSet::new(),
            searching_hosted_web_searches: HashSet::new(),
            completed_hosted_web_searches: HashSet::new(),
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

        let mut reconciled_messages = HashSet::new();
        for (output_index, text) in std::mem::take(&mut self.output_text) {
            let message_index = self
                .output_values
                .get(&output_index)
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
                .map(|_| output_index)
                .or_else(|| {
                    self.output_values.iter().find_map(|(index, item)| {
                        (item.get("type").and_then(Value::as_str) == Some("message")
                            && !reconciled_messages.contains(index))
                        .then_some(*index)
                    })
                });
            let replaced_index = message_index.and_then(|index| {
                let output_text = self
                    .output_values
                    .get_mut(&index)?
                    .get_mut("content")?
                    .as_array_mut()?
                    .iter_mut()
                    .find(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))?
                    .as_object_mut()?;
                output_text.insert("text".to_string(), Value::String(text.clone()));
                Some(index)
            });
            if let Some(index) = replaced_index {
                reconciled_messages.insert(index);
            } else {
                let output_index = (output_index..)
                    .find(|index| !self.output_values.contains_key(index))
                    .expect("usize output index space cannot be exhausted");
                self.output_values.insert(
                    output_index,
                    serde_json::json!({
                        "type": "message",
                        "content": [{"type": "output_text", "text": text}]
                    }),
                );
            }
        }

        let response_id = self.response_id.clone();
        let model = self.model.clone();
        let output_values = std::mem::take(&mut self.output_values)
            .into_values()
            .collect::<Vec<_>>();
        let response = ResponsesResponse::from_stream_parts(
            self.response_id.clone(),
            self.model.clone(),
            output_values.clone(),
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
                    let output_index = value
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .and_then(|index| usize::try_from(index).ok())
                        .unwrap_or_default();
                    self.output_text
                        .entry(output_index)
                        .or_default()
                        .push_str(delta);
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        response_index: output_index,
                        delta: delta.to_string(),
                    });
                }
            }
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    let output_index = value
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .and_then(|index| usize::try_from(index).ok())
                        .unwrap_or_else(|| {
                            (0..)
                                .find(|index| !self.output_values.contains_key(index))
                                .expect("usize output index space cannot be exhausted")
                        });
                    self.output_values.insert(output_index, item.clone());
                    if item.get("type").and_then(Value::as_str) == Some("web_search_call") {
                        self.emit_hosted_search_timing(
                            ProviderTimingMilestone::HostedWebSearchCompleted,
                            output_index,
                            on_event,
                        );
                    }
                }
            }
            "response.output_item.added" => {
                let item = value.get("item");
                if let Some(output_index) = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                {
                    match item
                        .and_then(|item| item.get("type"))
                        .and_then(Value::as_str)
                    {
                        Some("function_call") => {
                            let provider_call_id = item
                                .and_then(|item| item.get("call_id"))
                                .and_then(Value::as_str)
                                .filter(|value| !value.trim().is_empty());
                            let name = item
                                .and_then(|item| item.get("name"))
                                .and_then(Value::as_str)
                                .filter(|value| !value.trim().is_empty());
                            if let (Some(provider_call_id), Some(name)) = (provider_call_id, name)
                                && self.started_tool_calls.insert(provider_call_id.to_string())
                            {
                                on_event(GenerateStreamEvent::ToolCallStarted {
                                    output_index,
                                    provider_call_id: provider_call_id.to_string(),
                                    name: name.to_string(),
                                });
                            }
                        }
                        Some("web_search_call")
                            if self.started_hosted_web_searches.insert(output_index) =>
                        {
                            let id = item
                                .and_then(|item| item.get("id"))
                                .and_then(Value::as_str)
                                .map(ToString::to_string);
                            on_event(GenerateStreamEvent::HostedWebSearchStarted {
                                output_index,
                                id,
                            });
                        }
                        _ => {}
                    }
                }
            }
            "response.web_search_call.in_progress" | "response.web_search_call.searching" => {
                if let Some(output_index) = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                {
                    if self.started_hosted_web_searches.insert(output_index) {
                        let id = value
                            .get("item_id")
                            .and_then(Value::as_str)
                            .map(ToString::to_string);
                        on_event(GenerateStreamEvent::HostedWebSearchStarted { output_index, id });
                    }
                    if event_type == "response.web_search_call.searching"
                        && self.searching_hosted_web_searches.insert(output_index)
                    {
                        on_event(GenerateStreamEvent::ProviderTiming {
                            milestone: ProviderTimingMilestone::HostedWebSearchSearching,
                            output_index: Some(output_index),
                        });
                    }
                }
            }
            "response.web_search_call.completed" => {
                if let Some(output_index) = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                {
                    self.emit_hosted_search_timing(
                        ProviderTimingMilestone::HostedWebSearchCompleted,
                        output_index,
                        on_event,
                    );
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
                        self.output_values.extend(items.iter().cloned().enumerate());
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

    fn emit_hosted_search_timing(
        &mut self,
        milestone: ProviderTimingMilestone,
        output_index: usize,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        if self.completed_hosted_web_searches.insert(output_index) {
            on_event(GenerateStreamEvent::ProviderTiming {
                milestone,
                output_index: Some(output_index),
            });
        }
    }

    fn log_malformed(&self, message: impl Into<String>, raw: serde_json::Value) {
        self.diagnostics
            .log_malformed_with_request_id(message, self.response_id.as_deref(), raw);
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
                    response_index: 0,
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
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
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Searching \"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"memory.\"}\n\
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
            "Searching memory."
        );
    }

    #[test]
    fn streamed_messages_keep_boundaries_without_repeating_done_text() {
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator
            .push_chunk(
                "event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"Got it\"}\n\
                 \n\
                 event: response.output_item.done\n\
                 data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"phase\":\"commentary\",\"content\":[{\"type\":\"output_text\",\"text\":\"Got it\"}]}}\n\
                 \n\
                 event: response.output_text.delta\n\
                 data: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"delta\":\"What time works?\"}\n\
                 \n\
                 event: response.output_item.done\n\
                 data: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"message\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"What time works?\"}]}}\n\
                 \n\
                 event: response.completed\n\
                 data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\
                 \n",
                &mut |event| events.push(event),
            )
            .expect("two streamed messages");

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "Got it".to_string(),
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 1,
                    delta: "What time works?".to_string(),
                },
            ]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        let generated = response
            .finalize(
                &super::super::tools::ResponsesToolNameMap::default(),
                crate::ProviderToolTransport::None,
                &test_diagnostics(),
            )
            .expect("normalized response");
        assert_eq!(
            generated.responses,
            vec![
                crate::GenerateResponseItem::Text {
                    id: None,
                    phase: Some(crate::AssistantTextPhase::Commentary),
                    text: "Got it".to_string(),
                    citations: Vec::new(),
                },
                crate::GenerateResponseItem::Text {
                    id: None,
                    phase: Some(crate::AssistantTextPhase::FinalAnswer),
                    text: "What time works?".to_string(),
                    citations: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn response_from_sse_preserves_function_call_items_with_streamed_text() {
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator
            .push_chunk(
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Checking.\"}\n\
             \n\
             event: response.output_item.added\n\
             data: {\"type\":\"response.output_item.added\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"\"}}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
                &mut |event| events.push(event),
            )
            .expect("sse events");
        let response = accumulator.finish(&mut |_| {}).expect("sse response");

        assert!(events.contains(&GenerateStreamEvent::ToolCallStarted {
            output_index: 1,
            provider_call_id: "call_1".to_string(),
            name: "search_memory".to_string(),
        }));

        assert_eq!(response.output_text().expect("output text"), "Checking.");
        let tools = [noema_capabilities::ToolSpec::new(
            "search_memory",
            "Search memory.",
            serde_json::json!({"type": "object"}),
        )
        .expect("tool")];
        let names =
            super::super::tools::ResponsesToolNameMap::from_tools(&tools).expect("tool names");
        let calls = response
            .native_tool_calls_with_names(&names)
            .expect("native calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id.as_deref(), Some("item_1"));
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("call_1"));
        assert_eq!(calls[0].name, "search_memory");
        assert_eq!(calls[0].payload["query"], "trains");
    }

    #[test]
    fn streamed_response_preserves_hosted_search_and_citations() {
        let response = response_from_sse(
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Current answer.\"}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"current answer\"}}}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"discarded done text\",\"annotations\":[{\"type\":\"url_citation\",\"title\":\"Official source\",\"url\":\"https://example.com/source\",\"start_index\":0,\"end_index\":10}]}]}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\
             \n",
        )
        .expect("sse response");

        assert!(
            response
                .output_text()
                .expect("output text")
                .contains("Current answer.")
        );
        assert_eq!(response.hosted_web_searches().len(), 1);
        assert_eq!(response.hosted_web_searches()[0].tool_name, "web.search");
        assert_eq!(
            response.hosted_web_searches()[0].arguments["query"],
            "current answer"
        );
        assert_eq!(
            response.citations(),
            vec![crate::GenerateCitation {
                title: "Official source".to_string(),
                url: "https://example.com/source".to_string(),
                start_index: Some(0),
                end_index: Some(10),
            }]
        );
        let generated = response
            .finalize(
                &super::super::tools::ResponsesToolNameMap::default(),
                crate::ProviderToolTransport::Native,
                &test_diagnostics(),
            )
            .expect("normalized response");
        assert_eq!(generated.hosted_web_searches.len(), 1);
        assert_eq!(generated.hosted_web_searches[0].output_index, 0);
        let crate::GenerateResponseItem::Text { citations, .. } = &generated.responses[0];
        assert_eq!(citations.len(), 1);
        assert_eq!(generated.assistant_text(), "Current answer.");
    }

    #[test]
    fn hosted_search_lifecycle_is_normalized_and_deduplicated() {
        let mut events = Vec::new();
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator
            .push_chunk(
                "event: response.output_item.added\n\
                 data: {\"type\":\"response.output_item.added\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"in_progress\"}}\n\
                 \n\
                 event: response.web_search_call.in_progress\n\
                 data: {\"type\":\"response.web_search_call.in_progress\",\"output_index\":2,\"item_id\":\"ws_1\"}\n\
                 \n\
                 event: response.web_search_call.searching\n\
                 data: {\"type\":\"response.web_search_call.searching\",\"output_index\":2,\"item_id\":\"ws_1\"}\n\
                 \n\
                 event: response.output_item.done\n\
                 data: {\"type\":\"response.output_item.done\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"weather\"}}}\n\
                 \n\
                 event: response.completed\n\
                 data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\
                 \n",
                &mut |event| events.push(event),
            )
            .expect("hosted search events");

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::HostedWebSearchStarted {
                    output_index: 2,
                    id: Some("ws_1".to_string()),
                },
                GenerateStreamEvent::ProviderTiming {
                    milestone: ProviderTimingMilestone::HostedWebSearchSearching,
                    output_index: Some(2),
                },
                GenerateStreamEvent::ProviderTiming {
                    milestone: ProviderTimingMilestone::HostedWebSearchCompleted,
                    output_index: Some(2),
                },
            ]
        );
        accumulator.finish(&mut |_| {}).expect("response");
    }

    #[test]
    fn incremental_sse_parser_buffers_utf8_and_delimiter_splits() {
        let accent = "\u{00e9}";
        let payload = format!(
            "event: response.output_text.delta\n\
             data: {{\"type\":\"response.output_text.delta\",\"delta\":\"caf{accent}\"}}\n\n\
             event: response.completed\n\
             data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp_test\",\"status\":\"completed\"}}}}\n\n"
        );
        let split_at = payload.find(accent).expect("accent byte offset") + 1;
        assert_split_payload(payload.as_bytes(), split_at, &format!("caf{accent}"));

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
        assert_split_payload(payload, first_delimiter + 2, "Hi");
    }

    fn assert_split_payload(payload: &[u8], split_at: usize, expected: &str) {
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
                response_index: 0,
                delta: expected.to_string()
            }]
        );
        let response = accumulator.finish(&mut |_| {}).expect("response");
        assert_eq!(response.id.as_deref(), Some("resp_test"));
        assert_eq!(response.output_text().expect("output text"), expected);
    }

    fn response_from_sse(text: &str) -> Result<ResponsesResponse, ProviderError> {
        let mut accumulator = SseAccumulator::new(test_diagnostics());
        accumulator.push_chunk(text, &mut |_| {})?;
        accumulator.finish(&mut |_| {})
    }
}
