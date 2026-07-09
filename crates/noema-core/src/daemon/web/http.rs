use std::collections::HashMap;

use serde::Serialize;
use std::borrow::Cow;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use super::DaemonError;

const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
pub(super) const MAX_API_BODY_BYTES: usize = 64 * 1024;
const HTTP_BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Debug)]
pub(super) struct HttpRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) query: Option<String>,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Vec<u8>,
}

impl HttpRequest {
    pub(super) async fn read_from(stream: &mut TcpStream) -> Result<Self, HttpRequestError> {
        let mut bytes = Vec::with_capacity(1024);
        let mut buffer = [0_u8; 1024];

        loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                return Err(HttpRequestError::bad_request("missing web request"));
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if bytes.len() > MAX_HTTP_HEADER_BYTES {
                return Err(HttpRequestError::payload_too_large(
                    "request headers too large",
                ));
            }
        }

        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| HttpRequestError::bad_request("missing web request headers"))?;
        let body_start = header_end + 4;
        let text = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| HttpRequestError::bad_request("invalid request headers"))?;
        let mut lines = text.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request line"))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request method"))?;
        let path = request_parts
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request path"))?;

        let mut headers = HashMap::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            if name == "content-length" && headers.contains_key(&name) {
                return Err(HttpRequestError::bad_request("duplicate content length"));
            }
            headers.insert(name, value.trim().to_string());
        }

        let content_length = content_length(&headers)?;
        let mut body = bytes[body_start..].to_vec();
        if content_length > MAX_API_BODY_BYTES {
            return Err(HttpRequestError::payload_too_large(
                "request body too large",
            ));
        }
        if body.len() > content_length {
            return Err(HttpRequestError::bad_request("unexpected request bytes"));
        }
        while body.len() < content_length {
            let read = tokio::time::timeout(HTTP_BODY_READ_TIMEOUT, stream.read(&mut buffer))
                .await
                .map_err(|_| HttpRequestError::bad_request("request body timed out"))??;
            if read == 0 {
                return Err(HttpRequestError::bad_request("incomplete request body"));
            }
            body.extend_from_slice(&buffer[..read]);
            if body.len() > content_length {
                return Err(HttpRequestError::bad_request("unexpected request bytes"));
            }
        }

        Ok(Self {
            method: method.to_string(),
            path: normalized_path(path),
            query: path.split_once('?').map(|(_path, query)| query.to_string()),
            headers,
            body,
        })
    }

    pub(super) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Debug)]
pub(super) struct HttpRequestError {
    status: &'static str,
    message: &'static str,
}

impl HttpRequestError {
    pub(super) const fn bad_request(message: &'static str) -> Self {
        Self {
            status: "400 Bad Request",
            message,
        }
    }

    const fn payload_too_large(message: &'static str) -> Self {
        Self {
            status: "413 Payload Too Large",
            message,
        }
    }

    pub(super) const fn status(&self) -> &'static str {
        self.status
    }

    pub(super) const fn message(&self) -> &'static str {
        self.message
    }
}

impl From<std::io::Error> for HttpRequestError {
    fn from(_source: std::io::Error) -> Self {
        Self::bad_request("invalid request")
    }
}

pub(super) fn content_length(headers: &HashMap<String, String>) -> Result<usize, HttpRequestError> {
    let Some(value) = headers.get("content-length") else {
        return Ok(0);
    };
    value
        .parse::<usize>()
        .map_err(|_| HttpRequestError::bad_request("invalid content length"))
}

pub(super) fn normalized_path(path: &str) -> String {
    path.split_once('?')
        .map_or(path, |(path, _query)| path)
        .to_string()
}

pub(super) async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), DaemonError> {
    let content_type = safe_header_value_or_default(content_type);
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

pub(super) async fn write_binary_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    content_disposition: Option<&str>,
    body: &[u8],
) -> Result<(), DaemonError> {
    let content_type = safe_header_value_or_default(content_type);
    let mut headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(content_disposition) = content_disposition {
        headers.push_str("Content-Disposition: ");
        headers.push_str(content_disposition);
        headers.push_str("\r\n");
    }
    headers.push_str("\r\n");
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

fn safe_header_value_or_default(value: &str) -> Cow<'_, str> {
    if value.is_empty() {
        return Cow::Borrowed("application/octet-stream");
    }
    if http::HeaderValue::from_str(value).is_ok() {
        Cow::Borrowed(value)
    } else {
        Cow::Borrowed("application/octet-stream")
    }
}

pub(super) fn attachment_content_disposition(filename: &str) -> String {
    let escaped = filename
        .chars()
        .map(|character| match character {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            _ if character.is_control() => "_".to_string(),
            _ => character.to_string(),
        })
        .collect::<String>();
    format!("attachment; filename=\"{escaped}\"")
}

pub(super) async fn write_json<T: Serialize>(
    stream: &mut TcpStream,
    status: &str,
    value: &T,
) -> Result<(), DaemonError> {
    let body =
        serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_response(stream, status, "application/json; charset=utf-8", &body).await
}

pub(super) async fn write_json_error(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<(), DaemonError> {
    write_json(stream, status, &serde_json::json!({ "error": message })).await
}

#[cfg(test)]
mod tests {
    use super::{attachment_content_disposition, safe_header_value_or_default};

    #[test]
    fn attachment_content_disposition_escapes_quotes_and_controls() {
        assert_eq!(
            attachment_content_disposition("report\"\r\nv1.md"),
            "attachment; filename=\"report\\\"__v1.md\""
        );
    }

    #[test]
    fn invalid_content_type_falls_back_to_octet_stream() {
        assert_eq!(
            safe_header_value_or_default("text/plain\r\nX-Evil: yes"),
            "application/octet-stream"
        );
        assert_eq!(
            safe_header_value_or_default("text/markdown"),
            "text/markdown"
        );
    }
}
