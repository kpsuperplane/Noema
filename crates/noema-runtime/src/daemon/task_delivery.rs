//! Durable Work notification delivery for the supervised task runtime.

use noema_store::NoemaStore;
use serde_json::json;

use super::{LEASE_SECONDS, PERSONAL_WORKSPACE_ID, TaskRuntimeServices};
use crate::daemon::{WorkRuntimeEvent, log_system_error};

pub(super) async fn drain_work_notifications(services: &TaskRuntimeServices) {
    let request = noema_store::WorkNotificationLeaseRequest {
        worker_id: format!("work-notification:{}", std::process::id()),
        lease_seconds: LEASE_SECONDS,
        limit: 32,
    };
    let claimed = match services.store.claim_work_notifications(request).await {
        Ok(claimed) => claimed,
        Err(error) => {
            log_system_error(
                &services.system_errors,
                "work_notification_claim_failed",
                "Work notification queue could not be claimed",
                None,
                error,
            );
            return;
        }
    };
    for notification in claimed {
        let work_event = match notification_work_event(&notification) {
            Ok(event) => event,
            Err(error) => {
                log_system_error(
                    &services.system_errors,
                    "work_notification_scope_invalid",
                    "Work notification scope could not be resolved",
                    Some(json!({"notification_id": notification.notification_id.clone()})),
                    error,
                );
                fail_notification(
                    services,
                    notification.notification_id,
                    notification.lease_token,
                    "notification_scope_invalid",
                    "notification task scope is invalid",
                    None,
                )
                .await;
                continue;
            }
        };
        let conversation_id =
            match primary_conversation_id(&services.store, &notification.destination_id).await {
                Ok(Some(conversation_id)) => conversation_id,
                Ok(None) => {
                    fail_notification(
                        services,
                        notification.notification_id,
                        notification.lease_token,
                        "conversation_unavailable",
                        "primary conversation is unavailable",
                        Some(work_event.clone()),
                    )
                    .await;
                    continue;
                }
                Err(error) => {
                    log_system_error(
                        &services.system_errors,
                        "work_notification_conversation_lookup_failed",
                        "Work notification destination could not be resolved",
                        Some(json!({"notification_id": notification.notification_id.clone()})),
                        error,
                    );
                    fail_notification(
                        services,
                        notification.notification_id,
                        notification.lease_token,
                        "conversation_lookup_failed",
                        "primary conversation lookup failed",
                        Some(work_event.clone()),
                    )
                    .await;
                    continue;
                }
            };
        let notification_id = notification.notification_id;
        let lease_token = notification.lease_token;
        if let Err(error) = services
            .runtime
            .deliver_work_notification(
                noema_store::CompleteWorkNotification {
                    notification_id: notification_id.clone(),
                    lease_token: lease_token.clone(),
                    conversation_id,
                },
                work_event.clone(),
            )
            .await
        {
            log_system_error(
                &services.system_errors,
                "work_notification_delivery_failed",
                "Work notification could not be delivered",
                Some(json!({"notification_id": notification_id.clone()})),
                error,
            );
            fail_notification(
                services,
                notification_id,
                lease_token,
                "delivery_failed",
                "runtime notification delivery failed",
                Some(work_event),
            )
            .await;
        }
    }
}

pub(super) async fn fail_notification(
    services: &TaskRuntimeServices,
    notification_id: String,
    lease_token: String,
    error_code: &str,
    error_message: &str,
    work_event: Option<WorkRuntimeEvent>,
) {
    match services
        .store
        .fail_work_notification(noema_store::FailWorkNotification {
            notification_id: notification_id.clone(),
            lease_token,
            error_code: error_code.to_string(),
            error_message: Some(error_message.to_string()),
            retryable: true,
        })
        .await
    {
        Ok(()) => {
            if let Some(event) = work_event {
                services.subscriptions.publish_work(event);
            }
        }
        Err(error) => log_system_error(
            &services.system_errors,
            "work_notification_failure_disposition_failed",
            "Work notification failure could not be committed",
            Some(json!({"notification_id": notification_id})),
            error,
        ),
    }
}

pub(super) fn notification_work_event(
    notification: &noema_store::ClaimedWorkNotification,
) -> Result<WorkRuntimeEvent, String> {
    let task_id = notification
        .payload
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "notification payload has no task identity".to_string())?;
    let task_id =
        noema_tasks::TaskId::new(task_id.to_string()).map_err(|error| error.to_string())?;
    Ok(WorkRuntimeEvent::Committed {
        workspace_id: PERSONAL_WORKSPACE_ID.to_string(),
        task_id: Some(task_id.to_string()),
    })
}

async fn primary_conversation_id(
    store: &NoemaStore,
    human_id: &str,
) -> Result<Option<String>, noema_store::StoreError> {
    store
        .primary_conversation_for_human(human_id)
        .await
        .map(|conversation| conversation.map(|conversation| conversation.conversation_id))
}
