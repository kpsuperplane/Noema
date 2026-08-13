//! Authenticated HTTPS and WebSocket GraphQL transport for remote desktop mode.

use std::{sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use reqwest::{StatusCode, header};
use serde_json::{Value, json};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Error as WebSocketError, Message,
        client::IntoClientRequest,
        http::{HeaderValue, header as websocket_header},
    },
};

use crate::{desktop_profile::RemoteProfile, remote_pairing::trusted_origin};

const MAX_GRAPHQL_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const RECONNECT_DELAYS: [u64; 5] = [1, 2, 5, 10, 30];
const WEBSOCKET_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct RemoteGraphql {
    origin: Arc<str>,
    client_id: Arc<str>,
    token: Arc<str>,
    http: reqwest::Client,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RemoteError {
    Unauthorized,
    Offline,
    InvalidResponse,
}

pub(crate) enum RemoteSubscriptionEvent {
    Next(Value),
    Offline,
    Unauthorized,
    Complete,
}

enum SubscriptionEnd {
    Complete,
    Transient,
    Unauthorized,
}

impl RemoteGraphql {
    pub(crate) fn new(profile: RemoteProfile) -> Result<Self, String> {
        let origin = trusted_origin(&profile.metadata.origin)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| "Noema could not prepare the remote connection.".to_string())?;
        Ok(Self {
            origin: origin.into(),
            client_id: profile.metadata.client_id.into(),
            token: profile.token.into(),
            http,
        })
    }

    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    pub(crate) fn client_id(&self) -> &str {
        &self.client_id
    }

    pub(crate) async fn execute(&self, request: Value) -> Result<Value, RemoteError> {
        let request = self.build_graphql_request(request)?;
        let response = self
            .http
            .execute(request)
            .await
            .map_err(|_| RemoteError::Offline)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(RemoteError::Unauthorized);
        }
        if !response.status().is_success() {
            return Err(RemoteError::InvalidResponse);
        }
        let bytes = response.bytes().await.map_err(|_| RemoteError::Offline)?;
        if bytes.len() > MAX_GRAPHQL_RESPONSE_BYTES {
            return Err(RemoteError::InvalidResponse);
        }
        serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)
    }

    pub(crate) async fn health(&self) -> Result<(), RemoteError> {
        let response = self
            .execute(json!({"query": "query DesktopConnectionHealth { __typename }"}))
            .await?;
        if response.get("data").is_some() {
            Ok(())
        } else {
            Err(RemoteError::InvalidResponse)
        }
    }

    pub(crate) async fn revoke_self(&self) -> Result<(), RemoteError> {
        let response = self
            .execute(json!({
                "query": "mutation DesktopDisconnect($clientId: String!) { revokeClient(clientId: $clientId) { clientId } }",
                "variables": {"clientId": self.client_id()}
            }))
            .await?;
        if response
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err(RemoteError::InvalidResponse);
        }
        Ok(())
    }

    pub(crate) async fn subscribe<F>(&self, request: Value, mut emit: F)
    where
        F: FnMut(RemoteSubscriptionEvent) -> bool,
    {
        let mut reconnect_index = 0;
        let mut reported_offline = false;
        loop {
            match self.subscribe_once(&request, &mut emit).await {
                SubscriptionEnd::Complete => {
                    let _ = emit(RemoteSubscriptionEvent::Complete);
                    return;
                }
                SubscriptionEnd::Unauthorized => {
                    let _ = emit(RemoteSubscriptionEvent::Unauthorized);
                    return;
                }
                SubscriptionEnd::Transient => {
                    if !reported_offline {
                        reported_offline = true;
                        if !emit(RemoteSubscriptionEvent::Offline) {
                            return;
                        }
                    }
                    let seconds = RECONNECT_DELAYS[reconnect_index];
                    reconnect_index = (reconnect_index + 1).min(RECONNECT_DELAYS.len() - 1);
                    tokio::time::sleep(Duration::from_secs(seconds)).await;
                }
            }
        }
    }

    async fn subscribe_once<F>(&self, payload: &Value, emit: &mut F) -> SubscriptionEnd
    where
        F: FnMut(RemoteSubscriptionEvent) -> bool,
    {
        let mut request = match self.websocket_url().into_client_request() {
            Ok(request) => request,
            Err(_) => return SubscriptionEnd::Transient,
        };
        let authorization = match HeaderValue::from_str(&format!("Bearer {}", self.token)) {
            Ok(value) => value,
            Err(_) => return SubscriptionEnd::Unauthorized,
        };
        request
            .headers_mut()
            .insert(websocket_header::AUTHORIZATION, authorization);
        request.headers_mut().insert(
            websocket_header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("graphql-transport-ws"),
        );
        let (mut socket, _) =
            match tokio::time::timeout(WEBSOCKET_TIMEOUT, connect_async(request)).await {
                Ok(Ok(connection)) => connection,
                Ok(Err(WebSocketError::Http(response))) if response.status().as_u16() == 401 => {
                    return SubscriptionEnd::Unauthorized;
                }
                Ok(Err(_)) | Err(_) => return SubscriptionEnd::Transient,
            };
        if socket
            .send(Message::Text(
                json!({"type": "connection_init"}).to_string().into(),
            ))
            .await
            .is_err()
        {
            return SubscriptionEnd::Transient;
        }
        let acknowledged = tokio::time::timeout(WEBSOCKET_TIMEOUT, async {
            loop {
                match socket.next().await {
                    Some(Ok(Message::Text(text))) => match message_kind(&text).as_deref() {
                        Some("connection_ack") => break true,
                        Some("ping")
                            if socket
                                .send(Message::Text(json!({"type": "pong"}).to_string().into()))
                                .await
                                .is_err() =>
                        {
                            break false;
                        }
                        _ => {}
                    },
                    Some(Ok(Message::Ping(bytes))) => {
                        if socket.send(Message::Pong(bytes)).await.is_err() {
                            break false;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break false,
                }
            }
        })
        .await
        .unwrap_or(false);
        if !acknowledged {
            return SubscriptionEnd::Transient;
        }
        if socket
            .send(Message::Text(
                json!({"id": "1", "type": "subscribe", "payload": payload})
                    .to_string()
                    .into(),
            ))
            .await
            .is_err()
        {
            return SubscriptionEnd::Transient;
        }
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Text(text)) => match map_protocol_message(&text) {
                    ProtocolMessage::Next(value) => {
                        if !emit(RemoteSubscriptionEvent::Next(value)) {
                            return SubscriptionEnd::Complete;
                        }
                    }
                    ProtocolMessage::Complete => return SubscriptionEnd::Complete,
                    ProtocolMessage::Ping => {
                        if socket
                            .send(Message::Text(json!({"type": "pong"}).to_string().into()))
                            .await
                            .is_err()
                        {
                            return SubscriptionEnd::Transient;
                        }
                    }
                    ProtocolMessage::Ignore => {}
                },
                Ok(Message::Ping(bytes)) => {
                    if socket.send(Message::Pong(bytes)).await.is_err() {
                        return SubscriptionEnd::Transient;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => return SubscriptionEnd::Transient,
                Ok(_) => {}
            }
        }
        SubscriptionEnd::Transient
    }

    fn build_graphql_request(&self, body: Value) -> Result<reqwest::Request, RemoteError> {
        self.http
            .post(format!("{}/graphql", self.origin))
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .json(&body)
            .build()
            .map_err(|_| RemoteError::InvalidResponse)
    }

    fn websocket_url(&self) -> String {
        format!(
            "wss://{}/graphql/ws",
            self.origin.trim_start_matches("https://")
        )
    }
}

enum ProtocolMessage {
    Next(Value),
    Complete,
    Ping,
    Ignore,
}

fn message_kind(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value.get("type")?.as_str().map(str::to_owned)
}

fn map_protocol_message(text: &str) -> ProtocolMessage {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return ProtocolMessage::Ignore;
    };
    match value.get("type").and_then(Value::as_str) {
        Some("next") => value
            .get("payload")
            .cloned()
            .map_or(ProtocolMessage::Ignore, ProtocolMessage::Next),
        Some("error") => ProtocolMessage::Next(json!({
            "errors": value.get("payload").cloned().unwrap_or(Value::Null)
        })),
        Some("complete") => ProtocolMessage::Complete,
        Some("ping") => ProtocolMessage::Ping,
        _ => ProtocolMessage::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::*;
    use crate::desktop_profile::RemoteMetadata;

    fn remote() -> RemoteGraphql {
        let client_id = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        let token = format!("{client_id}.{}", URL_SAFE_NO_PAD.encode([2_u8; 32]));
        RemoteGraphql::new(RemoteProfile {
            metadata: RemoteMetadata {
                origin: "https://noema.example".to_string(),
                client_id,
            },
            token,
        })
        .expect("remote")
    }

    #[test]
    fn graphql_request_keeps_values_and_adds_bearer_authorization() {
        let remote = remote();
        let body = json!({"query": "query Example($value: String!) { echo(value: $value) }", "variables": {"value": "ordinary-value"}});
        let request = remote.build_graphql_request(body.clone()).expect("request");
        assert_eq!(request.url().as_str(), "https://noema.example/graphql");
        assert!(
            request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("Bearer "))
        );
        let saved: Value =
            serde_json::from_slice(request.body().expect("body").as_bytes().expect("bytes"))
                .expect("json body");
        assert_eq!(saved, body);
    }

    #[test]
    fn websocket_protocol_maps_next_error_complete_and_ping() {
        assert!(matches!(
            map_protocol_message(r#"{"type":"next","payload":{"data":{"ok":true}}}"#),
            ProtocolMessage::Next(value) if value["data"]["ok"] == true
        ));
        assert!(matches!(
            map_protocol_message(r#"{"type":"error","payload":[{"message":"denied"}]}"#),
            ProtocolMessage::Next(value) if value["errors"][0]["message"] == "denied"
        ));
        assert!(matches!(
            map_protocol_message(r#"{"type":"complete"}"#),
            ProtocolMessage::Complete
        ));
        assert!(matches!(
            map_protocol_message(r#"{"type":"ping"}"#),
            ProtocolMessage::Ping
        ));
    }
}
