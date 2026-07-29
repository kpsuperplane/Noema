#[derive(Debug, Default)]
pub(crate) struct MarkdownMessageDeltaSplitter {
    pending: String,
    leading: String,
    fence: Option<(char, usize)>,
    segment_index: usize,
    segment_has_content: bool,
}

impl MarkdownMessageDeltaSplitter {
    pub(crate) fn push(&mut self, delta: &str) -> Vec<(usize, String)> {
        self.pending.push_str(delta);
        let mut output = Vec::new();
        while let Some(newline) = self.pending.find('\n') {
            let line = self.pending.drain(..=newline).collect::<String>();
            self.process_line(line, &mut output);
        }
        output
    }

    pub(crate) fn finish(&mut self) -> Vec<(usize, String)> {
        let mut output = Vec::new();
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.process_line(line, &mut output);
        }
        self.leading.clear();
        output
    }

    fn process_line(&mut self, line: String, output: &mut Vec<(usize, String)>) {
        let content = line.trim_end_matches(['\r', '\n']);
        if self.fence.is_none() && content == "---" {
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
        output.push((self.segment_index, line));
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

pub(crate) fn split_markdown_messages(text: &str) -> Vec<String> {
    let mut splitter = MarkdownMessageDeltaSplitter::default();
    let mut segments = Vec::<String>::new();
    for (index, delta) in splitter.push(text).into_iter().chain(splitter.finish()) {
        if segments.len() <= index {
            segments.resize_with(index + 1, String::new);
        }
        segments[index].push_str(&delta);
    }
    segments
        .into_iter()
        .map(|segment| segment.trim_end_matches(['\r', '\n']).to_string())
        .filter(|segment| !segment.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_delimiters_split_across_chunks_but_fenced_rules_do_not() {
        let cases = [
            ("one\n---\ntwo", vec!["one", "two"]),
            ("---\n\none\n---\n---\n\ntwo\n---", vec!["one", "two"]),
            (
                "```md\n---\n```\n---\nafter",
                vec!["```md\n---\n```", "after"],
            ),
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
}
