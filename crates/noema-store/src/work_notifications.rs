//! Notification outbox lease and deterministic delivery contracts.
//!
//! The outbox is durable work state, not a best-effort broadcast.  A delivery
//! worker leases rows, lets the primary agent narrate notifications that need a
//! message, and then atomically inserts any deterministic task reference and
//! acknowledges the lease. Retrying after a crash reuses the deterministic
//! narration turn and item identity instead of inserting duplicate history.

use std::str::FromStr;

use noema_conversations::{ConversationItemKind, ConversationItemRecord, ConversationItemStatus};
use noema_tasks::{
    NotificationDestination, NotificationKind, TaskId, TaskSourceKind, WorkDomainError,
    WorkEventPayload,
};
use ring::digest::{SHA256, digest};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{NoemaStore, conversations::load_conversation_item_tx, ids::allocate_id};
use crate::{
    StoreError,
    work_events::{
        WORK_EVENT_COLUMNS, WorkEventScope, append_work_event_tx, decode_work_event_record,
    },
    work_reads::rows::load_task,
};

const EXPIRED_LEASE_MAINTENANCE_BATCH: i64 = 100;

struct LeasedNotification {
    event_sequence: i64,
    destination_kind: String,
    destination_id: String,
    kind: String,
    payload_json: String,
    attempt_count: i64,
}

/// Enqueue one owner-directed card in the same transaction as its source
/// event.  The unique outbox key makes retries and command replay harmless.
pub(crate) fn enqueue_work_notification_tx(
    transaction: &Transaction<'_>,
    event: &noema_tasks::WorkEventRecord,
    kind: NotificationKind,
    payload: &serde_json::Value,
) -> Result<Option<noema_tasks::WorkEventRecord>, StoreError> {
    let owners = {
        let mut statement = transaction.prepare(
            "SELECT human_id FROM workspace_memberships WHERE workspace_id = ?1 AND role = 'owner' ORDER BY human_id",
        )?;
        statement
            .query_map([event.workspace_id().as_str()], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let [human_id] = owners.as_slice() else {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    };
    let notification_id = allocate_id("notification");
    let payload_json = serde_json::to_string(payload)?;
    let inserted = transaction.execute(
        "INSERT INTO work_notification_outbox (notification_id, event_sequence, destination_kind, destination_id, notification_kind, payload_json) VALUES (?1, ?2, 'human_primary_conversation', ?3, ?4, ?5) ON CONFLICT(event_sequence, destination_kind, destination_id, notification_kind) DO NOTHING",
        params![notification_id, event.event_sequence(), human_id, kind.as_str(), payload_json],
    )?;
    if inserted == 0 {
        let existing = transaction
            .query_row(
                "SELECT destination_kind, destination_id, notification_kind, payload_json
                 FROM work_notification_outbox
                 WHERE event_sequence = ?1 AND destination_kind = 'human_primary_conversation'
                   AND destination_id = ?2 AND notification_kind = ?3",
                params![event.event_sequence(), human_id, kind.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "notification conflict row disappeared".to_string(),
            })?;
        let existing_payload: serde_json::Value = serde_json::from_str(&existing.3)?;
        if existing.0 != NotificationDestination::HumanPrimaryConversation.as_str()
            || existing.1 != *human_id
            || existing.2 != kind.as_str()
            || existing_payload.ne(payload)
        {
            return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
        }
        return Ok(None);
    }
    let notification_event = WorkEventPayload::notification_queued(
        notification_id,
        event.event_sequence(),
        kind,
        NotificationDestination::HumanPrimaryConversation,
    )
    .map_err(StoreError::Work)?;
    Ok(Some(append_work_event_tx(
        transaction,
        WorkEventScope {
            workspace_id: event.workspace_id().clone(),
            project_id: event.project_id().cloned(),
            task_id: event.task_id().cloned(),
            run_id: event.run_id().map(str::to_string),
            actor_id: "actor:store:notification".to_string(),
            causation_id: Some(event.event_id().to_string()),
            correlation_id: event.correlation_id().to_string(),
        },
        notification_event,
    )?))
}

impl NoemaStore {
    /// Claim pending/failed cards in stable availability order.
    ///
    /// # Errors
    ///
    /// Returns an error when the lease request is invalid or the outbox cannot
    /// be repaired and leased atomically.
    pub async fn claim_work_notifications(
        &self,
        request: WorkNotificationLeaseRequest,
    ) -> Result<Vec<ClaimedWorkNotification>, StoreError> {
        request.validate().map_err(StoreError::Work)?;
        let lease_token = allocate_id("notification_lease");
        self.with_immediate_transaction_retry(|transaction| {
            fail_expired_notification_leases_tx(transaction)?;
            park_exhausted_notifications_tx(transaction)?;
            let ids = {
                let mut statement = transaction.prepare(
                    "SELECT notification_id FROM work_notification_outbox WHERE status IN ('pending', 'failed') AND attempt_count < 4294967295 AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ORDER BY available_at, notification_id LIMIT ?1",
                )?;
                statement
                    .query_map([i64::from(request.limit)], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for notification_id in &ids {
                transaction.execute(
                    "UPDATE work_notification_outbox SET status = 'leased', lease_owner = ?2, lease_token = ?3, lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?4 || ' seconds'), attempt_count = attempt_count + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE notification_id = ?1 AND status IN ('pending', 'failed')",
                    params![notification_id, request.worker_id, lease_token, request.lease_seconds],
                )?;
            }
            ids.into_iter()
                .map(|notification_id| load_claimed_notification_tx(transaction, &notification_id))
                .collect()
        })
        .await
    }

    /// Acknowledge a delivered notification and insert its task reference when
    /// the transcript still needs one.
    ///
    /// # Errors
    ///
    /// Returns an error when the completion identity is invalid, the lease is
    /// stale, or deterministic delivery persistence fails.
    pub async fn complete_work_notification(
        &self,
        completion: CompleteWorkNotification,
    ) -> Result<Option<ConversationItemRecord>, StoreError> {
        completion.validate().map_err(StoreError::Work)?;
        self.with_immediate_transaction_retry(|transaction| {
            let row = load_leased_notification_tx(
                transaction,
                &completion.notification_id,
                &completion.lease_token,
            )?;
            let destination_kind = NotificationDestination::from_str(&row.destination_kind)
                .map_err(StoreError::Work)?;
            let notification_kind =
                NotificationKind::from_str(&row.kind).map_err(StoreError::Work)?;
            validate_notification_destination_tx(
                transaction,
                &row,
                &completion.conversation_id,
            )?;
            let item = if should_suppress_near_term_task_reference_tx(
                transaction,
                &row,
                notification_kind,
                &completion.notification_id,
                &completion.conversation_id,
            )? {
                None
            } else {
                Some(insert_notification_item_tx(
                    transaction,
                    &row,
                    destination_kind,
                    notification_kind,
                    &completion.notification_id,
                    &completion.conversation_id,
                )?)
            };
            let changed = transaction.execute(
                "UPDATE work_notification_outbox SET status = 'delivered', delivered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE notification_id = ?1 AND status = 'leased' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![completion.notification_id, completion.lease_token],
            )?;
            if changed != 1 {
                return Err(StoreError::Work(WorkDomainError::RunFenced));
            }
            let source = load_source_event_tx(transaction, row.event_sequence)?;
            let payload = WorkEventPayload::notification_delivered(
                completion.notification_id.clone(),
                source.event_sequence(),
                notification_kind,
                notification_attempt_count(row.attempt_count)?,
            )
            .map_err(StoreError::Work)?;
            append_notification_event_tx(
                transaction,
                source,
                payload,
            )?;
            Ok(item)
        })
        .await
    }

    /// Release a leased card for retry or terminal delivery failure.
    ///
    /// # Errors
    ///
    /// Returns an error when the failure envelope is invalid, the lease is
    /// stale, or the failure event cannot be committed atomically.
    pub async fn fail_work_notification(
        &self,
        failure: FailWorkNotification,
    ) -> Result<(), StoreError> {
        failure.validate().map_err(StoreError::Work)?;
        let safe_code = noema_tasks::SafeErrorCode::new(failure.error_code.clone())
            .map_err(StoreError::Work)?;
        self.with_immediate_transaction_retry(|transaction| {
            let row = load_leased_notification_tx(
                transaction,
                &failure.notification_id,
                &failure.lease_token,
            )?;
            // The V3 schema has no terminal `dead_letter` status.  A
            // non-retryable failure is therefore durably parked at the
            // maximum sortable timestamp, while retryable failures become
            // available after a bounded delay.
            let next_status = "failed";
            let changed = transaction.execute(
                "UPDATE work_notification_outbox SET status = ?3, available_at = CASE WHEN ?4 = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+60 seconds') ELSE '9999-12-31T23:59:59.999Z' END, lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, last_error_code = ?5, last_error_message = ?6, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE notification_id = ?1 AND status = 'leased' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![failure.notification_id, failure.lease_token, next_status, failure.retryable as i64, safe_code.as_str(), failure.error_message],
            )?;
            if changed != 1 {
                return Err(StoreError::Work(WorkDomainError::RunFenced));
            }
            append_notification_failure_tx(
                transaction,
                failure.notification_id.clone(),
                row.event_sequence,
                &row.kind,
                notification_attempt_count(row.attempt_count)?,
                safe_code.clone(),
                failure.retryable,
            )?;
            Ok(())
        })
        .await
    }
}

fn insert_notification_item_tx(
    transaction: &Transaction<'_>,
    row: &LeasedNotification,
    destination_kind: NotificationDestination,
    notification_kind: NotificationKind,
    notification_id: &str,
    conversation_id: &str,
) -> Result<ConversationItemRecord, StoreError> {
    let item_id =
        deterministic_notification_item_id(notification_id, destination_kind, conversation_id)?;
    let notification_payload: serde_json::Value = serde_json::from_str(&row.payload_json)?;
    let task_id = notification_payload
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "task notification payload has no task identity".to_string(),
        })?;
    let task_id = TaskId::new(task_id.to_string()).map_err(StoreError::Work)?;
    let task = load_task(transaction, &task_id)?.ok_or_else(|| StoreError::InvariantViolation {
        message: "task notification references a missing task".to_string(),
    })?;
    let source_turn_id = (notification_kind == NotificationKind::TaskCreated
        && matches!(
            task.provenance.source_kind,
            TaskSourceKind::ChatCapture | TaskSourceKind::ChatDelegate
        )
        && task.provenance.conversation_id.as_deref() == Some(conversation_id))
    .then(|| task.provenance.turn_id.clone())
    .flatten();
    let item_payload = serde_json::json!({ "task_id": task.task_id.as_str() });
    let next_sequence: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM conversation_items WHERE conversation_id = ?1",
        [conversation_id],
        |value| value.get(0),
    )?;
    let metadata = serde_json::json!({
        "notification_kind": notification_kind.as_str(),
        "notification_id": notification_id,
        "work_notification": notification_payload,
    });
    let inserted = transaction.execute(
        "INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id, content_text, payload_json, metadata_json) VALUES (?1, ?2, ?3, ?4, 'task_reference', 'completed', 'actor:store:notification', NULL, ?5, ?6) ON CONFLICT(item_id) DO NOTHING",
        params![item_id, conversation_id, source_turn_id, next_sequence, item_payload.to_string(), metadata.to_string()],
    )?;
    let item = load_conversation_item_tx(transaction, &item_id)?.ok_or_else(|| {
        StoreError::InvariantViolation {
            message: "notification conversation item disappeared".to_string(),
        }
    })?;
    let valid = item.conversation_id == conversation_id
        && item.turn_id == source_turn_id
        && item.kind == ConversationItemKind::TaskReference
        && item.status == ConversationItemStatus::Completed
        && item.content_text.is_none()
        && item.payload_json == item_payload
        && item.metadata == metadata;
    if !valid {
        return Err(if inserted == 0 {
            StoreError::Work(WorkDomainError::IdempotencyConflict)
        } else {
            StoreError::InvariantViolation {
                message: "inserted notification conversation item failed readback proof"
                    .to_string(),
            }
        });
    }
    Ok(item)
}

fn validate_notification_destination_tx(
    transaction: &Transaction<'_>,
    row: &LeasedNotification,
    conversation_id: &str,
) -> Result<(), StoreError> {
    let expected_conversation: Option<String> = transaction
        .query_row(
            "SELECT primary_conversation_id FROM humans WHERE human_id = ?1",
            [&row.destination_id],
            |value| value.get(0),
        )
        .optional()?
        .flatten();
    if expected_conversation.as_deref() != Some(conversation_id) {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    }
    Ok(())
}

fn should_suppress_near_term_task_reference_tx(
    transaction: &Transaction<'_>,
    row: &LeasedNotification,
    notification_kind: NotificationKind,
    notification_id: &str,
    conversation_id: &str,
) -> Result<bool, StoreError> {
    if notification_kind == NotificationKind::TaskCreated {
        return Ok(false);
    }
    let notification_payload: serde_json::Value = serde_json::from_str(&row.payload_json)?;
    let task_id = notification_payload
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "task notification payload has no task identity".to_string(),
        })?;
    let notification_message_sequence = transaction.query_row(
        "SELECT MAX(sequence_index) FROM conversation_items
             WHERE conversation_id = ?1 AND kind = 'assistant_text'
               AND json_extract(metadata_json, '$.source') = 'work_notification'
               AND json_extract(metadata_json, '$.notification_id') = ?2",
        params![conversation_id, notification_id],
        |value| value.get::<_, Option<i64>>(0),
    )?;
    let Some(notification_message_sequence) = notification_message_sequence else {
        return Ok(false);
    };
    let latest_reference_sequence = transaction.query_row(
        "SELECT MAX(sequence_index) FROM conversation_items
             WHERE conversation_id = ?1 AND sequence_index < ?2
               AND kind = 'task_reference'
               AND json_extract(payload_json, '$.task_id') = ?3",
        params![conversation_id, notification_message_sequence, task_id],
        |value| value.get::<_, Option<i64>>(0),
    )?;
    let Some(latest_reference_sequence) = latest_reference_sequence else {
        return Ok(false);
    };
    let intervening_messages: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM conversation_items
         WHERE conversation_id = ?1 AND sequence_index > ?2 AND sequence_index < ?3
           AND kind IN ('user_text', 'assistant_text', 'multiple_choice_prompt', 'multiple_choice_selection')",
        params![
            conversation_id,
            latest_reference_sequence,
            notification_message_sequence
        ],
        |value| value.get(0),
    )?;
    Ok(intervening_messages <= 1)
}

fn load_leased_notification_tx(
    transaction: &Transaction<'_>,
    notification_id: &str,
    lease_token: &str,
) -> Result<LeasedNotification, StoreError> {
    transaction
        .query_row(
            "SELECT event_sequence, destination_kind, destination_id, notification_kind,
                    payload_json, attempt_count
             FROM work_notification_outbox
             WHERE notification_id = ?1 AND status = 'leased' AND lease_token = ?2
               AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![notification_id, lease_token],
            |row| {
                Ok(LeasedNotification {
                    event_sequence: row.get(0)?,
                    destination_kind: row.get(1)?,
                    destination_id: row.get(2)?,
                    kind: row.get(3)?,
                    payload_json: row.get(4)?,
                    attempt_count: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::Work(WorkDomainError::RunFenced))
}

fn load_claimed_notification_tx(
    transaction: &Transaction<'_>,
    notification_id: &str,
) -> Result<ClaimedWorkNotification, StoreError> {
    let row = transaction.query_row(
        "SELECT event_sequence, destination_kind, destination_id, notification_kind, payload_json, lease_owner, lease_token, attempt_count, lease_expires_at FROM work_notification_outbox WHERE notification_id = ?1 AND status = 'leased'",
        [notification_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, i64>(7)?, row.get::<_, String>(8)?)),
    )?;
    Ok(ClaimedWorkNotification {
        notification_id: notification_id.to_string(),
        event_sequence: u64::try_from(row.0).map_err(|_| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "notification.event_sequence",
                message: "sequence is outside the u64 range".to_string(),
            })
        })?,
        destination_kind: NotificationDestination::from_str(&row.1).map_err(StoreError::Work)?,
        destination_id: row.2,
        notification_kind: NotificationKind::from_str(&row.3).map_err(StoreError::Work)?,
        payload: serde_json::from_str(&row.4)?,
        lease_owner: row.5,
        lease_token: row.6,
        attempt_count: u32::try_from(row.7).map_err(|_| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "notification.attempt_count",
                message: "attempt count is outside the u32 range".to_string(),
            })
        })?,
        lease_expires_at: row.8,
    })
}

fn load_source_event_tx(
    transaction: &Transaction<'_>,
    sequence: i64,
) -> Result<noema_tasks::WorkEventRecord, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE event_sequence = ?1"),
            [sequence],
            decode_work_event_record,
        )
        .map_err(StoreError::Sqlite)
}

fn append_notification_event_tx(
    transaction: &Transaction<'_>,
    source: noema_tasks::WorkEventRecord,
    payload: WorkEventPayload,
) -> Result<(), StoreError> {
    append_work_event_tx(
        transaction,
        WorkEventScope {
            workspace_id: source.workspace_id().clone(),
            project_id: source.project_id().cloned(),
            task_id: source.task_id().cloned(),
            run_id: source.run_id().map(str::to_string),
            actor_id: "actor:store:notification".to_string(),
            causation_id: Some(source.event_id().to_string()),
            correlation_id: source.correlation_id().to_string(),
        },
        payload,
    )?;
    Ok(())
}

fn append_notification_failure_tx(
    transaction: &Transaction<'_>,
    notification_id: String,
    event_sequence: i64,
    kind: &str,
    attempt_count: u32,
    error_code: noema_tasks::SafeErrorCode,
    retryable: bool,
) -> Result<(), StoreError> {
    let source = load_source_event_tx(transaction, event_sequence)?;
    let payload = WorkEventPayload::notification_failed(
        notification_id,
        source.event_sequence(),
        NotificationKind::from_str(kind).map_err(StoreError::Work)?,
        attempt_count,
        error_code,
        retryable,
    )
    .map_err(StoreError::Work)?;
    append_notification_event_tx(transaction, source, payload)
}

fn notification_attempt_count(value: i64) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| StoreError::InvariantViolation {
        message: "notification attempt count exceeds u32".to_string(),
    })
}

fn fail_expired_notification_leases_tx(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let expired = transaction
        .prepare(
            "SELECT notification_id, event_sequence, notification_kind, attempt_count
             FROM work_notification_outbox
             WHERE status = 'leased' AND lease_expires_at IS NOT NULL
               AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             ORDER BY lease_expires_at, notification_id
             LIMIT ?1",
        )?
        .query_map([EXPIRED_LEASE_MAINTENANCE_BATCH], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let error_code = noema_tasks::SafeErrorCode::new("lease_expired").map_err(StoreError::Work)?;
    for (notification_id, event_sequence, kind, attempt_count) in expired {
        let changed = transaction.execute(
            "UPDATE work_notification_outbox
             SET status = 'failed', lease_owner = NULL, lease_token = NULL,
                 lease_expires_at = NULL,
                 available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 last_error_code = 'lease_expired',
                 last_error_message = 'notification lease expired',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE notification_id = ?1 AND status = 'leased'
               AND lease_expires_at IS NOT NULL
               AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            [notification_id.as_str()],
        )?;
        if changed != 1 {
            continue;
        }
        append_notification_failure_tx(
            transaction,
            notification_id,
            event_sequence,
            &kind,
            notification_attempt_count(attempt_count)?,
            error_code.clone(),
            true,
        )?;
    }
    Ok(())
}

fn park_exhausted_notifications_tx(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let exhausted = transaction
        .prepare(
            "SELECT notification_id, event_sequence, notification_kind, attempt_count
             FROM work_notification_outbox
             WHERE status IN ('pending', 'failed') AND attempt_count >= 4294967295
               AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             ORDER BY available_at, notification_id",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (notification_id, event_sequence, kind, attempt_count) in exhausted {
        let changed = transaction.execute(
            "UPDATE work_notification_outbox SET status = 'failed',
                    available_at = '9999-12-31T23:59:59.999Z',
                    lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                    last_error_code = 'attempts_exhausted',
                    last_error_message = 'notification delivery attempts exhausted',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE notification_id = ?1 AND status IN ('pending', 'failed')
               AND attempt_count >= 4294967295",
            [notification_id.as_str()],
        )?;
        if changed != 1 {
            continue;
        }
        append_notification_failure_tx(
            transaction,
            notification_id,
            event_sequence,
            &kind,
            notification_attempt_count(attempt_count)?,
            noema_tasks::SafeErrorCode::new("attempts_exhausted").map_err(StoreError::Work)?,
            false,
        )?;
    }
    Ok(())
}

/// Lease request for pending/failed outbox rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkNotificationLeaseRequest {
    /// Worker identity.
    pub worker_id: String,
    /// Lease duration in seconds.
    pub lease_seconds: i64,
    /// Maximum rows to claim in one bounded batch.
    pub limit: u32,
}

impl WorkNotificationLeaseRequest {
    /// Validate the bounded worker lease envelope.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] for a blank worker, an invalid
    /// lease duration, or a batch limit outside the supported range.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.worker_id.trim().is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.worker_id",
                message: "worker id cannot be blank".to_string(),
            });
        }
        if self.lease_seconds < 1 {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.lease_seconds",
                message: "lease duration must be positive".to_string(),
            });
        }
        if self.limit == 0 || self.limit > 100 {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.limit",
                message: "limit must be between 1 and 100".to_string(),
            });
        }
        Ok(())
    }
}

/// One claimed notification row returned to a delivery worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimedWorkNotification {
    /// Durable notification identity.
    pub notification_id: String,
    /// Source global event sequence.
    pub event_sequence: u64,
    /// Destination kind (V1 is the owner's primary conversation).
    pub destination_kind: NotificationDestination,
    /// Owning human identity used to resolve the current primary conversation.
    pub destination_id: String,
    /// Card kind.
    pub notification_kind: NotificationKind,
    /// Redacted card payload.
    pub payload: serde_json::Value,
    /// Worker that owns the current delivery lease.
    pub lease_owner: String,
    /// Opaque token proving ownership of the current delivery lease.
    pub lease_token: String,
    /// Attempt count after claiming this row.
    pub attempt_count: u32,
    /// Lease expiry timestamp.
    pub lease_expires_at: String,
}

/// Completion input after any required notification narration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteWorkNotification {
    /// Notification row being acknowledged.
    pub notification_id: String,
    /// Exact worker lease token.
    pub lease_token: String,
    /// Destination conversation that received the card.
    pub conversation_id: String,
}

impl CompleteWorkNotification {
    /// Validate completion identity before opening the acknowledgement tx.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when an identity field is blank.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.notification_id.trim().is_empty()
            || self.lease_token.trim().is_empty()
            || self.conversation_id.trim().is_empty()
        {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.complete",
                message: "notification, lease, and conversation are required".to_string(),
            });
        }
        Ok(())
    }
}

/// Failure input for a leased outbox row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailWorkNotification {
    /// Notification row being released/retried.
    pub notification_id: String,
    /// Exact worker lease token.
    pub lease_token: String,
    /// Safe error code only.
    pub error_code: String,
    /// Redacted bounded diagnostic.
    pub error_message: Option<String>,
    /// Whether the worker may retry this row.
    pub retryable: bool,
}

impl FailWorkNotification {
    /// Validate the bounded failure envelope.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] for blank identity/error fields
    /// or an overlong diagnostic message.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.notification_id.trim().is_empty()
            || self.lease_token.trim().is_empty()
            || self.error_code.trim().is_empty()
        {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.failure",
                message: "notification, lease, and error code are required".to_string(),
            });
        }
        if self
            .error_message
            .as_deref()
            .is_some_and(|value| value.len() > 1024)
        {
            return Err(WorkDomainError::InvalidInput {
                field: "notification.error_message",
                message: "error message is too long".to_string(),
            });
        }
        Ok(())
    }
}

/// Deterministic conversation-item identity for one notification/destination.
///
/// The destination is included so a future multi-destination outbox cannot
/// accidentally collide with the V1 human-primary card identity.
pub(crate) fn deterministic_notification_item_id(
    notification_id: &str,
    destination_kind: NotificationDestination,
    destination_id: &str,
) -> Result<String, StoreError> {
    if notification_id.trim().is_empty() || destination_id.trim().is_empty() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "notification.delivery_identity",
            message: "notification and destination cannot be blank".to_string(),
        }));
    }
    let source = format!(
        "noema-work-notification:v1\0{}\0{}\0{}",
        notification_id,
        destination_kind.as_str(),
        destination_id
    );
    let hash = digest(&SHA256, source.as_bytes());
    let mut result = String::with_capacity(24);
    result.push_str("work_notification:");
    for byte in hash.as_ref().iter().take(16) {
        use std::fmt::Write as _;
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(result)
}
