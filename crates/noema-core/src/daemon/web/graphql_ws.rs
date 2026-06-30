use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use super::{
    DaemonError, WebState,
    http::{HttpRequest, write_response},
};

const MAX_WS_FRAME_BYTES: usize = 1024 * 1024;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const GRAPHQL_WS_PROTOCOL: &str = "graphql-transport-ws";

pub(super) async fn upgrade_graphql_websocket(
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

    let response = graphql_websocket_upgrade_response(
        key,
        request.header("sec-websocket-protocol").unwrap_or_default(),
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    handle_graphql_websocket(stream, state).await
}

pub(super) async fn handle_graphql_websocket(
    mut stream: TcpStream,
    state: WebState,
) -> Result<(), DaemonError> {
    let schema = crate::graphql::build_schema(crate::graphql::GraphqlState::from_web_state(state));
    loop {
        let Some(frame) = WebSocketFrame::read_from(&mut stream).await? else {
            return Ok(());
        };

        match frame.opcode {
            WebSocketOpcode::Text => {
                let text = String::from_utf8(frame.payload)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                let message = serde_json::from_str::<Value>(&text)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                let message_type =
                    message.get("type").and_then(Value::as_str).ok_or_else(|| {
                        DaemonError::Protocol("missing GraphQL websocket message type".to_string())
                    })?;

                match message_type {
                    "connection_init" => {
                        send_ws_json(&mut stream, &json!({ "type": "connection_ack" })).await?;
                    }
                    "subscribe" => {
                        let id = message
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("1")
                            .to_string();
                        let payload = message.get("payload").cloned().ok_or_else(|| {
                            DaemonError::Protocol("missing GraphQL websocket payload".to_string())
                        })?;
                        let request = serde_json::from_value::<async_graphql::Request>(payload)
                            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                        let mut responses = schema.execute_stream(request);
                        while let Some(response) = responses.next().await {
                            send_ws_json(
                                &mut stream,
                                &json!({
                                    "id": id,
                                    "type": "next",
                                    "payload": response,
                                }),
                            )
                            .await?;
                        }
                        send_ws_json(
                            &mut stream,
                            &json!({
                                "id": id,
                                "type": "complete",
                            }),
                        )
                        .await?;
                    }
                    "complete" => return Ok(()),
                    _ => {
                        send_ws_json(
                            &mut stream,
                            &json!({
                                "type": "error",
                                "payload": [
                                    { "message": "unsupported GraphQL websocket message type" }
                                ],
                            }),
                        )
                        .await?;
                    }
                }
            }
            WebSocketOpcode::Ping => {
                write_ws_frame(&mut stream, WebSocketOpcode::Pong, &frame.payload).await?;
            }
            WebSocketOpcode::Pong => {}
        }
    }
}

async fn send_ws_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let bytes =
        serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_ws_frame(stream, WebSocketOpcode::Text, &bytes).await
}

pub(super) fn websocket_accept_key(key: &str) -> String {
    let mut value = String::with_capacity(key.len() + WEBSOCKET_GUID.len());
    value.push_str(key.trim());
    value.push_str(WEBSOCKET_GUID);
    let digest = digest(&SHA1_FOR_LEGACY_USE_ONLY, value.as_bytes());
    general_purpose::STANDARD.encode(digest.as_ref())
}

pub(super) fn graphql_websocket_upgrade_response(key: &str, requested_protocols: &str) -> String {
    let accept = websocket_accept_key(key);
    let protocol_header = websocket_protocol_header(requested_protocols);
    format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n{protocol_header}\r\n"
    )
}

fn websocket_protocol_header(requested_protocols: &str) -> &'static str {
    if requested_protocols
        .split(',')
        .map(str::trim)
        .any(|protocol| protocol == GRAPHQL_WS_PROTOCOL)
    {
        "Sec-WebSocket-Protocol: graphql-transport-ws\r\n"
    } else {
        ""
    }
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
