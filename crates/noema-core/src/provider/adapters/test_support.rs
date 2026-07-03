//! Shared test helpers for Responses API adapter tests.
//!
//! Provides a minimal one-shot HTTP server that captures a single request and
//! replies with a canned response, used by the OpenAI and Codex adapter tests.

use std::collections::HashMap;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

/// A single HTTP request captured by [`spawn_server`].
pub(crate) struct CapturedRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: String,
}

/// Spawn a one-shot HTTP server that captures the first request and replies
/// with `status` and `response_body`. Returns the base URL and a receiver for
/// the captured request.
pub(crate) async fn spawn_server(
    status: u16,
    response_body: impl Into<String>,
) -> (String, oneshot::Receiver<CapturedRequest>) {
    let response_body = response_body.into();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (request_tx, request_rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let request = read_request(&mut socket).await;
        let _ = request_tx.send(request);

        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            429 => "Too Many Requests",
            _ => "Error",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("write response");
    });

    (format!("http://{addr}"), request_rx)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> CapturedRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = socket.read(&mut buffer).await.expect("read request");
        assert_ne!(read, 0, "client closed before complete request");
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            let text = String::from_utf8_lossy(&bytes);
            if let Some(content_length) = parse_content_length(&text) {
                let header_end = bytes
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .expect("header end")
                    + 4;
                if bytes.len() >= header_end + content_length {
                    break;
                }
            }
        }
    }

    parse_request(&bytes)
}

fn parse_request(bytes: &[u8]) -> CapturedRequest {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("header end");
    let headers_text = String::from_utf8(bytes[..header_end].to_vec()).expect("headers utf8");
    let body = String::from_utf8(bytes[header_end + 4..].to_vec()).expect("body utf8");
    let mut lines = headers_text.lines();
    let request_line = lines.next().expect("request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().expect("method").to_string();
    let path = request_parts.next().expect("path").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }

    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn parse_content_length(text: &str) -> Option<usize> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.eq_ignore_ascii_case("content-length"))
            .then(|| value.trim().parse().ok())
            .flatten()
    })
}
