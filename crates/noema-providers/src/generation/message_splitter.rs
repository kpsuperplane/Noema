use std::ops::Range;

#[derive(Debug, Default)]
pub(crate) struct MarkdownMessageDeltaSplitter {
    pending: String,
    leading: String,
    fence: Option<(char, usize)>,
    segment_index: usize,
    segment_has_content: bool,
    consumed_utf16: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownMessageSegment {
    pub(crate) text: String,
    pub(crate) source_utf16: Range<usize>,
}

#[derive(Debug)]
struct MarkdownMessageChunk {
    segment_index: usize,
    text: String,
    source_utf16: Range<usize>,
}

impl MarkdownMessageDeltaSplitter {
    pub(crate) fn push(&mut self, delta: &str) -> Vec<(usize, String)> {
        self.push_chunks(delta)
            .into_iter()
            .map(|chunk| (chunk.segment_index, chunk.text))
            .collect()
    }

    fn push_chunks(&mut self, delta: &str) -> Vec<MarkdownMessageChunk> {
        self.pending.push_str(delta);
        let mut output = Vec::new();
        while let Some(newline) = self.pending.find('\n') {
            let line = self.pending.drain(..=newline).collect::<String>();
            self.process_line(line, &mut output);
        }
        output
    }

    pub(crate) fn finish(&mut self) -> Vec<(usize, String)> {
        self.finish_chunks()
            .into_iter()
            .map(|chunk| (chunk.segment_index, chunk.text))
            .collect()
    }

    fn finish_chunks(&mut self) -> Vec<MarkdownMessageChunk> {
        let mut output = Vec::new();
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.process_line(line, &mut output);
        }
        self.leading.clear();
        output
    }

    fn process_line(&mut self, line: String, output: &mut Vec<MarkdownMessageChunk>) {
        let source_start = self.consumed_utf16;
        self.consumed_utf16 = self
            .consumed_utf16
            .saturating_add(line.encode_utf16().count());
        let content = line.trim_end_matches(['\r', '\n']);
        if self.fence.is_none() && content == "---" {
            self.leading.clear();
            if self.segment_has_content {
                self.segment_index = self.segment_index.saturating_add(1);
                self.segment_has_content = false;
            }
            return;
        }

        if self.fence.is_none() && content.trim().is_empty() {
            self.leading.clear();
            if self.segment_has_content {
                self.segment_index = self.segment_index.saturating_add(1);
                self.segment_has_content = false;
            }
            return;
        }

        self.update_fence(content);
        if !self.segment_has_content && content.trim().is_empty() {
            self.leading.push_str(&line);
            return;
        }
        if !self.segment_has_content {
            self.segment_has_content = true;
            self.leading.clear();
        }
        output.push(MarkdownMessageChunk {
            segment_index: self.segment_index,
            text: line,
            source_utf16: source_start..self.consumed_utf16,
        });
    }

    fn update_fence(&mut self, line: &str) {
        let trimmed = line.trim_start_matches(' ');
        if line.len().saturating_sub(trimmed.len()) > 3 {
            return;
        }
        let Some(marker) = trimmed
            .chars()
            .next()
            .filter(|marker| matches!(marker, '`' | '~'))
        else {
            return;
        };
        let count = trimmed
            .chars()
            .take_while(|character| *character == marker)
            .count();
        if count < 3 {
            return;
        }
        match self.fence {
            None => self.fence = Some((marker, count)),
            Some((open_marker, minimum))
                if marker == open_marker
                    && count >= minimum
                    && trimmed[count..].trim().is_empty() =>
            {
                self.fence = None;
            }
            Some(_) => {}
        }
    }
}

#[cfg(test)]
fn split_markdown_messages(text: &str) -> Vec<String> {
    split_markdown_message_segments(text)
        .into_iter()
        .map(|segment| segment.text)
        .collect()
}

pub(crate) fn split_markdown_message_segments(text: &str) -> Vec<MarkdownMessageSegment> {
    let mut splitter = MarkdownMessageDeltaSplitter::default();
    let mut segments = Vec::<MarkdownMessageSegment>::new();
    for chunk in splitter
        .push_chunks(text)
        .into_iter()
        .chain(splitter.finish_chunks())
    {
        if segments.len() <= chunk.segment_index {
            segments.resize_with(chunk.segment_index + 1, || MarkdownMessageSegment {
                text: String::new(),
                source_utf16: chunk.source_utf16.start..chunk.source_utf16.start,
            });
        }
        let segment = &mut segments[chunk.segment_index];
        if segment.text.is_empty() {
            segment.source_utf16.start = chunk.source_utf16.start;
        }
        segment.text.push_str(&chunk.text);
        segment.source_utf16.end = chunk.source_utf16.end;
    }
    segments
        .into_iter()
        .filter_map(|mut segment| {
            let trimmed = segment.text.trim_end_matches(['\r', '\n']);
            let removed_utf16 = segment.text[trimmed.len()..].encode_utf16().count();
            segment.source_utf16.end = segment.source_utf16.end.saturating_sub(removed_utf16);
            segment.text.truncate(trimmed.len());
            (!segment.text.trim().is_empty()).then_some(segment)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_boundaries_split_across_chunks_but_fenced_rules_do_not() {
        let cases = [
            ("one\n---\ntwo", vec!["one", "two"]),
            ("one\n\ntwo", vec!["one", "two"]),
            ("---\n\none\n---\n---\n\ntwo\n---", vec!["one", "two"]),
            (
                "```md\n---\n```\n---\nafter",
                vec!["```md\n---\n```", "after"],
            ),
            ("```md\none\n\ntwo\n```", vec!["```md\none\n\ntwo\n```"]),
            ("~~~\n---\n~~~\n---\nafter", vec!["~~~\n---\n~~~", "after"]),
            ("before\n----\nafter", vec!["before\n----\nafter"]),
        ];
        for (input, expected) in cases {
            assert_eq!(split_markdown_messages(input), expected, "{input:?}");
        }

        let mut splitter = MarkdownMessageDeltaSplitter::default();
        let mut output = splitter.push("one\n--");
        output.extend(splitter.push("-\ntwo"));
        output.extend(splitter.finish());
        assert_eq!(
            output,
            vec![(0, "one\n".to_string()), (1, "two".to_string())]
        );
    }

    #[test]
    fn segment_ranges_preserve_utf16_offsets_and_removed_separators() {
        let segments = split_markdown_message_segments("😀 first\n\nsecond\n---\nthird");

        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>(),
            ["😀 first", "second", "third"]
        );
        assert_eq!(segments[0].source_utf16, 0..8);
        assert_eq!(segments[1].source_utf16, 10..16);
        assert_eq!(segments[2].source_utf16, 21..26);
    }
}
