//! Incremental parser for structured Noema JSON response streams.

use crate::provider::GenerateStreamEvent;

#[derive(Debug, Default)]
pub(crate) struct NoemaAssistantTextDeltaExtractor {
    stack: Vec<JsonContext>,
    string: Option<JsonStringReader>,
    memory_proposals_started_emitted: bool,
}

impl NoemaAssistantTextDeltaExtractor {
    pub(crate) fn push_delta(
        &mut self,
        delta: &str,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        let mut visible_delta = String::new();

        for ch in delta.chars() {
            if self.string.is_some() {
                let stream_visible = self
                    .string
                    .as_ref()
                    .is_some_and(|reader| matches!(reader.target, JsonStringTarget::ItemText))
                    && self.current_item_is_assistant();
                let closed = self
                    .string
                    .as_mut()
                    .is_some_and(|reader| reader.push_char(ch, stream_visible, &mut visible_delta));
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

        if !visible_delta.is_empty() {
            on_event(GenerateStreamEvent::AssistantTextDelta {
                delta: visible_delta,
            });
        }
    }

    fn push_object(&mut self, on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send)) {
        if self.stack.last().is_some_and(|context| {
            matches!(context, JsonContext::Array(JsonArrayRole::MemoryProposals))
        }) && !self.memory_proposals_started_emitted
        {
            self.memory_proposals_started_emitted = true;
            on_event(GenerateStreamEvent::MemoryProposalsStarted);
        }
        let role = match self.stack.last_mut() {
            None => JsonObjectRole::Root,
            Some(JsonContext::Array(JsonArrayRole::Output { next_index })) => {
                let output_index = *next_index;
                *next_index += 1;
                JsonObjectRole::OutputItem(OutputItemState {
                    output_index,
                    ..OutputItemState::default()
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
            .is_some_and(|context| context.is_root_output_value())
        {
            JsonArrayRole::Output { next_index: 0 }
        } else if self
            .stack
            .last()
            .is_some_and(|context| context.is_current_memory_proposals_value())
        {
            JsonArrayRole::MemoryProposals
        } else {
            JsonArrayRole::Nested
        };
        self.stack.push(JsonContext::Array(role));
    }

    fn pop_container(&mut self) {
        self.stack.pop();
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
            (JsonObjectRole::OutputItem(_), "text") => JsonStringTarget::ItemText,
            (JsonObjectRole::OutputItem(_), "kind") => JsonStringTarget::ItemKind,
            (JsonObjectRole::OutputItem(_), "name") => JsonStringTarget::ItemName,
            _ => JsonStringTarget::Value,
        }
    }

    fn finish_string(
        &mut self,
        reader: JsonStringReader,
        visible_delta: &mut String,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) {
        match reader.target {
            JsonStringTarget::Key => {
                if let Some(JsonContext::Object(context)) = self.stack.last_mut() {
                    context.pending_key = Some(reader.decoded);
                    context.expecting_key = false;
                }
            }
            JsonStringTarget::ItemKind => {
                if let Some(item) = self.current_item_mut() {
                    item.kind = Some(reader.decoded);
                    if item.is_assistant() && !item.buffered_text.is_empty() {
                        visible_delta.push_str(&std::mem::take(&mut item.buffered_text));
                    }
                    if let Some(event) = item.tool_call_started_event() {
                        on_event(event);
                    }
                }
                self.complete_scalar_value();
            }
            JsonStringTarget::ItemName => {
                if let Some(item) = self.current_item_mut() {
                    item.name = Some(reader.decoded);
                    if let Some(event) = item.tool_call_started_event() {
                        on_event(event);
                    }
                }
                self.complete_scalar_value();
            }
            JsonStringTarget::ItemText => {
                if let Some(item) = self.current_item_mut()
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
                role: JsonObjectRole::OutputItem(item),
                ..
            }) => item.is_assistant(),
            JsonContext::Object(_) | JsonContext::Array(_) => false,
        })
    }

    fn current_item_mut(&mut self) -> Option<&mut OutputItemState> {
        match self.stack.last_mut()? {
            JsonContext::Object(JsonObjectContext {
                role: JsonObjectRole::OutputItem(item),
                ..
            }) => Some(item),
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
    fn is_root_output_value(&self) -> bool {
        matches!(
            self,
            Self::Object(JsonObjectContext {
                role: JsonObjectRole::Root,
                pending_key: Some(key),
                ..
            }) if key == "output"
        )
    }

    fn is_current_memory_proposals_value(&self) -> bool {
        matches!(
            self,
            Self::Object(JsonObjectContext {
                role: JsonObjectRole::OutputItem(item),
                pending_key: Some(key),
                ..
            }) if key == "proposals" && item.is_memory_proposals()
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
    OutputItem(OutputItemState),
    Nested,
}

#[derive(Debug)]
pub(crate) enum JsonArrayRole {
    Output { next_index: usize },
    MemoryProposals,
    Nested,
}

#[derive(Debug, Default)]
pub(crate) struct OutputItemState {
    output_index: usize,
    kind: Option<String>,
    name: Option<String>,
    buffered_text: String,
    tool_call_started_emitted: bool,
}

impl OutputItemState {
    fn is_assistant(&self) -> bool {
        self.kind.as_deref() == Some("assistant_text")
    }

    fn is_memory_proposals(&self) -> bool {
        self.kind.as_deref() == Some("memory_proposals")
    }

    fn is_tool_call(&self) -> bool {
        self.kind.as_deref() == Some("tool_call")
    }

    fn tool_call_started_event(&mut self) -> Option<GenerateStreamEvent> {
        if !self.is_tool_call() || self.tool_call_started_emitted {
            return None;
        }
        let name = self.name.as_deref()?.trim();
        if name.is_empty() {
            return None;
        }
        self.tool_call_started_emitted = true;
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
    ItemKind,
    ItemName,
    ItemText,
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
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hi"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta("\\nthere\\u00", &mut |event| events.push(event));
        extractor.push_delta(
            "21\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}",
            &mut |event| events.push(event),
        );

        let streamed_text = assistant_text_from_events(&events);
        assert_eq!(streamed_text, "Hi\nthere!");
    }

    #[test]
    fn noema_assistant_text_delta_extractor_handles_text_before_kind() {
        let streamed_text = extract_streamed_text(&[
            r#"{"type":"noema_response","output":[{"text":"Hel"#,
            r#"lo","kind":"assistant_text"},{"kind":"memory_proposals","proposals":[]}]}"#,
        ]);

        assert_eq!(streamed_text, "Hello");
    }

    #[test]
    fn noema_assistant_text_delta_extractor_ignores_assistant_text_phase() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","phase":"commentary","text":"Checking"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta(
            r#" now."},{"kind":"memory_proposals","proposals":[]}]} "#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            assistant_text_from_events(&events),
            "Checking now.".to_string()
        );
    }

    #[test]
    fn noema_assistant_text_delta_extractor_streams_multiple_assistant_items() {
        let streamed_text = extract_streamed_text(&[
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hel"#,
            r#"lo"},{"kind":"assistant_text","text":" again"},{"kind":"memory_proposals","proposals":[]}]}"#,
        ]);

        assert_eq!(streamed_text, "Hello again");
    }

    #[test]
    fn noema_assistant_text_delta_extractor_ignores_nested_payload_text() {
        let streamed_text = extract_streamed_text(&[
            r#"{"type":"noema_response","output":[{"kind":"tool_result","id":"tool_1","name":"search_memory","payload":{"kind":"assistant_text","text":"wrong"}}"#,
            r#",{"kind":"structured","schema":"test","payload":{"output":[{"kind":"assistant_text","text":"also wrong"}]}}"#,
            r#",{"text":"right","kind":"assistant_text"},{"kind":"memory_proposals","proposals":[]}]}"#,
        ]);

        assert_eq!(streamed_text, "right");
    }

    #[test]
    fn noema_assistant_text_delta_extractor_emits_memory_started_for_non_empty_proposals() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"},"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta(
            r#"{"kind":"memory_proposals","proposals":[{"proposal":{"content":"Kevin likes trains.""#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "Hello".to_string()
                },
                GenerateStreamEvent::MemoryProposalsStarted,
            ]
        );
    }

    #[test]
    fn noema_assistant_text_delta_extractor_does_not_emit_memory_started_for_empty_proposals() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"},"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta(
            r#"{"kind":"memory_proposals","proposals":[]}]} "#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: "Hello".to_string()
            }]
        );
    }

    #[test]
    fn noema_assistant_text_delta_extractor_emits_tool_call_started_when_name_streams() {
        let mut extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut events = Vec::new();
        extractor.push_delta(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Need tool"},"#,
            &mut |event| events.push(event),
        );
        extractor.push_delta(
            r#"{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains""#,
            &mut |event| events.push(event),
        );

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
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
            r#"{"type":"noema_response","output":[{"kind":"structured","schema":"test","payload":{"kind":"tool_call","name":"wrong"}}]}"#,
            &mut |event| events.push(event),
        );

        assert_eq!(events, Vec::<GenerateStreamEvent>::new());
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
                GenerateStreamEvent::AssistantTextDelta { delta } => Some(delta.as_str()),
                GenerateStreamEvent::MemoryProposalsStarted => None,
                GenerateStreamEvent::ToolCallStarted { .. } => None,
            })
            .collect()
    }
}
