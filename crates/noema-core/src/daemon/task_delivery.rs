//! Exactly-once structured task status delivery into source conversations.

use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem, NoemaStore,
    TaskStatus,
    graphql::{ConversationLiveEvent, ConversationSubscriptionRegistry},
};

use super::{TurnStreamEvent, TurnTranscriptItem};

/// Append and publish the latest actionable or terminal task status as a
/// structured task reference. Durable task-event identity makes repeated
/// delivery attempts idempotent without presenting the update as primary-agent
/// authored prose.
pub(crate) async fn deliver_task_status_event(
    store: &NoemaStore,
    subscriptions: &ConversationSubscriptionRegistry,
    task_id: &str,
) -> Result<(), String> {
    let task = store
        .get_task(task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before status delivery".to_string())?;
    if !matches!(
        task.status,
        TaskStatus::WaitingForHuman
            | TaskStatus::Completed
            | TaskStatus::Failed
            | TaskStatus::Cancelled
    ) {
        return Ok(());
    }
    let Some(conversation_id) = task.source.conversation_id.clone() else {
        return Ok(());
    };
    let event_kind = format!("task.{}", task.status.as_str());
    let event = store
        .list_task_events_after(&task.task_id, None, i64::MAX)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .rev()
        .find(|event| event.event_kind == event_kind)
        .ok_or_else(|| format!("task status event missing for {event_kind}"))?;
    let item_id = format!("item:task_status:{}", event.event_id);
    let (record, inserted) = store
        .append_conversation_item_with_id_if_absent(
            item_id,
            NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: task.source.turn_id.clone(),
                parent_item_id: task.source.item_id.clone(),
                kind: ConversationItemKind::TaskReference,
                status: ConversationItemStatus::Completed,
                author: ActorRef::system("system:task-runtime"),
                content_text: Some(task.title.clone()),
                payload_json: serde_json::json!({
                    "task_id": task.task_id,
                    "title": task.title,
                    "status": task.status.as_str(),
                    "revision": task.revision_index,
                    "task_event_id": event.event_id,
                    "blocking_question": task.blocked_question,
                    "final_submission_id": task.final_submission_id,
                    "error_code": task.error_code,
                    "error_message": task.error_message,
                }),
                metadata: serde_json::json!({
                    "source": "background_task_status",
                    "task_id": task.task_id,
                    "task_event_id": event.event_id,
                    "task_status": task.status.as_str(),
                }),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    if !inserted {
        return Ok(());
    }
    subscriptions.publish(ConversationLiveEvent::Turn {
        client_message_id: None,
        event: Box::new(TurnStreamEvent::ConversationItem {
            conversation_id: record.conversation_id.clone(),
            item_id: record.item_id,
            cursor: Some(record.cursor),
            turn_id: record.turn_id,
            metadata: record.metadata,
            item: Box::new(TurnTranscriptItem::TaskReference {
                task_id: task.task_id,
                title: task.title,
                status: task.status.as_str().to_string(),
                revision: task.revision_index,
            }),
        }),
    });
    subscriptions.publish(ConversationLiveEvent::Completed {
        conversation_id,
        client_message_id: None,
    });
    Ok(())
}
