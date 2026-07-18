//! Incremental parser for structured Noema JSON response streams.

use crate::GenerateStreamEvent;

/// Incrementally extracts model-visible deltas from a structured response stream.
#[derive(Debug, Default)]
pub struct NoemaAssistantTextDeltaExtractor {
    stack: Vec<JsonContext>,
    string: Option<JsonStringReader>,
    response_count: usize,
    root_completed: bool,
}

impl NoemaAssistantTextDeltaExtractor {
    /// Consume one JSON text delta and emit any newly completed visible events.
    pub fn push_delta(
        &mut self,
        delta: &str,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        let mut visible_delta = IndexedVisibleDelta::default();

        for ch in delta.chars() {
            if self.root_completed && self.stack.is_empty() {
                continue;
            }
            if self.string.is_some() {
                let stream_visible =
                    self.string.as_ref().is_some_and(|reader| {
                        matches!(reader.target, JsonStringTarget::ResponseText)
                    }) && self.current_item_is_assistant();
                if stream_visible {
                    visible_delta.prepare(self.current_response_index(), on_event);
                }
                let closed = self.string.as_mut().is_some_and(|reader| {
                    reader.push_char(ch, stream_visible, &mut visible_delta.text)
                });
                if closed && let Some(reader) = self.string.take() {
                    self.finish_string(reader, &mut visible_delta, on_event);
                }
                continue;
            }

            match ch {
                '"' => {
                    self.string = Some(JsonStringReader::new(self.next_string_target()));
                }
                '{' => self.push_object(on_event),
                '[' => self.push_array(),
                '}' | ']' => self.pop_container(),
                ',' => self.handle_comma(),
                ':' => {}
                _ if ch.is_whitespace() => {}
                _ => self.complete_scalar_value(),
            }
        }

        visible_delta.flush(on_event);
    }

    fn push_object(&mut self, _on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send)) {
        let role = match self.stack.last_mut() {
            None => JsonObjectRole::Root,
            Some(JsonContext::Array(JsonArrayRole::Responses { next_index })) => {
                let index = *next_index;
                *next_index += 1;
                self.response_count += 1;
                JsonObjectRole::ResponseItem(ResponseItemState {
                    index,
                    ..ResponseItemState::default()
                })
            }
            Some(JsonContext::Array(JsonArrayRole::ToolCalls { next_index })) => {
                let output_index = self.response_count + *next_index;
                *next_index += 1;
                JsonObjectRole::ToolCall(ToolCallState {
                    output_index,
                    ..ToolCallState::default()
                })
            }
            Some(_) => JsonObjectRole::Nested,
        };
        self.stack.push(JsonContext::Object(JsonObjectContext {
            role,
            pending_key: None,
            expecting_key: true,
        }));
    }

    fn push_array(&mut self) {
        let role = if self
            .stack
            .last()
            .is_some_and(|context| context.is_root_responses_value())
        {
            JsonArrayRole::Responses { next_index: 0 }
        } else if self
            .stack
            .last()
            .is_some_and(|context| context.is_root_tool_calls_value())
        {
            JsonArrayRole::ToolCalls { next_index: 0 }
        } else {
            JsonArrayRole::Nested
        };
        self.stack.push(JsonContext::Array(role));
    }

    fn pop_container(&mut self) {
        let popped_root = matches!(
            self.stack.last(),
            Some(JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::Root,
                ..
            }))
        );
        self.stack.pop();
        if popped_root {
            self.root_completed = true;
        }
        self.complete_scalar_value();
    }

    fn handle_comma(&mut self) {
        if let Some(JsonContext::Object(context)) = self.stack.last_mut() {
            context.pending_key = None;
            context.expecting_key = true;
        }
    }

    fn complete_scalar_value(&mut self) {
        if let Some(JsonContext::Object(context)) = self.stack.last_mut()
            && context.pending_key.is_some()
        {
            context.pending_key = None;
            context.expecting_key = false;
        }
    }

    fn next_string_target(&self) -> JsonStringTarget {
        let Some(JsonContext::Object(context)) = self.stack.last() else {
            return JsonStringTarget::Ignored;
        };
        if context.expecting_key {
            return JsonStringTarget::Key;
        }
        let Some(key) = context.pending_key.as_deref() else {
            return JsonStringTarget::Ignored;
        };
        match (&context.role, key) {
            (JsonObjectRole::ResponseItem(_), "text") => JsonStringTarget::ResponseText,
            (JsonObjectRole::ResponseItem(_), "kind") => JsonStringTarget::ResponseKind,
            (JsonObjectRole::ToolCall(_), "name") => JsonStringTarget::ToolCallName,
            _ => JsonStringTarget::Value,
        }
    }

    fn finish_string(
        &mut self,
        reader: JsonStringReader,
        visible_delta: &mut IndexedVisibleDelta,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        match reader.target {
            JsonStringTarget::Key => {
                if let Some(JsonContext::Object(context)) = self.stack.last_mut() {
                    context.pending_key = Some(reader.decoded);
                    context.expecting_key = false;
                }
            }
            JsonStringTarget::ResponseKind => {
                if let Some(item) = self.current_response_item_mut() {
                    item.kind = Some(reader.decoded);
                    if item.is_text() && !item.buffered_text.is_empty() {
                        visible_delta.prepare(Some(item.index), on_event);
                        visible_delta
                            .text
                            .push_str(&std::mem::take(&mut item.buffered_text));
                    }
                }
                self.complete_scalar_value();
            }
            JsonStringTarget::ToolCallName => {
                if let Some(tool_call) = self.current_tool_call_mut() {
                    tool_call.name = Some(reader.decoded);
                    if let Some(event) = tool_call.started_event() {
                        on_event(event);
                    }
                }
                self.complete_scalar_value();
            }
            JsonStringTarget::ResponseText => {
                if let Some(item) = self.current_response_item_mut()
                    && item.kind.is_none()
                {
                    item.buffered_text.push_str(&reader.decoded);
                }
                self.complete_scalar_value();
            }
            JsonStringTarget::Value | JsonStringTarget::Ignored => self.complete_scalar_value(),
        }
    }

    fn current_item_is_assistant(&self) -> bool {
        self.stack.last().is_some_and(|context| match context {
            JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::ResponseItem(item),
                ..
            }) => item.is_text(),
            JsonContext::Object(_) | JsonContext::Array(_) => false,
        })
    }

    fn current_response_index(&self) -> Option<usize> {
        match self.stack.last()? {
            JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::ResponseItem(item),
                ..
            }) => Some(item.index),
            JsonContext::Object(_) | JsonContext::Array(_) => None,
        }
    }

    fn current_response_item_mut(&mut self) -> Option<&mut ResponseItemState> {
        match self.stack.last_mut()? {
            JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::ResponseItem(item),
                ..
            }) => Some(item),
            JsonContext::Object(_) | JsonContext::Array(_) => None,
        }
    }

    fn current_tool_call_mut(&mut self) -> Option<&mut ToolCallState> {
        match self.stack.last_mut()? {
            JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::ToolCall(tool_call),
                ..
            }) => Some(tool_call),
            JsonContext::Object(_) | JsonContext::Array(_) => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum JsonContext {
    Object(JsonObjectContext),
    Array(JsonArrayRole),
}

impl JsonContext {
    fn is_root_responses_value(&self) -> bool {
        matches!(
            self,
            Self::Object(JsonObjectContext {
                role: JsonObjectRole::Root,
                pending_key: Some(key),
                ..
            }) if key == "responses"
        )
    }

    fn is_root_tool_calls_value(&self) -> bool {
        matches!(
            self,
            Self::Object(JsonObjectContext {
                role: JsonObjectRole::Root,
                pending_key: Some(key),
                ..
            }) if key == "tool_calls"
        )
    }
}

#[derive(Debug)]
pub(crate) struct JsonObjectContext {
    role: JsonObjectRole,
    pending_key: Option<String>,
    expecting_key: bool,
}

#[derive(Debug)]
pub(crate) enum JsonObjectRole {
    Root,
    ResponseItem(ResponseItemState),
    ToolCall(ToolCallState),
    Nested,
}

#[derive(Debug)]
pub(crate) enum JsonArrayRole {
    Responses { next_index: usize },
    ToolCalls { next_index: usize },
    Nested,
}

#[derive(Debug, Default)]
pub(crate) struct ResponseItemState {
    index: usize,
    kind: Option<String>,
    buffered_text: String,
}

impl ResponseItemState {
    fn is_text(&self) -> bool {
        self.kind.as_deref() == Some("text")
    }
}

#[derive(Debug, Default)]
struct IndexedVisibleDelta {
    response_index: Option<usize>,
    text: String,
}

impl IndexedVisibleDelta {
    fn prepare(
        &mut self,
        response_index: Option<usize>,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        if self.response_index == response_index {
            return;
        }
        self.flush(on_event);
        self.response_index = response_index;
    }

    fn flush(&mut self, on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send)) {
        if self.text.is_empty() {
            return;
        }
        on_event(GenerateStreamEvent::AssistantTextDelta {
            response_index: self.response_index.unwrap_or(0),
            delta: std::mem::take(&mut self.text),
        });
    }
}

#[derive(Debug, Default)]
pub(crate) struct ToolCallState {
    output_index: usize,
    name: Option<String>,
    started_emitted: bool,
}

impl ToolCallState {
    fn started_event(&mut self) -> Option<GenerateStreamEvent> {
        if self.started_emitted {
            return None;
        }
        let name = self.name.as_deref()?.trim();
        if name.is_empty() {
            return None;
        }
        self.started_emitted = true;
        Some(GenerateStreamEvent::ToolCallStarted {
            output_index: self.output_index,
            name: name.to_string(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct JsonStringReader {
    target: JsonStringTarget,
    decoded: String,
    escape: Option<JsonStringEscape>,
    pending_high_surrogate: Option<u16>,
}

impl JsonStringReader {
    fn new(target: JsonStringTarget) -> Self {
        Self {
            target,
            decoded: String::new(),
            escape: None,
            pending_high_surrogate: None,
        }
    }

    fn push_char(&mut self, ch: char, stream_visible: bool, visible_delta: &mut String) -> bool {
        match self.escape.take() {
            Some(JsonStringEscape::Simple) => {
                self.push_escaped_char(ch, stream_visible, visible_delta)
            }
            Some(JsonStringEscape::Unicode(mut escape)) => {
                if ch.is_ascii_hexdigit() {
                    escape.push(ch);
                    if escape.len() == 4 {
                        if let Ok(unit) = u16::from_str_radix(&escape, 16) {
                            self.push_unicode_escape(unit, stream_visible, visible_delta);
                        }
                    } else {
                        self.escape = Some(JsonStringEscape::Unicode(escape));
                    }
                }
            }
            None => match ch {
                '\\' => {
                    self.escape = Some(JsonStringEscape::Simple);
                }
                '"' => {
                    if self.pending_high_surrogate.take().is_some() {
                        self.push_decoded_char(
                            char::REPLACEMENT_CHARACTER,
                            stream_visible,
                            visible_delta,
                        );
                    }
                    return true;
                }
                _ => self.push_decoded_char(ch, stream_visible, visible_delta),
            },
        }
        false
    }

    fn push_escaped_char(&mut self, ch: char, stream_visible: bool, visible_delta: &mut String) {
        match ch {
            '"' => self.push_decoded_char('"', stream_visible, visible_delta),
            '\\' => self.push_decoded_char('\\', stream_visible, visible_delta),
            '/' => self.push_decoded_char('/', stream_visible, visible_delta),
            'b' => self.push_decoded_char('\u{0008}', stream_visible, visible_delta),
            'f' => self.push_decoded_char('\u{000c}', stream_visible, visible_delta),
            'n' => self.push_decoded_char('\n', stream_visible, visible_delta),
            'r' => self.push_decoded_char('\r', stream_visible, visible_delta),
            't' => self.push_decoded_char('\t', stream_visible, visible_delta),
            'u' => {
                self.escape = Some(JsonStringEscape::Unicode(String::new()));
            }
            _ => {}
        }
    }

    fn push_unicode_escape(&mut self, unit: u16, stream_visible: bool, visible_delta: &mut String) {
        if let Some(high) = self.pending_high_surrogate.take() {
            if (0xdc00..=0xdfff).contains(&unit) {
                let scalar = 0x10000 + (((high - 0xd800) as u32) << 10) + ((unit - 0xdc00) as u32);
                if let Some(ch) = char::from_u32(scalar) {
                    self.push_decoded_char(ch, stream_visible, visible_delta);
                }
                return;
            }
            self.push_decoded_char(char::REPLACEMENT_CHARACTER, stream_visible, visible_delta);
        }

        if (0xd800..=0xdbff).contains(&unit) {
            self.pending_high_surrogate = Some(unit);
        } else if (0xdc00..=0xdfff).contains(&unit) {
            self.push_decoded_char(char::REPLACEMENT_CHARACTER, stream_visible, visible_delta);
        } else if let Some(ch) = char::from_u32(unit as u32) {
            self.push_decoded_char(ch, stream_visible, visible_delta);
        }
    }

    fn push_decoded_char(&mut self, ch: char, stream_visible: bool, visible_delta: &mut String) {
        if stream_visible {
            visible_delta.push(ch);
        } else {
            self.decoded.push(ch);
        }
    }
}

#[derive(Debug)]
enum JsonStringTarget {
    Key,
    ResponseKind,
    ToolCallName,
    ResponseText,
    Value,
    Ignored,
}

#[derive(Debug)]
enum JsonStringEscape {
    Simple,
    Unicode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noema_assistant_text_delta_extractor_decodes_escaped_visible_text() {
        let cases: &[(&str, &[&str], &str)] = &[
            (
                "escaped text",
                &[
                    r#"{"response_status":"final","responses":[{"kind":"text","text":"Hi"#,
                    "\\nthere\\u00",
                    "21\"}],\"tool_calls\":[]}",
                ],
                "Hi\nthere!",
            ),
            (
                "text before kind",
                &[
                    r#"{"response_status":"final","responses":[{"text":"Hel"#,
                    r#"lo","kind":"text"}],"tool_calls":[]}"#,
                ],
                "Hello",
            ),
            (
                "phase metadata",
                &[
                    r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"commentary","text":"Checking"#,
                    r#" now."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]} "#,
                ],
                "Checking now.",
            ),
            (
                "multiple text items",
                &[
                    r#"{"response_status":"final","responses":[{"kind":"text","text":"Hel"#,
                    r#"lo"},{"kind":"text","text":" again"}],"tool_calls":[]}"#,
                ],
                "Hello again",
            ),
            (
                "duplicate envelope",
                &[
                    r#"{"response_status":"final","responses":[{"kind":"text","text":"same"}],"tool_calls":[]}"#,
                    r#"{"response_status":"final","responses":[{"kind":"text","text":"same"}],"tool_calls":[]}"#,
                ],
                "same",
            ),
            (
                "nested payload",
                &[
                    r#"{"response_status":"final","responses":[{"kind":"structured","schema":"test","payload":{"responses":[{"kind":"text","text":"also wrong"}]}}"#,
                    r#",{"text":"right","kind":"text"}],"tool_calls":[]}"#,
                ],
                "right",
            ),
        ];

        for (case, chunks, expected) in cases {
            assert_eq!(extract_streamed_text(chunks), *expected, "{case}");
        }
    }

    #[test]
    fn noema_assistant_text_delta_extractor_tags_multiple_assistant_items() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"response_status":"final","responses":[{"kind":"text","text":"one"},{"kind":"text","text":"two"}],"tool_calls":[]}"#,
            &mut |event| events.push(event),
        );

        let deltas = events
            .into_iter()
            .filter_map(|event| match event {
                GenerateStreamEvent::AssistantTextDelta {
                    response_index,
                    delta,
                } => Some((response_index, delta)),
                GenerateStreamEvent::ToolCallStarted { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(deltas, vec![(0, "one".to_string()), (1, "two".to_string())]);
    }

    #[test]
    fn noema_assistant_text_delta_extractor_emits_tool_call_started_when_name_streams() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"commentary","text":"Need tool"}],"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta(
            r#""tool_calls":[{"id":"call_1","name":"search_memory","payload":{"query":"trains""#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "Need tool".to_string()
                },
                GenerateStreamEvent::ToolCallStarted {
                    output_index: 1,
                    name: "search_memory".to_string()
                },
            ]
        );
    }

    #[test]
    fn noema_assistant_text_delta_extractor_ignores_nested_tool_call_like_payloads() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"response_status":"final","responses":[{"kind":"structured","schema":"test","payload":{"name":"wrong"}}],"tool_calls":[]}"#,
            &mut |event| events.push(event),
        );

        assert_eq!(events, Vec::<GenerateStreamEvent>::new());
    }

    #[test]
    fn noema_assistant_text_delta_extractor_emits_silent_tool_call_started() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            events,
            vec![GenerateStreamEvent::ToolCallStarted {
                output_index: 0,
                name: "search_memory".to_string(),
            }]
        );
    }

    fn extract_streamed_text(chunks: &[&str]) -> String {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        for chunk in chunks {
            extractor.push_delta(chunk, &mut |event| events.push(event));
        }
        assistant_text_from_events(&events)
    }

    fn assistant_text_from_events(events: &[GenerateStreamEvent]) -> String {
        events
            .iter()
            .filter_map(|event| match event {
                GenerateStreamEvent::AssistantTextDelta { delta, .. } => Some(delta.as_str()),
                GenerateStreamEvent::ToolCallStarted { .. } => None,
            })
            .collect()
    }
}
