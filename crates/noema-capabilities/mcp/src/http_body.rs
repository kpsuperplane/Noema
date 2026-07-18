//! Shared bounded response-body reader for MCP and OAuth HTTP clients.

pub(crate) enum BodyReadError {
    Request(reqwest::Error),
    Limit,
}

pub(crate) async fn bounded_response_body(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, BodyReadError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(BodyReadError::Limit);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(BodyReadError::Request)? {
        if !append_bounded(&mut body, &chunk, max_bytes) {
            return Err(BodyReadError::Limit);
        }
    }
    Ok(body)
}

pub(crate) fn append_bounded(body: &mut Vec<u8>, chunk: &[u8], max_bytes: usize) -> bool {
    if chunk.len() > max_bytes.saturating_sub(body.len()) {
        return false;
    }
    body.extend_from_slice(chunk);
    true
}
