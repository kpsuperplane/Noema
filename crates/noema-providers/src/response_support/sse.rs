use crate::ProviderError;

pub(crate) struct SseEvent {
    pub(crate) event: Option<String>,
    pub(crate) data: Option<String>,
}

pub(crate) fn parse_sse_event_bytes(raw: &[u8]) -> Result<SseEvent, ProviderError> {
    let raw = std::str::from_utf8(raw).map_err(|source| ProviderError::MalformedResponse {
        message: format!("failed to decode SSE event as UTF-8: {source}"),
    })?;
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
    Ok(SseEvent {
        event,
        data: (!data.is_empty()).then(|| data.join("\n")),
    })
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
