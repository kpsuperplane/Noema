//! Local web UI server for the Noema daemon.

use std::{collections::HashMap, path::PathBuf};

use base64::{Engine as _, engine::general_purpose};
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};
use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

use crate::{
    WebConfig,
    frontend_protocol::{WebClientMessage, WebMemoryStorageStatus, WebServerMessage, WebStatus},
};

use super::{
    protocol::{DaemonError, TurnStreamEvent},
    runtime::CodexRuntimeHandle,
};

const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_WS_FRAME_BYTES: usize = 1024 * 1024;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const INDEX_HTML: &str = include_str!("assets/index.html");
const APP_JS: &str = include_str!("assets/app.js");
const STYLES_CSS: &str = include_str!("assets/styles.css");
const NOEMA_MARK_SVG: &str = include_str!("assets/noema-mark.svg");

/// State shared by local web UI connections.
#[derive(Debug, Clone)]
pub(super) struct WebState {
    runtime: CodexRuntimeHandle,
    database_path: PathBuf,
}

impl WebState {
    /// Build shared web UI state.
    #[must_use]
    pub(super) fn new(runtime: CodexRuntimeHandle, database_path: PathBuf) -> Self {
        Self {
            runtime,
            database_path,
        }
    }
}

/// Bind the local web UI listener.
pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, DaemonError> {
    TcpListener::bind((config.host.as_str(), config.port))
        .await
        .map_err(|source| {
            DaemonError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}

/// Handle one local web UI connection.
pub(super) async fn handle_connection(
    mut stream: TcpStream,
    state: WebState,
) -> Result<(), DaemonError> {
    let request = HttpRequest::read_from(&mut stream).await?;

    if request.method == "GET" && request.path == "/api/chat/ws" {
        upgrade_websocket(stream, &request, state).await?;
        return Ok(());
    }

    if request.method == "GET" && request.path == "/api/status" {
        let status = web_status_from_state(&state);
        let body = serde_json::to_vec(&status)
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
        write_response(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            &body,
        )
        .await?;
        return Ok(());
    }

    if request.method == "GET"
        && let Some(asset) = embedded_asset(&request.path)
    {
        write_response(&mut stream, "200 OK", asset.content_type, asset.body).await?;
        return Ok(());
    }

    write_response(
        &mut stream,
        "404 Not Found",
        "text/plain; charset=utf-8",
        b"not found",
    )
    .await?;
    Ok(())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
}

impl HttpRequest {
    async fn read_from(stream: &mut TcpStream) -> Result<Self, DaemonError> {
        let mut bytes = Vec::with_capacity(1024);
        let mut buffer = [0_u8; 1024];

        loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                return Err(DaemonError::Protocol(
                    "web client closed before sending request".to_string(),
                ));
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if bytes.len() > MAX_HTTP_HEADER_BYTES {
                return Err(DaemonError::Protocol(
                    "web request headers too large".to_string(),
                ));
            }
        }

        let text = std::str::from_utf8(&bytes)
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
        let header_text = text
            .split_once("\r\n\r\n")
            .map_or(text, |(headers, _body)| headers);
        let mut lines = header_text.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| DaemonError::Protocol("missing web request line".to_string()))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| DaemonError::Protocol("missing web request method".to_string()))?;
        let path = request_parts
            .next()
            .ok_or_else(|| DaemonError::Protocol("missing web request path".to_string()))?;

        let mut headers = HashMap::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }

        Ok(Self {
            method: method.to_string(),
            path: normalized_path(path),
            headers,
        })
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

fn normalized_path(path: &str) -> String {
    path.split_once('?')
        .map_or(path, |(path, _query)| path)
        .to_string()
}

struct EmbeddedAsset {
    content_type: &'static str,
    body: &'static [u8],
}

fn embedded_asset(path: &str) -> Option<EmbeddedAsset> {
    let asset = match path {
        "/" | "/index.html" | "/chat" => EmbeddedAsset {
            content_type: "text/html; charset=utf-8",
            body: INDEX_HTML.as_bytes(),
        },
        "/assets/app.js" => EmbeddedAsset {
            content_type: "application/javascript; charset=utf-8",
            body: APP_JS.as_bytes(),
        },
        "/assets/styles.css" => EmbeddedAsset {
            content_type: "text/css; charset=utf-8",
            body: STYLES_CSS.as_bytes(),
        },
        "/assets/noema-mark.svg" => EmbeddedAsset {
            content_type: "image/svg+xml; charset=utf-8",
            body: NOEMA_MARK_SVG.as_bytes(),
        },
        _ => return None,
    };

    Some(asset)
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), DaemonError> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

async fn upgrade_websocket(
    mut stream: TcpStream,
    request: &HttpRequest,
    state: WebState,
) -> Result<(), DaemonError> {
    let key = request
        .header("sec-websocket-key")
        .ok_or_else(|| DaemonError::Protocol("missing websocket key".to_string()))?;
    let upgrade = request.header("upgrade").unwrap_or_default();
    if !upgrade.eq_ignore_ascii_case("websocket") {
        write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"expected websocket upgrade",
        )
        .await?;
        return Ok(());
    }

    let accept = websocket_accept_key(key);
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    handle_websocket(stream, state).await
}

async fn handle_websocket(mut stream: TcpStream, state: WebState) -> Result<(), DaemonError> {
    loop {
        let Some(frame) = WebSocketFrame::read_from(&mut stream).await? else {
            return Ok(());
        };

        match frame.opcode {
            WebSocketOpcode::Text => {
                let text = String::from_utf8(frame.payload)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                let request = serde_json::from_str::<WebClientMessage>(&text)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                handle_websocket_message(&mut stream, &state, request).await?;
            }
            WebSocketOpcode::Ping => {
                write_ws_frame(&mut stream, WebSocketOpcode::Pong, &frame.payload).await?;
            }
            WebSocketOpcode::Pong => {}
        }
    }
}

async fn handle_websocket_message(
    stream: &mut TcpStream,
    state: &WebState,
    request: WebClientMessage,
) -> Result<(), DaemonError> {
    match request {
        WebClientMessage::Start { model, cwd } => {
            match state.runtime.start_conversation(model, cwd).await {
                Ok(started) => {
                    send_ws_json(stream, &WebServerMessage::conversation_started(started)).await?;
                }
                Err(error) => send_ws_error(stream, error.to_string()).await?,
            }
        }
        WebClientMessage::Turn {
            conversation_id,
            input,
            client_message_id,
        } => {
            let (item_tx, mut item_rx) = mpsc::unbounded_channel();
            let completion = state.runtime.turn(conversation_id.clone(), input, item_tx);
            tokio::pin!(completion);

            loop {
                tokio::select! {
                    Some(event) = item_rx.recv() => {
                        send_web_turn_event(stream, event, client_message_id.clone()).await?;
                    }
                    result = &mut completion => {
                        while let Ok(event) = item_rx.try_recv() {
                            send_web_turn_event(stream, event, client_message_id.clone()).await?;
                        }

                        match result {
                            Ok(()) => {
                                send_ws_json(
                                    stream,
                                    &WebServerMessage::TurnCompleted {
                                        conversation_id,
                                        client_message_id,
                                    },
                                ).await?;
                            }
                            Err(error) => send_ws_error(stream, error.to_string()).await?,
                        }
                        break;
                    }
                }
            }
        }
        WebClientMessage::End { conversation_id } => {
            match state.runtime.end_conversation(conversation_id).await {
                Ok(()) => {
                    send_ws_json(
                        stream,
                        &WebServerMessage::Ok {
                            message: Some("conversation ended".to_string()),
                        },
                    )
                    .await?;
                }
                Err(error) => send_ws_error(stream, error.to_string()).await?,
            }
        }
    }

    Ok(())
}

async fn send_web_turn_event(
    stream: &mut TcpStream,
    event: TurnStreamEvent,
    client_message_id: Option<String>,
) -> Result<(), DaemonError> {
    let message = match event {
        TurnStreamEvent::ConversationItem {
            conversation_id,
            item_id,
            turn_id,
            item,
        } => WebServerMessage::ConversationItem {
            conversation_id,
            client_message_id,
            item_id,
            turn_id,
            item,
        },
        TurnStreamEvent::AgentStatusChanged {
            conversation_id,
            status,
        } => WebServerMessage::AgentStatusChanged {
            conversation_id,
            status,
        },
    };
    send_ws_json(stream, &message).await
}

async fn send_ws_error(stream: &mut TcpStream, message: String) -> Result<(), DaemonError> {
    send_ws_json(stream, &WebServerMessage::Error { message }).await
}

async fn send_ws_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let bytes =
        serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_ws_frame(stream, WebSocketOpcode::Text, &bytes).await
}

fn websocket_accept_key(key: &str) -> String {
    let mut value = String::with_capacity(key.len() + WEBSOCKET_GUID.len());
    value.push_str(key.trim());
    value.push_str(WEBSOCKET_GUID);
    let digest = digest(&SHA1_FOR_LEGACY_USE_ONLY, value.as_bytes());
    general_purpose::STANDARD.encode(digest.as_ref())
}

#[derive(Debug)]
struct WebSocketFrame {
    opcode: WebSocketOpcode,
    payload: Vec<u8>,
}

impl WebSocketFrame {
    async fn read_from(stream: &mut TcpStream) -> Result<Option<Self>, DaemonError> {
        let mut header = [0_u8; 2];
        match stream.read_exact(&mut header).await {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error.into()),
        }

        let opcode = match header[0] & 0x0f {
            0x1 => WebSocketOpcode::Text,
            0x8 => return Ok(None),
            0x9 => WebSocketOpcode::Ping,
            0xA => WebSocketOpcode::Pong,
            other => {
                return Err(DaemonError::Protocol(format!(
                    "unsupported websocket opcode: {other}"
                )));
            }
        };
        let masked = header[1] & 0x80 != 0;
        if !masked {
            return Err(DaemonError::Protocol(
                "client websocket frame was not masked".to_string(),
            ));
        }

        let payload_len = read_payload_len(stream, header[1] & 0x7f).await?;
        if payload_len > MAX_WS_FRAME_BYTES {
            return Err(DaemonError::Protocol(
                "websocket frame too large".to_string(),
            ));
        }

        let mut mask = [0_u8; 4];
        stream.read_exact(&mut mask).await?;
        let mut payload = vec![0_u8; payload_len];
        stream.read_exact(&mut payload).await?;
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % mask.len()];
        }

        Ok(Some(Self { opcode, payload }))
    }
}

async fn read_payload_len(stream: &mut TcpStream, initial: u8) -> Result<usize, DaemonError> {
    match initial {
        len @ 0..=125 => Ok(usize::from(len)),
        126 => {
            let mut bytes = [0_u8; 2];
            stream.read_exact(&mut bytes).await?;
            Ok(usize::from(u16::from_be_bytes(bytes)))
        }
        127 => {
            let mut bytes = [0_u8; 8];
            stream.read_exact(&mut bytes).await?;
            usize::try_from(u64::from_be_bytes(bytes)).map_err(|_| {
                DaemonError::Protocol("websocket frame length exceeds platform size".to_string())
            })
        }
        _ => unreachable!("masked bit is stripped before payload length parsing"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebSocketOpcode {
    Text,
    Ping,
    Pong,
}

async fn write_ws_frame(
    stream: &mut TcpStream,
    opcode: WebSocketOpcode,
    payload: &[u8],
) -> Result<(), DaemonError> {
    let opcode = match opcode {
        WebSocketOpcode::Text => 0x1,
        WebSocketOpcode::Ping => 0x9,
        WebSocketOpcode::Pong => 0xA,
    };
    let mut header = Vec::with_capacity(10);
    header.push(0x80 | opcode);
    match payload.len() {
        len @ 0..=125 => header.push(u8::try_from(len).expect("small websocket payload")),
        len @ 126..=65_535 => {
            header.push(126);
            header.extend_from_slice(
                &u16::try_from(len)
                    .expect("medium websocket payload")
                    .to_be_bytes(),
            );
        }
        len => {
            header.push(127);
            header.extend_from_slice(
                &u64::try_from(len)
                    .expect("large websocket payload")
                    .to_be_bytes(),
            );
        }
    }

    stream.write_all(&header).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

fn web_status_from_state(state: &WebState) -> WebStatus {
    WebStatus::new(if state.database_path.is_file() {
        WebMemoryStorageStatus::Ready
    } else {
        WebMemoryStorageStatus::Initializing
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn websocket_accept_key_matches_rfc_example() {
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn websocket_client_message_uses_snake_case_tag() {
        let message = serde_json::from_value::<WebClientMessage>(json!({
            "type": "conversation_turn",
            "conversation_id": "conversation_1",
            "input": "hello",
            "client_message_id": "client_1"
        }))
        .expect("message");

        assert!(matches!(
            message,
            WebClientMessage::Turn {
                conversation_id,
                input,
                client_message_id: Some(client_message)
            } if conversation_id == "conversation_1"
                && input == "hello"
                && client_message == "client_1"
        ));
    }
}
