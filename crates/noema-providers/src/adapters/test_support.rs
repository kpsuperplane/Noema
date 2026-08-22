//! Shared test helpers for provider HTTP integration tests.
//!
//! Provides a minimal one-shot HTTP server that captures a single request and
//! replies with a canned response.

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug)]
struct StaticCodexCredentials {
    access_token: String,
    refresh_token: String,
}

impl crate::ProviderCredentialAccess for StaticCodexCredentials {
    fn api_key<'a>(
        &'a self,
        _provider_kind: &'a str,
        _provider_account_id: &'a str,
    ) -> crate::ProviderCredentialFuture<'a> {
        Box::pin(async {
            Err(crate::ProviderError::MissingCredentials {
                provider: "exa".to_string(),
                credential: "provider account".to_string(),
            })
        })
    }

    fn codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> crate::ProviderCredentialFuture<'a> {
        let token = self.access_token.clone();
        Box::pin(async move { Ok(crate::ProviderCredential::from(token)) })
    }

    fn refresh_codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> crate::ProviderCredentialFuture<'a> {
        let token = self.refresh_token.clone();
        Box::pin(async move { Ok(crate::ProviderCredential::from(token)) })
    }
}

pub(crate) fn static_codex_credentials(
    access_token: impl Into<String>,
    refresh_token: impl Into<String>,
) -> crate::ProviderCredentialAccessHandle {
    std::sync::Arc::new(StaticCodexCredentials {
        access_token: access_token.into(),
        refresh_token: refresh_token.into(),
    })
}

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
        write_json_response(&mut socket, status, &response_body).await;
    });

    (format!("http://{addr}"), request_rx)
}

/// Spawn a server that captures one request for each canned response in order.
pub(crate) async fn spawn_scripted_server<B>(
    responses: impl IntoIterator<Item = (u16, B)>,
) -> (String, oneshot::Receiver<Vec<CapturedRequest>>)
where
    B: Into<String>,
{
    let responses = responses
        .into_iter()
        .map(|(status, body)| (status, body.into()))
        .collect::<Vec<_>>();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (requests_tx, requests_rx) = oneshot::channel();

    tokio::spawn(async move {
        let mut requests = Vec::with_capacity(responses.len());
        for (status, body) in responses {
            let (mut socket, _) = listener.accept().await.expect("accept");
            requests.push(read_request(&mut socket).await);
            write_json_response(&mut socket, status, &body).await;
        }
        let _ = requests_tx.send(requests);
    });

    (format!("http://{addr}"), requests_rx)
}

/// Spawn one WebSocket connection with one event script for each request.
pub(crate) async fn spawn_websocket_server(
    scripts: Vec<Vec<serde_json::Value>>,
) -> (String, oneshot::Receiver<Vec<serde_json::Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (requests_tx, requests_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        let mut socket = accept_async(socket).await.expect("WebSocket handshake");
        let mut requests = Vec::with_capacity(scripts.len());
        for events in scripts {
            let message = socket
                .next()
                .await
                .expect("request message")
                .expect("valid request message");
            let text = message.into_text().expect("text request");
            requests.push(serde_json::from_str(&text).expect("JSON request"));
            for event in events {
                socket
                    .send(Message::Text(event.to_string().into()))
                    .await
                    .expect("send event");
            }
        }
        let _ = requests_tx.send(requests);
    });
    (format!("http://{addr}"), requests_rx)
}

pub(crate) async fn spawn_blocking_websocket_server() -> (String, oneshot::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        let mut socket = accept_async(socket).await.expect("WebSocket handshake");
        socket
            .next()
            .await
            .expect("request message")
            .expect("valid request message");
        request_tx.send(()).expect("signal request");
        std::future::pending::<()>().await;
    });
    (format!("http://{addr}"), request_rx)
}

/// Spawn a scripted server whose final response waits for an explicit release.
pub(crate) async fn spawn_blocking_server(
    leading_responses: Vec<(u16, String)>,
    final_status: u16,
    final_body: impl Into<String>,
) -> (String, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let final_body = final_body.into();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    tokio::spawn(async move {
        for (status, body) in leading_responses {
            let (mut socket, _) = listener.accept().await.expect("accept");
            read_request(&mut socket).await;
            write_json_response(&mut socket, status, &body).await;
        }
        let (mut socket, _) = listener.accept().await.expect("accept blocked request");
        read_request(&mut socket).await;
        started_tx.send(()).expect("signal request");
        release_rx.await.expect("release response");
        write_json_response(&mut socket, final_status, &final_body).await;
    });
    (format!("http://{addr}"), started_rx, release_tx)
}

pub(crate) async fn write_json_response(
    socket: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    socket
        .write_all(response.as_bytes())
        .await
        .expect("write response");
}

pub(crate) async fn read_request(socket: &mut tokio::net::TcpStream) -> CapturedRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = socket.read(&mut buffer).await.expect("read request");
        assert_ne!(read, 0, "client closed before complete request");
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            let text = String::from_utf8_lossy(&bytes);
            let content_length = parse_content_length(&text).unwrap_or(0);
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
