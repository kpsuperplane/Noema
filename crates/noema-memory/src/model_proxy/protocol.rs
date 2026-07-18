//! Bounded HTTP/1.1 framing for the private loopback proxy.

use serde_json::{Value, json};
use tokio::{io::AsyncReadExt, net::TcpStream};

use super::MemoryModelProxyError;

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl HttpRequest {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _value)| key.eq_ignore_ascii_case(name))
            .map(|(_key, value)| value.as_str())
    }
}

pub(crate) async fn read_http_request(
    stream: &mut TcpStream,
) -> Result<HttpRequest, MemoryModelProxyError> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(MemoryModelProxyError::Protocol(
                "connection closed before request headers".to_string(),
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(MemoryModelProxyError::Protocol(
                "request exceeded maximum size".to_string(),
            ));
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };
    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| MemoryModelProxyError::Protocol("headers are not utf-8".to_string()))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| MemoryModelProxyError::Protocol("missing request line".to_string()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| MemoryModelProxyError::Protocol("missing method".to_string()))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| MemoryModelProxyError::Protocol("missing path".to_string()))?
        .to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    let content_length = headers
        .iter()
        .find(|(name, _value)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_name, value)| value.parse::<usize>())
        .transpose()
        .map_err(|_| MemoryModelProxyError::Protocol("invalid content-length".to_string()))?
        .unwrap_or(0);
    if content_length > MAX_REQUEST_BYTES {
        return Err(MemoryModelProxyError::Protocol(
            "request body exceeded maximum size".to_string(),
        ));
    }
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(MemoryModelProxyError::Protocol(
                "connection closed before request body".to_string(),
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > body_start + content_length {
            break;
        }
    }
    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

pub(crate) struct HttpResponse {
    status: http::StatusCode,
    body: Vec<u8>,
}

impl HttpResponse {
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let reason = self.status.canonical_reason().unwrap_or("Unknown");
        let mut bytes = format!(
            "HTTP/1.1 {} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            self.status.as_u16(),
            self.body.len()
        )
        .into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

pub(crate) fn json_response(status: http::StatusCode, value: Value) -> HttpResponse {
    HttpResponse {
        status,
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

pub(super) fn json_error(status: http::StatusCode, code: &str) -> HttpResponse {
    json_error_message(status, code, code.to_string())
}

pub(super) fn json_error_message(
    status: http::StatusCode,
    code: &str,
    message: impl Into<String>,
) -> HttpResponse {
    json_response(
        status,
        json!({
            "error": {
                "type": "noema_memory_model_proxy_error",
                "code": code,
                "message": message.into()
            }
        }),
    )
}
