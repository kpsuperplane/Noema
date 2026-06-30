//! GraphQL client operations used by the CLI.

pub(crate) mod http;
pub(crate) mod transcript;
pub(crate) mod ws;

use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use transcript::graphql_turn_event;
use ws::subscribe_conversation_events;

pub(crate) use http::{GraphqlRequest, execute};
pub(crate) use transcript::GraphqlTurnEvent;

static NEXT_CLIENT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

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

fn next_client_message_id() -> String {
    let id = NEXT_CLIENT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
    format!("cli_turn_{id}")
}
