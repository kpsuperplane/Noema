//! GraphQL WebSocket transport for CLI subscriptions.

use crate::graphql::{
    http::GraphqlRequest,
    transcript::{graphql_data_is_subscription_ready, graphql_data_is_turn_completed},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use url::Url;

const CONVERSATION_EVENTS_SUBSCRIPTION: &str = r#"
subscription ConversationEvents($conversationId: String!) {
  conversationEvents(conversationId: $conversationId) {
    __typename
    ... on GraphqlConversationItemEvent {
      conversationId
      clientMessageId
      itemId
      turnId
      item {
        __typename
        ... on GraphqlUserText {
          text
        }
        ... on GraphqlAssistantText {
          text
        }
        ... on GraphqlActivity {
          id
          activityKind
          status
          title
          summary
          metadata
        }
        ... on GraphqlA2UiCard {
          id
          schema
          payload
        }
        ... on GraphqlErrorNotice {
          message
          recoverable
        }
      }
    }
    ... on GraphqlAgentStatusEvent {
      conversationId
      status
    }
    ... on GraphqlTurnCompletedEvent {
      conversationId
      clientMessageId
    }
    ... on GraphqlSubscriptionReadyEvent {
      conversationId
    }
  }
}
"#;

/// Subscribe to GraphQL conversation event data payloads.
pub(crate) async fn subscribe_conversation_events(
    base_url: &str,
    conversation_id: &str,
    client_message_id: &str,
) -> Result<mpsc::UnboundedReceiver<Result<Value, String>>, String> {
    let ws_url = graphql_ws_url(base_url)?;
    let mut request = ws_url
        .into_client_request()
        .map_err(|error| error.to_string())?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("graphql-transport-ws"),
    );

    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| error.to_string())?;
    let (mut write, mut read) = socket.split();
    write_ws_json(&mut write, json!({ "type": "connection_init" })).await?;

    loop {
        let message = read
            .next()
            .await
            .ok_or_else(|| "GraphQL subscription closed before connection ack".to_string())?
            .map_err(|error| error.to_string())?;
        let Some(value) = websocket_json(message)? else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("connection_ack") => break,
            Some("ping") => write_ws_json(&mut write, json!({ "type": "pong" })).await?,
            Some("error") => return Err(graphql_ws_error_message(&value)),
            Some("complete") => {
                return Err("GraphQL subscription completed before connection ack".to_string());
            }
            _ => {}
        }
    }

    let id = "cli-conversation-events";
    write_ws_json(
        &mut write,
        json!({
            "id": id,
            "type": "subscribe",
            "payload": GraphqlRequest::new(
                CONVERSATION_EVENTS_SUBSCRIPTION,
                json!({ "conversationId": conversation_id }),
            ),
        }),
    )
    .await?;

    wait_for_subscription_ready(&mut read, &mut write, id, conversation_id).await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let conversation_id = conversation_id.to_string();
    let client_message_id = client_message_id.to_string();
    tokio::spawn(async move {
        while let Some(message) = read.next().await {
            match message
                .map_err(|error| error.to_string())
                .and_then(websocket_json)
            {
                Ok(Some(value)) => match value.get("type").and_then(Value::as_str) {
                    Some("next") => {
                        if let Some(errors) = value.pointer("/payload/errors") {
                            let _ = tx.send(Err(errors.to_string()));
                            break;
                        }
                        if let Some(data) = value.pointer("/payload/data") {
                            let data = data.clone();
                            let is_completed = graphql_data_is_turn_completed(
                                &data,
                                &conversation_id,
                                &client_message_id,
                            );
                            if tx.send(Ok(data)).is_err() {
                                break;
                            }
                            if is_completed {
                                break;
                            }
                        }
                    }
                    Some("ping") => {
                        if let Err(error) =
                            write_ws_json(&mut write, json!({ "type": "pong" })).await
                        {
                            let _ = tx.send(Err(error));
                            break;
                        }
                    }
                    Some("error") => {
                        let _ = tx.send(Err(graphql_ws_error_message(&value)));
                        break;
                    }
                    Some("complete") => break,
                    _ => {}
                },
                Ok(None) => {}
                Err(error) => {
                    let _ = tx.send(Err(error));
                    break;
                }
            }
        }
    });

    Ok(rx)
}

async fn wait_for_subscription_ready<R, W>(
    read: &mut R,
    write: &mut W,
    id: &str,
    conversation_id: &str,
) -> Result<(), String>
where
    R: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    W: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    loop {
        let message = read
            .next()
            .await
            .ok_or_else(|| "GraphQL subscription closed before readiness event".to_string())?
            .map_err(|error| error.to_string())?;
        let Some(value) = websocket_json(message)? else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("next") if value.get("id").and_then(Value::as_str) == Some(id) => {
                if let Some(errors) = value.pointer("/payload/errors") {
                    return Err(errors.to_string());
                }
                let Some(data) = value.pointer("/payload/data") else {
                    continue;
                };
                if graphql_data_is_subscription_ready(data, conversation_id) {
                    return Ok(());
                }
            }
            Some("ping") => write_ws_json(write, json!({ "type": "pong" })).await?,
            Some("error") => return Err(graphql_ws_error_message(&value)),
            Some("complete") if value.get("id").and_then(Value::as_str) == Some(id) => {
                return Err("GraphQL subscription completed before readiness event".to_string());
            }
            _ => {}
        }
    }
}

fn graphql_ws_url(base_url: &str) -> Result<String, String> {
    let mut url = Url::parse(base_url).map_err(|error| error.to_string())?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => return Err(format!("unsupported GraphQL base URL scheme: {other}")),
    };
    url.set_scheme(scheme)
        .map_err(|_| format!("could not convert {base_url} to a WebSocket URL"))?;
    url.set_path("/graphql/ws");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

async fn write_ws_json<S>(write: &mut S, value: Value) -> Result<(), String>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    write
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(|error| error.to_string())
}

fn websocket_json(message: Message) -> Result<Option<Value>, String> {
    match message {
        Message::Text(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| error.to_string()),
        Message::Binary(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| error.to_string()),
        Message::Close(_) => Ok(None),
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Ok(None),
    }
}

fn graphql_ws_error_message(value: &Value) -> String {
    value
        .get("payload")
        .map_or_else(|| value.to_string(), Value::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_ws_url_uses_subscription_endpoint() {
        let url = graphql_ws_url("http://127.0.0.1:3737").expect("url");

        assert_eq!(url, "ws://127.0.0.1:3737/graphql/ws");
    }
}
