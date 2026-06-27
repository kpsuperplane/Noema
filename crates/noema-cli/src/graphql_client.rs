//! Minimal GraphQL client used by the CLI while product APIs move off the
//! local daemon socket protocol.

use futures_util::{SinkExt, StreamExt};
use noema_core::{TurnActivityStatus, TurnTranscriptItem};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use url::Url;

static NEXT_CLIENT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

/// JSON body sent to the Noema GraphQL endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct GraphqlRequest {
    pub(crate) query: &'static str,
    pub(crate) variables: Value,
}

impl GraphqlRequest {
    /// Build a GraphQL request body.
    pub(crate) const fn new(query: &'static str, variables: Value) -> Self {
        Self { query, variables }
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

/// One CLI-ready event from a GraphQL conversation turn stream.
pub(crate) struct GraphqlTurnEvent {
    pub(crate) transcript_item: Option<TurnTranscriptItem>,
    pub(crate) completed: bool,
    pub(crate) terminal_error: Option<String>,
}

/// Execute a GraphQL request and return the decoded `data` payload.
pub(crate) async fn execute<T: DeserializeOwned>(
    base_url: &str,
    request: GraphqlRequest,
) -> Result<T, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/graphql"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let body = response
        .json::<GraphqlResponse<T>>()
        .await
        .map_err(|error| error.to_string())?;

    if let Some(error) = body.errors.and_then(|errors| errors.into_iter().next()) {
        return Err(error.message);
    }

    body.data
        .ok_or_else(|| "GraphQL response did not include data".to_string())
}

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

const START_PRIMARY_CONVERSATION_MUTATION: &str = r#"
mutation StartPrimaryConversation($model: String, $cwd: String) {
  startPrimaryConversation(model: $model, cwd: $cwd) {
    conversationId
  }
}
"#;

const LOCAL_STATUS_QUERY: &str = r#"
query CliLocalStatusProbe {
  localStatus {
    localService
  }
}
"#;

const SEND_CONVERSATION_TURN_MUTATION: &str = r#"
mutation SendConversationTurn($input: GraphqlSendConversationTurnInput!) {
  sendConversationTurn(input: $input) {
    conversationId
  }
}
"#;

#[derive(Debug, Deserialize)]
struct StartPrimaryConversationData {
    #[serde(rename = "startPrimaryConversation")]
    start_primary_conversation: GraphqlStartedConversation,
}

#[derive(Debug, Deserialize)]
struct GraphqlStartedConversation {
    #[serde(rename = "conversationId")]
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
struct SendConversationTurnData {
    #[serde(rename = "sendConversationTurn")]
    send_conversation_turn: GraphqlAcceptedTurn,
}

#[derive(Debug, Deserialize)]
struct GraphqlAcceptedTurn {
    #[serde(rename = "conversationId")]
    conversation_id: String,
}

/// Verify that the resolved GraphQL HTTP endpoint is reachable.
pub(crate) async fn validate_graphql_base_url(base_url: &str) -> Result<(), String> {
    execute::<serde_json::Value>(
        base_url,
        GraphqlRequest::new(LOCAL_STATUS_QUERY, serde_json::json!({})),
    )
    .await
    .map_err(|error| {
        format!(
            "could not reach Noema GraphQL at {base_url}/graphql using the resolved daemon web URL. If the daemon is already running, restart it with the same config or run `noema start` with the intended web settings. Details: {error}"
        )
    })?;

    Ok(())
}

/// Start or resume the primary conversation through GraphQL.
pub(crate) async fn start_primary_conversation(
    base_url: &str,
    model: Option<String>,
    cwd: Option<String>,
) -> Result<String, String> {
    let data = execute::<StartPrimaryConversationData>(
        base_url,
        GraphqlRequest::new(
            START_PRIMARY_CONVERSATION_MUTATION,
            json!({
                "model": model,
                "cwd": cwd,
            }),
        ),
    )
    .await?;

    Ok(data.start_primary_conversation.conversation_id)
}

/// Subscribe, send one turn, and return CLI-ready turn events.
pub(crate) async fn stream_conversation_turn(
    base_url: &str,
    conversation_id: &str,
    prompt: String,
) -> Result<mpsc::UnboundedReceiver<Result<GraphqlTurnEvent, String>>, String> {
    let client_message_id = next_client_message_id();
    let mut graphql_events =
        subscribe_conversation_events(base_url, conversation_id, &client_message_id).await?;
    send_conversation_turn(base_url, conversation_id, prompt, &client_message_id).await?;

    let conversation_id = conversation_id.to_string();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(event) = graphql_events.recv().await {
            match graphql_turn_event(event, &conversation_id, &client_message_id) {
                Ok(event) => {
                    let completed = event.completed;
                    let _ = tx.send(Ok(event));
                    if completed {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error));
                    break;
                }
            }
        }
    });

    Ok(rx)
}

async fn send_conversation_turn(
    base_url: &str,
    conversation_id: &str,
    prompt: String,
    client_message_id: &str,
) -> Result<(), String> {
    let data = execute::<SendConversationTurnData>(
        base_url,
        GraphqlRequest::new(
            SEND_CONVERSATION_TURN_MUTATION,
            json!({
                "input": {
                    "conversationId": conversation_id,
                    "input": prompt,
                    "clientMessageId": client_message_id,
                },
            }),
        ),
    )
    .await?;

    if data.send_conversation_turn.conversation_id != conversation_id {
        return Err(format!(
            "GraphQL accepted turn for unexpected conversation {}",
            data.send_conversation_turn.conversation_id
        ));
    }

    Ok(())
}

/// Subscribe to GraphQL conversation event data payloads.
async fn subscribe_conversation_events(
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

fn next_client_message_id() -> String {
    let id = NEXT_CLIENT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
    format!("cli_turn_{id}")
}

fn graphql_turn_event(
    data: Result<Value, String>,
    conversation_id: &str,
    client_message_id: &str,
) -> Result<GraphqlTurnEvent, String> {
    let data = data?;
    let event = data
        .get("conversationEvents")
        .ok_or_else(|| "GraphQL event payload missing conversationEvents".to_string())?;
    if !graphql_event_matches_turn(event, conversation_id, client_message_id) {
        return Ok(GraphqlTurnEvent {
            transcript_item: None,
            completed: false,
            terminal_error: None,
        });
    }
    let transcript_item = transcript_item_from_graphql_event(event)?;

    Ok(GraphqlTurnEvent {
        terminal_error: nonrecoverable_error_message(&transcript_item),
        transcript_item,
        completed: is_graphql_turn_completed_event(event, conversation_id, client_message_id),
    })
}

fn graphql_data_is_turn_completed(
    data: &Value,
    conversation_id: &str,
    client_message_id: &str,
) -> bool {
    data.get("conversationEvents").is_some_and(|event| {
        is_graphql_turn_completed_event(event, conversation_id, client_message_id)
    })
}

fn graphql_data_is_subscription_ready(data: &Value, conversation_id: &str) -> bool {
    data.get("conversationEvents").is_some_and(|event| {
        event.get("__typename").and_then(Value::as_str) == Some("GraphqlSubscriptionReadyEvent")
            && event.get("conversationId").and_then(Value::as_str) == Some(conversation_id)
    })
}

fn graphql_event_matches_turn(
    event: &Value,
    conversation_id: &str,
    client_message_id: &str,
) -> bool {
    match event.get("__typename").and_then(Value::as_str) {
        Some("GraphqlConversationItemEvent" | "GraphqlTurnCompletedEvent") => {
            event.get("conversationId").and_then(Value::as_str) == Some(conversation_id)
                && event.get("clientMessageId").and_then(Value::as_str) == Some(client_message_id)
        }
        _ => false,
    }
}

fn transcript_item_from_graphql_event(event: &Value) -> Result<Option<TurnTranscriptItem>, String> {
    if event.get("__typename").and_then(Value::as_str) != Some("GraphqlConversationItemEvent") {
        return Ok(None);
    }

    let item = event
        .get("item")
        .ok_or_else(|| "GraphQL conversation item event missing item".to_string())?;
    match required_str(item, "__typename")? {
        "GraphqlUserText" => Ok(Some(TurnTranscriptItem::UserText {
            text: required_str(item, "text")?.to_string(),
        })),
        "GraphqlAssistantText" => Ok(Some(TurnTranscriptItem::AssistantText {
            text: required_str(item, "text")?.to_string(),
        })),
        "GraphqlActivity" => Ok(Some(TurnTranscriptItem::Activity {
            id: required_str(item, "id")?.to_string(),
            activity_kind: required_str(item, "activityKind")?.to_string(),
            status: graphql_activity_status(required_str(item, "status")?)?,
            title: required_str(item, "title")?.to_string(),
            summary: item
                .get("summary")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            metadata: item.get("metadata").cloned().unwrap_or(Value::Null),
        })),
        "GraphqlA2UiCard" => Ok(Some(TurnTranscriptItem::A2uiCard {
            id: required_str(item, "id")?.to_string(),
            schema: required_str(item, "schema")?.to_string(),
            payload: item.get("payload").cloned().unwrap_or(Value::Null),
        })),
        "GraphqlErrorNotice" => Ok(Some(TurnTranscriptItem::ErrorNotice {
            message: required_str(item, "message")?.to_string(),
            recoverable: item
                .get("recoverable")
                .and_then(Value::as_bool)
                .ok_or_else(|| "GraphQL error notice missing recoverable".to_string())?,
        })),
        typename => Err(format!(
            "unsupported GraphQL transcript item type: {typename}"
        )),
    }
}

fn nonrecoverable_error_message(item: &Option<TurnTranscriptItem>) -> Option<String> {
    match item {
        Some(TurnTranscriptItem::ErrorNotice {
            message,
            recoverable: false,
        }) => Some(message.clone()),
        _ => None,
    }
}

fn graphql_activity_status(value: &str) -> Result<TurnActivityStatus, String> {
    match value {
        "STARTED" => Ok(TurnActivityStatus::Started),
        "COMPLETED" => Ok(TurnActivityStatus::Completed),
        "FAILED" => Ok(TurnActivityStatus::Failed),
        other => Err(format!("unsupported GraphQL activity status: {other}")),
    }
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("GraphQL payload missing string field {field}"))
}

fn is_graphql_turn_completed_event(
    event: &Value,
    conversation_id: &str,
    client_message_id: &str,
) -> bool {
    event.get("__typename").and_then(Value::as_str) == Some("GraphqlTurnCompletedEvent")
        && event.get("conversationId").and_then(Value::as_str) == Some(conversation_id)
        && event.get("clientMessageId").and_then(Value::as_str) == Some(client_message_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_body_includes_query_and_variables() {
        let body = GraphqlRequest::new(
            "query Ping { localStatus { localService } }",
            serde_json::json!({}),
        );

        assert!(body.query.contains("localStatus"));
        assert_eq!(body.variables, serde_json::json!({}));
    }

    #[test]
    fn graphql_ws_url_uses_subscription_endpoint() {
        let url = graphql_ws_url("http://127.0.0.1:3737").expect("url");

        assert_eq!(url, "ws://127.0.0.1:3737/graphql/ws");
    }

    #[test]
    fn graphql_client_converts_assistant_event_to_transcript_item() {
        let event = serde_json::json!({
            "__typename": "GraphqlConversationItemEvent",
            "conversationId": "conversation_1",
            "clientMessageId": "client_1",
            "item": {
                "__typename": "GraphqlAssistantText",
                "text": "hello"
            }
        });

        let item = transcript_item_from_graphql_event(&event).expect("item");

        assert_eq!(
            item,
            Some(TurnTranscriptItem::AssistantText {
                text: "hello".to_string()
            })
        );
    }

    #[test]
    fn graphql_client_matches_turn_completed_for_client_message() {
        let event = serde_json::json!({
            "__typename": "GraphqlTurnCompletedEvent",
            "conversationId": "conversation_1",
            "clientMessageId": "client_1"
        });

        assert!(is_graphql_turn_completed_event(
            &event,
            "conversation_1",
            "client_1"
        ));
        assert!(!is_graphql_turn_completed_event(
            &event,
            "conversation_1",
            "client_2"
        ));
    }

    #[test]
    fn graphql_client_ignores_item_for_mismatched_client_message() {
        let data = Ok(serde_json::json!({
            "conversationEvents": {
                "__typename": "GraphqlConversationItemEvent",
                "conversationId": "conversation_1",
                "clientMessageId": "other_client",
                "item": {
                    "__typename": "GraphqlAssistantText",
                    "text": "not for this turn"
                }
            }
        }));

        let event = graphql_turn_event(data, "conversation_1", "client_1").expect("event");

        assert!(event.transcript_item.is_none());
        assert!(!event.completed);
    }

    #[test]
    fn graphql_client_ignores_item_for_mismatched_conversation() {
        let data = Ok(serde_json::json!({
            "conversationEvents": {
                "__typename": "GraphqlConversationItemEvent",
                "conversationId": "other_conversation",
                "clientMessageId": "client_1",
                "item": {
                    "__typename": "GraphqlAssistantText",
                    "text": "not for this conversation"
                }
            }
        }));

        let event = graphql_turn_event(data, "conversation_1", "client_1").expect("event");

        assert!(event.transcript_item.is_none());
        assert!(!event.completed);
    }

    #[test]
    fn graphql_client_marks_nonrecoverable_error_notice_as_terminal_error() {
        let data = Ok(serde_json::json!({
            "conversationEvents": {
                "__typename": "GraphqlConversationItemEvent",
                "conversationId": "conversation_1",
                "clientMessageId": "client_1",
                "item": {
                    "__typename": "GraphqlErrorNotice",
                    "message": "provider failed",
                    "recoverable": false
                }
            }
        }));

        let event = graphql_turn_event(data, "conversation_1", "client_1").expect("event");

        assert_eq!(event.terminal_error.as_deref(), Some("provider failed"));
    }
}
