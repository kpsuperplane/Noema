use super::http::{HttpRequest, HttpRequestError};

pub(super) fn validate_json_post_request(request: &HttpRequest) -> Result<(), HttpRequestError> {
    validate_mutation_request(request)?;
    let content_type = request.header("content-type").unwrap_or_default();
    let content_type = content_type.split(';').next().unwrap_or_default().trim();
    if !content_type.eq_ignore_ascii_case("application/json") {
        return Err(HttpRequestError::bad_request("expected JSON request"));
    }
    Ok(())
}

pub(super) fn validate_mutation_request(request: &HttpRequest) -> Result<(), HttpRequestError> {
    let Some(origin) = request.header("origin") else {
        return Ok(());
    };
    let host = request.header("host").unwrap_or_default();
    if origin_matches_host(origin, host) {
        Ok(())
    } else {
        Err(HttpRequestError::bad_request("invalid request origin"))
    }
}

// Browser POSTs should be same-origin. Keep this intentionally small for the
// local daemon: exact host matches are accepted, as are localhost aliases with
// the same port.
pub(super) fn origin_matches_host(origin: &str, host: &str) -> bool {
    let Some(origin_host) = origin_authority(origin) else {
        return false;
    };
    origin_host.eq_ignore_ascii_case(host) || local_authorities_match(origin_host, host)
}

pub(super) fn origin_authority(origin: &str) -> Option<&str> {
    let authority = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    Some(authority.split('/').next().unwrap_or_default())
}

pub(super) fn local_authorities_match(left: &str, right: &str) -> bool {
    let (left_host, left_port) = split_authority(left);
    let (right_host, right_port) = split_authority(right);
    left_port == right_port && is_local_host(left_host) && is_local_host(right_host)
}

pub(super) fn split_authority(authority: &str) -> (&str, Option<&str>) {
    authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)))
}

pub(super) fn is_local_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}
