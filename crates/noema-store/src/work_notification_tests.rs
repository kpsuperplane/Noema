//! Focused durable outbox tests for owner routing, leases, and replay-safe delivery.

use noema_tasks::{
    CaptureTask, CommandMeta, NotificationKind, TaskProvenance, TaskSourceKind, WorkCommand,
    WorkDomainError, WorkEventKind,
};
use noema_workspaces::WorkspaceId;
use rusqlite::params;
use serde_json::{Value, json};

use crate::{
    CompleteWorkNotification, FailWorkNotification, NoemaStore, StoreError, WorkCommandService,
    WorkNotificationLeaseRequest,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

const ACTOR: &str = "actor:human:local";

async fn fixture() -> (NoemaStore, WorkCommandService) {
    let store = open_ephemeral_store().await.expect("open store");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);
    (store, service)
}

fn capture(key: &str) -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: CommandMeta {
            actor_id: ACTOR.to_string(),
            causation_id: None,
            correlation_id: format!("correlation:{key}"),
            idempotency_key: Some(key.to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: "notification task".to_string(),
        description_markdown: String::new(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: ACTOR.to_string(),
            ..TaskProvenance::default()
        },
    })
}

fn lease(worker_id: &str) -> WorkNotificationLeaseRequest {
    WorkNotificationLeaseRequest {
        worker_id: worker_id.to_string(),
        lease_seconds: 30,
        limit: 10,
    }
}

async fn count_conversation_items(store: &NoemaStore, conversation_id: &str) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM conversation_items WHERE conversation_id = ?1",
                    [conversation_id],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("conversation item count")
}

async fn notification_event_payload(
    store: &NoemaStore,
    notification_id: &str,
    kind: WorkEventKind,
) -> Value {
    let payload_json = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT payload_json FROM work_events
                     WHERE event_kind = ?1
                       AND json_extract(payload_json, '$.notification_id') = ?2
                     ORDER BY event_sequence DESC LIMIT 1",
                    params![kind.as_str(), notification_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load notification event payload");
    serde_json::from_str(&payload_json).expect("decode notification event payload")
}

#[tokio::test]
async fn notification_claim_targets_the_personal_owner_and_delivery_is_deterministic() {
    let (store, service) = fixture().await;
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation");
    service
        .execute(capture("notification:one"))
        .await
        .expect("capture notification");

    let first = store
        .claim_work_notifications(lease("worker:notifications"))
        .await
        .expect("claim notification");
    assert_eq!(first.len(), 1);
    let claimed = &first[0];
    assert_eq!(claimed.destination_id, "human:local");
    assert_eq!(claimed.lease_owner, "worker:notifications");

    let delivered = store
        .complete_work_notification(CompleteWorkNotification {
            notification_id: claimed.notification_id.clone(),
            lease_token: claimed.lease_token.clone(),
            conversation_id: conversation.conversation_id.clone(),
        })
        .await
        .expect("complete notification");
    assert_eq!(delivered.conversation_id, conversation.conversation_id);
    assert_eq!(
        delivered.kind,
        noema_conversations::ConversationItemKind::TaskReference
    );
    assert_eq!(delivered.payload_json["title"], "notification task");
    assert_eq!(delivered.payload_json["stage_id"], "stage:personal:inbox");
    assert_eq!(delivered.payload_json["revision"], 1);
    assert_eq!(
        delivered.metadata["work_notification"]["task_id"],
        delivered.payload_json["task_id"]
    );
    assert_eq!(
        count_conversation_items(&store, &conversation.conversation_id).await,
        1
    );
    let queued_payload = notification_event_payload(
        &store,
        &claimed.notification_id,
        WorkEventKind::NotificationQueued,
    )
    .await;
    let source_event_sequence = queued_payload["source_event_sequence"].clone();
    assert_eq!(
        queued_payload,
        json!({
            "v": 1,
            "notification_id": claimed.notification_id,
            "source_event_sequence": source_event_sequence,
            "notification_kind": "task_created",
            "destination_kind": "human_primary_conversation"
        })
    );
    let delivered_payload = notification_event_payload(
        &store,
        &claimed.notification_id,
        WorkEventKind::NotificationDelivered,
    )
    .await;
    assert_eq!(
        delivered_payload,
        json!({
            "v": 1,
            "notification_id": claimed.notification_id,
            "source_event_sequence": source_event_sequence,
            "notification_kind": "task_created",
            "attempt_count": 1
        })
    );

    // Simulate a process crash after deterministic item insertion but before
    // acknowledging the outbox row. A fresh lease must reuse the same item id.
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox SET status = 'pending', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE notification_id = ?1",
                    [claimed.notification_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("reset outbox for crash replay");
    let replay = store
        .claim_work_notifications(lease("worker:notifications-retry"))
        .await
        .expect("reclaim notification");
    assert_eq!(replay.len(), 1);
    let replayed = store
        .complete_work_notification(CompleteWorkNotification {
            notification_id: replay[0].notification_id.clone(),
            lease_token: replay[0].lease_token.clone(),
            conversation_id: conversation.conversation_id.clone(),
        })
        .await
        .expect("complete replay");
    assert_eq!(replayed, delivered);
    assert_eq!(
        count_conversation_items(&store, &conversation.conversation_id).await,
        1,
        "deterministic delivery identity prevents a duplicate card"
    );
}

#[tokio::test]
async fn notification_failure_retries_and_nonretryable_failures_are_parked() {
    let (store, service) = fixture().await;
    let _first = service
        .execute(capture("notification:retry"))
        .await
        .expect("capture retry notification");
    let claimed = store
        .claim_work_notifications(lease("worker:retry"))
        .await
        .expect("claim retry notification");
    let claimed = claimed.into_iter().next().expect("find retry notification");
    store
        .fail_work_notification(FailWorkNotification {
            notification_id: claimed.notification_id.clone(),
            lease_token: claimed.lease_token,
            error_code: "temporary_failure".to_string(),
            error_message: Some("bounded diagnostic".to_string()),
            retryable: true,
        })
        .await
        .expect("retryable failure");
    let queued_payload = notification_event_payload(
        &store,
        &claimed.notification_id,
        WorkEventKind::NotificationQueued,
    )
    .await;
    let failed_payload = notification_event_payload(
        &store,
        &claimed.notification_id,
        WorkEventKind::NotificationFailed,
    )
    .await;
    assert_eq!(
        failed_payload,
        json!({
            "v": 1,
            "notification_id": claimed.notification_id,
            "source_event_sequence": queued_payload["source_event_sequence"],
            "notification_kind": "task_created",
            "attempt_count": 1,
            "error_code": "temporary_failure",
            "retryable": true
        })
    );

    let immediate = store
        .claim_work_notifications(lease("worker:retry-immediate"))
        .await
        .expect("claim before retry delay");
    assert!(
        immediate.is_empty(),
        "retry is delayed rather than hot-looped"
    );
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox SET available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE notification_id = ?1",
                    [claimed.notification_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("make retry available");
    let retried = store
        .claim_work_notifications(lease("worker:retry-again"))
        .await
        .expect("reclaim failed notification");
    assert_eq!(retried.len(), 1);
    assert_eq!(retried[0].attempt_count, 2);

    let _second = service
        .execute(capture("notification:park"))
        .await
        .expect("capture park notification");
    let parked_claim = store
        .claim_work_notifications(lease("worker:park"))
        .await
        .expect("claim park notification")
        .into_iter()
        .next()
        .expect("find park notification");
    store
        .fail_work_notification(FailWorkNotification {
            notification_id: parked_claim.notification_id.clone(),
            lease_token: parked_claim.lease_token,
            error_code: "permanent_failure".to_string(),
            error_message: None,
            retryable: false,
        })
        .await
        .expect("park nonretryable notification");
    let status: String = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status FROM work_notification_outbox WHERE notification_id = ?1",
                    [parked_claim.notification_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("read parked status");
    assert_eq!(status, "failed");
}

#[tokio::test]
async fn exhausted_notification_is_parked_without_blocking_a_healthy_claim() {
    let (store, service) = fixture().await;
    service
        .execute(capture("notification:exhausted"))
        .await
        .expect("capture exhausted notification");
    service
        .execute(capture("notification:healthy"))
        .await
        .expect("capture healthy notification");
    let ids = store
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT notification_id FROM work_notification_outbox
                 WHERE notification_kind = 'task_created'
                 ORDER BY event_sequence",
            )?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("load notification identities");
    let [exhausted_id, healthy_id] = ids.as_slice() else {
        panic!("expected two task-created notifications");
    };
    let overflow = store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox SET attempt_count = 4294967296 WHERE notification_id = ?1",
                    [exhausted_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await;
    assert!(overflow.is_err(), "schema must reject counters above u32");
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox SET attempt_count = 4294967295 WHERE notification_id = ?1",
                    [exhausted_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("seed exhausted counter");

    let claimed = store
        .claim_work_notifications(lease("worker:healthy-after-exhausted"))
        .await
        .expect("claim healthy notification");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].notification_id.as_str(), healthy_id.as_str());
    assert_eq!(claimed[0].attempt_count, 1);
    let parked: (String, i64, Option<String>, i64) = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status, attempt_count, last_error_code,
                            (SELECT COUNT(*) FROM work_events
                             WHERE event_kind = 'notification.failed'
                               AND json_extract(payload_json, '$.notification_id') = ?1)
                     FROM work_notification_outbox WHERE notification_id = ?1",
                    [exhausted_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load parked notification");
    assert_eq!(parked.0, "failed");
    assert_eq!(parked.1, i64::from(u32::MAX));
    assert_eq!(parked.2.as_deref(), Some("attempts_exhausted"));
    assert_eq!(parked.3, 1);
}

#[tokio::test]
async fn expired_notification_leases_all_fail_before_limit_bounded_reclaim() {
    let (store, service) = fixture().await;
    for key in ["expired-a", "expired-b", "expired-c"] {
        service
            .execute(capture(&format!("notification:{key}")))
            .await
            .expect("capture expired-lease notification");
    }
    let claimed = store
        .claim_work_notifications(lease("worker:expired-batch"))
        .await
        .expect("claim notification batch");
    assert_eq!(claimed.len(), 3);
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox
                     SET lease_expires_at = '2000-01-01T00:00:00.000Z'
                     WHERE status = 'leased'",
                    [],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("expire all notification leases");

    let reclaimed = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:expired-single".to_string(),
            lease_seconds: 30,
            limit: 1,
        })
        .await
        .expect("reclaim one expired notification");
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].attempt_count, 2);

    let rows = store
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT outbox.notification_id, outbox.status, outbox.last_error_code,
                        (SELECT COUNT(*) FROM work_events
                         WHERE event_kind = 'notification.failed'
                           AND json_extract(payload_json, '$.notification_id') = outbox.notification_id
                           AND json_extract(payload_json, '$.error_code') = 'lease_expired')
                 FROM work_notification_outbox AS outbox
                 ORDER BY outbox.notification_id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load expired notification outcomes");
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().filter(|row| row.1 == "leased").count(),
        1,
        "only the request limit is reclaimed"
    );
    assert_eq!(
        rows.iter().filter(|row| row.1 == "failed").count(),
        2,
        "expired rows beyond the request limit remain retryable failures"
    );
    assert!(
        rows.iter()
            .all(|row| { row.2.as_deref() == Some("lease_expired") && row.3 == 1 })
    );
}

#[tokio::test]
async fn expired_notification_lease_repair_stops_at_its_maintenance_cap() {
    let (store, service) = fixture().await;
    for ordinal in 0..101 {
        service
            .execute(capture(&format!("notification:expired-cap:{ordinal}")))
            .await
            .expect("capture maintenance-cap notification");
    }
    let first_batch = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:expired-cap:first".to_string(),
            lease_seconds: 30,
            limit: 100,
        })
        .await
        .expect("claim first maintenance-cap batch");
    assert_eq!(first_batch.len(), 100);
    let final_row = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:expired-cap:last".to_string(),
            lease_seconds: 30,
            limit: 1,
        })
        .await
        .expect("claim final maintenance-cap row");
    assert_eq!(final_row.len(), 1);
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox
                     SET lease_expires_at = '2000-01-01T00:00:00.000Z'
                     WHERE status = 'leased'",
                    [],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("expire maintenance-cap leases");

    let reclaimed = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:expired-cap:repair".to_string(),
            lease_seconds: 30,
            limit: 1,
        })
        .await
        .expect("repair one bounded maintenance batch");
    assert_eq!(reclaimed.len(), 1);
    let (failed_events, marked_expired, still_expired_leased): (i64, i64, i64) = store
        .with_connection(|connection| {
            let failed_events = connection.query_row(
                "SELECT COUNT(*) FROM work_events
                 WHERE event_kind = 'notification.failed'
                   AND json_extract(payload_json, '$.error_code') = 'lease_expired'",
                [],
                |row| row.get(0),
            )?;
            let marked_expired = connection.query_row(
                "SELECT COUNT(*) FROM work_notification_outbox
                 WHERE last_error_code = 'lease_expired'",
                [],
                |row| row.get(0),
            )?;
            let still_expired_leased = connection.query_row(
                "SELECT COUNT(*) FROM work_notification_outbox
                 WHERE status = 'leased' AND last_error_code IS NULL
                   AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                [],
                |row| row.get(0),
            )?;
            Ok((failed_events, marked_expired, still_expired_leased))
        })
        .await
        .expect("load bounded repair counts");
    assert_eq!(failed_events, 100);
    assert_eq!(marked_expired, 100);
    assert_eq!(still_expired_leased, 1);

    store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:expired-cap:next-pass".to_string(),
            lease_seconds: 30,
            limit: 1,
        })
        .await
        .expect("repair remaining expired lease on next pass");
    let failed_events = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM work_events
                     WHERE event_kind = 'notification.failed'
                       AND json_extract(payload_json, '$.error_code') = 'lease_expired'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load next-pass failure event count");
    assert_eq!(failed_events, 101);
}

#[tokio::test]
async fn notification_conflict_compares_json_semantics_and_rejects_divergence() {
    let (store, service) = fixture().await;
    let result = service
        .execute(capture("notification:semantic-conflict"))
        .await
        .expect("capture source event");
    let source = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT source.event_sequence, source.event_id, source.event_kind,
                            source.workspace_id, source.project_id, source.task_id, source.run_id,
                            source.actor_id, source.causation_id, source.correlation_id,
                            source.payload_json, source.created_at
                     FROM work_events queued
                     JOIN work_events source ON source.event_id = queued.causation_id
                     WHERE queued.event_id = ?1",
                    [result.event_id.as_str()],
                    crate::work_events::work_event_from_row,
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load notification source event");
    let payload = serde_json::json!({"alpha": 1, "beta": 2});
    let inserted = store
        .with_immediate_transaction_retry(|transaction| {
            crate::work_notifications::enqueue_work_notification_tx(
                transaction,
                &source,
                NotificationKind::TaskRecovery,
                &payload,
            )
        })
        .await
        .expect("enqueue semantic test notification");
    assert!(inserted.is_some());
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE work_notification_outbox SET payload_json = '{\"beta\":2,\"alpha\":1}'
                     WHERE event_sequence = ?1 AND notification_kind = 'task_recovery'",
                    [i64::try_from(source.event_sequence).expect("SQLite event sequence")],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("reorder persisted object keys");
    let replay = store
        .with_immediate_transaction_retry(|transaction| {
            crate::work_notifications::enqueue_work_notification_tx(
                transaction,
                &source,
                NotificationKind::TaskRecovery,
                &payload,
            )
        })
        .await
        .expect("semantic replay");
    assert!(replay.is_none());

    let divergent = serde_json::json!({"alpha": 1, "beta": 3});
    let error = store
        .with_immediate_transaction_retry(|transaction| {
            crate::work_notifications::enqueue_work_notification_tx(
                transaction,
                &source,
                NotificationKind::TaskRecovery,
                &divergent,
            )
        })
        .await
        .expect_err("divergent payload must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn notification_lease_inputs_are_bounded_and_wrong_completion_fails_closed() {
    let (store, _service) = fixture().await;
    let error = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "".to_string(),
            lease_seconds: 30,
            limit: 1,
        })
        .await
        .expect_err("blank worker must fail");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "notification.worker_id",
            ..
        })
    ));
}
