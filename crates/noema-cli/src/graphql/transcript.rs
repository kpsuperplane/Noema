//! Decode GraphQL conversation events into CLI transcript events.

use noema_core::{TurnActivityStatus, TurnTranscriptItem};
use serde_json::Value;

/// One CLI-ready event from a GraphQL conversation turn stream.
pub(crate) struct GraphqlTurnEvent {
    pub(crate) transcript_item: Option<TurnTranscriptItem>,
    pub(crate) completed: bool,
    pub(crate) terminal_error: Option<String>,
}

pub(crate) fn graphql_turn_event(
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
        terminal_error: nonrecoverable_error_message(transcript_item.as_ref()),
        transcript_item,
        completed: is_graphql_turn_completed_event(event, conversation_id, client_message_id),
    })
}

pub(crate) fn graphql_data_is_turn_completed(
    data: &Value,
    conversation_id: &str,
    client_message_id: &str,
) -> bool {
    data.get("conversationEvents").is_some_and(|event| {
        is_graphql_turn_completed_event(event, conversation_id, client_message_id)
    })
}

pub(crate) fn graphql_data_is_subscription_ready(data: &Value, conversation_id: &str) -> bool {
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

fn nonrecoverable_error_message(item: Option<&TurnTranscriptItem>) -> Option<String> {
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
