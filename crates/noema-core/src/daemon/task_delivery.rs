//! Exactly-once structured task status delivery into source conversations.

use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem, NoemaStore,
    TaskStatus,
    daemon::{
        CodexRuntimeHandle,
        runtime::{TaskCompletionArtifact, TaskCompletionCriterion, TaskCompletionDeliveryRequest},
    },
    graphql::{ConversationLiveEvent, ConversationSubscriptionRegistry},
};
use noema_home::{SystemErrorEvent, SystemErrorLogger};

use super::{TurnStreamEvent, TurnTranscriptItem};

/// Drain terminal task outcomes into primary-agent assistant turns.
pub(crate) async fn drain_task_completion_outbox(
    store: &NoemaStore,
    runtime: &CodexRuntimeHandle,
    system_errors: &SystemErrorLogger,
) {
    let completion_deliveries = match store.list_pending_task_completion_deliveries(32).await {
        Ok(deliveries) => deliveries,
        Err(error) => {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "task_completion_outbox_read_failed",
                    "Task completion delivery queue could not be read",
                )
                .with_error_chain([error.to_string()]),
            );
            return;
        }
    };
    for (task_id, delivery_id) in completion_deliveries {
        let Some(request) = (match build_task_completion_request(store, &task_id, delivery_id).await
        {
            Ok(request) => request,
            Err(error) => {
                system_errors.try_append(
                    SystemErrorEvent::new(
                        "task_completion_context_failed",
                        "Task completion context could not be assembled",
                    )
                    .with_context(serde_json::json!({ "task_id": task_id }))
                    .with_error_chain([error]),
                );
                continue;
            }
        }) else {
            continue;
        };
        if let Err(error) = runtime.deliver_task_completion(request).await {
            system_errors.try_append(
                SystemErrorEvent::new(
                    "task_completion_delivery_failed",
                    "Primary-agent task completion report could not be delivered",
                )
                .with_context(serde_json::json!({ "task_id": task_id }))
                .with_error_chain([error.to_string()]),
            );
        }
    }
}

async fn build_task_completion_request(
    store: &NoemaStore,
    task_id: &str,
    delivery_id: String,
) -> Result<Option<TaskCompletionDeliveryRequest>, String> {
    let Some(task) = store
        .get_task(task_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let Some(conversation_id) = task.source.conversation_id.clone() else {
        return Ok(None);
    };
    if !matches!(
        task.status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    ) {
        return Ok(None);
    }
    let submission = match task.final_submission_id.as_deref() {
        Some(submission_id) => store
            .get_task_submission(submission_id)
            .await
            .map_err(|error| error.to_string())?,
        None => None,
    };
    let latest_review = store
        .list_task_reviews(&task.task_id)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .last();
    let criteria = latest_review
        .as_ref()
        .map(|review| {
            review
                .criteria
                .iter()
                .map(|criterion| TaskCompletionCriterion {
                    criterion_id: criterion.criterion_id.clone(),
                    outcome: Some(criterion.outcome.as_str().to_string()),
                    evidence: criterion.evidence_markdown.clone(),
                    feedback: criterion.feedback.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let detail = task
        .blocked_question
        .clone()
        .or_else(|| task.error_message.clone())
        .or_else(|| {
            latest_review
                .as_ref()
                .map(|review| review.overall_feedback.clone())
        });
    let artifacts = submission
        .as_ref()
        .map(|submission| {
            submission
                .artifacts
                .iter()
                .map(|linked| {
                    let (external_url, download_url) = match &linked.version.storage {
                        crate::ArtifactVersionStorage::LocalFile { .. } => (
                            None,
                            Some(crate::artifact_download_url(
                                &linked.version.artifact_version_id,
                            )),
                        ),
                        crate::ArtifactVersionStorage::ExternalUrl { url } => {
                            (Some(url.clone()), None)
                        }
                    };
                    TaskCompletionArtifact {
                        artifact_id: linked.artifact.artifact_id.clone(),
                        artifact_version_id: linked.version.artifact_version_id.clone(),
                        title: linked.artifact.title.clone(),
                        artifact_kind: linked.artifact.artifact_kind.clone(),
                        storage_kind: linked.artifact.storage_kind.as_str().to_string(),
                        external_url,
                        download_url,
                        media_type: linked.version.media_type.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(TaskCompletionDeliveryRequest {
        delivery_id,
        task_id: task.task_id,
        conversation_id,
        source_item_id: task.source.item_id,
        title: task.title,
        status: task.status.as_str().to_string(),
        request_markdown: task.request_markdown,
        summary: submission
            .as_ref()
            .map(|submission| submission.summary.clone()),
        result_markdown: submission
            .as_ref()
            .map(|submission| submission.result_markdown.clone()),
        artifacts,
        review_feedback: latest_review.map(|review| review.overall_feedback),
        criteria,
        detail,
    }))
}

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
