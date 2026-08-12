use super::test_store;
use crate::{ApnsEnvironment, LiveActivityEvent, NewLiveActivityDelivery};
use serde_json::json;
#[tokio::test]
async fn live_registration_binds_to_active_client_and_redacts_tokens() {
    let store = test_store().await;
    store
        .insert_client("client:live-one", "human:local", "Phone", [1; 32])
        .await
        .expect("insert client");
    store
        .insert_client("client:live-old", "human:local", "Old phone", [9; 32])
        .await
        .expect("insert old client");
    store
        .register_client_live_activities(
            "client:live-old",
            &[1, 2, 3],
            ApnsEnvironment::Development,
            &[],
        )
        .await
        .expect("register old client");
    let old_activity = store
        .client_task_activity("client:live-old")
        .await
        .expect("read old activity")
        .expect("old activity");
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live-old".to_string(),
            delivery_key: "live:start:old".to_string(),
            activity_id: old_activity.activity_id.clone(),
            token: vec![1, 2, 3],
            environment: ApnsEnvironment::Development,
            event: LiveActivityEvent::Start,
            payload: json!({"aps":{"event":"start"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue old start");
    assert!(
        store
            .disable_client_live_activities("client:live-one")
            .await
            .expect("persist disabled tombstone")
    );
    let disabled = store
        .client_live_activity_registration("client:live-one")
        .await
        .expect("read disabled tombstone")
        .expect("disabled registration");
    assert!(!disabled.enabled);
    assert!(disabled.push_to_start_token.is_none());
    let registration = store
        .register_client_live_activities(
            "client:live-one",
            &[1, 2, 3],
            ApnsEnvironment::Development,
            &[],
        )
        .await
        .expect("register Live Activity");
    assert!(
        store
            .client_live_activity_registration("client:live-old")
            .await
            .expect("read transferred old registration")
            .is_none()
    );
    assert!(
        store
            .client_task_activity("client:live-old")
            .await
            .expect("read transferred old activity")
            .is_none()
    );
    let old_delivery_status = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status FROM live_activity_deliveries WHERE client_id = 'client:live-old' AND delivery_key = 'live:start:old'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read transferred start");
    assert_eq!(old_delivery_status, "suppressed");
    let debug = format!("{registration:?}");
    assert!(!debug.contains("1, 2, 3"));
    assert!(debug.contains("client:live-one"));
    assert!(
        store
            .register_client_live_activities(
                "client:missing",
                &[1, 2, 3],
                ApnsEnvironment::Development,
                &[],
            )
            .await
            .is_err()
    );
    let activity = store
        .client_task_activity("client:live-one")
        .await
        .expect("read activity")
        .expect("starting activity");
    assert_eq!(activity.lifecycle, "starting");
    let changed = store
        .register_client_live_activity_update(
            "client:live-one",
            activity.activity_id.as_deref().expect("activity id"),
            &[4, 5, 6],
        )
        .await
        .expect("register update token");
    assert!(changed);
    let activity = store
        .client_task_activity("client:live-one")
        .await
        .expect("read active activity")
        .expect("active activity");
    assert!(!format!("{activity:?}").contains("4, 5, 6"));
    assert_eq!(activity.lifecycle, "active");
    let active_activity_id = activity.activity_id.clone().expect("active activity id");
    store
        .register_client_live_activities(
            "client:live-one",
            &[1, 2, 3],
            ApnsEnvironment::Development,
            &[active_activity_id],
        )
        .await
        .expect("preserve reported activity");
    let preserved = store
        .client_task_activity("client:live-one")
        .await
        .expect("read preserved activity")
        .expect("preserved activity");
    assert_eq!(preserved.activity_id, activity.activity_id);
    assert_eq!(preserved.task_session_id, activity.task_session_id);
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live-one".to_string(),
            delivery_key: "live:update:stale".to_string(),
            activity_id: preserved.activity_id.clone(),
            token: vec![4, 5, 6],
            environment: ApnsEnvironment::Development,
            event: LiveActivityEvent::Update,
            payload: json!({"aps":{"event":"update"}}),
            urgency: "normal".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue stale update");

    store
        .register_client_live_activities(
            "client:live-one",
            &[1, 2, 3],
            ApnsEnvironment::Development,
            &[],
        )
        .await
        .expect("replace missing activity");

    let replacement = store
        .client_task_activity("client:live-one")
        .await
        .expect("read replacement")
        .expect("replacement activity");
    assert_eq!(replacement.lifecycle, "starting");
    assert_ne!(replacement.activity_id, preserved.activity_id);
    assert!(replacement.update_token.is_none());
    let delivery = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status, last_error_code FROM live_activity_deliveries WHERE client_id = 'client:live-one' AND delivery_key = 'live:update:stale'",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read stale delivery");
    assert_eq!(
        delivery,
        ("suppressed".to_string(), "activity_missing".to_string())
    );
}

#[tokio::test]
async fn live_activity_end_delivery_dismisses_and_allows_a_new_session() {
    let store = test_store().await;
    store
        .insert_client("client:live", "human:local", "Phone", [2; 32])
        .await
        .expect("insert client");
    store
        .register_client_live_activities(
            "client:live",
            &[1, 2, 3],
            ApnsEnvironment::Production,
            &[],
        )
        .await
        .expect("register Live Activity");
    let starting = store
        .client_task_activity("client:live")
        .await
        .expect("read starting activity")
        .expect("starting activity");
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live".to_string(),
            delivery_key: "live:start:test".to_string(),
            activity_id: starting.activity_id.clone(),
            token: vec![1, 2, 3],
            environment: ApnsEnvironment::Production,
            event: LiveActivityEvent::Start,
            payload: json!({"aps":{"event":"start"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue start");
    let start = store
        .claim_due_live_activity_delivery()
        .await
        .expect("claim start")
        .expect("start delivery");
    assert_eq!(start.event, LiveActivityEvent::Start);
    store
        .finish_live_activity_delivery(&start, "delivered", None)
        .await
        .expect("finish start");
    store
        .register_client_live_activity_update(
            "client:live",
            starting.activity_id.as_deref().expect("activity id"),
            &[4, 5, 6],
        )
        .await
        .expect("register update");
    let active = store
        .client_task_activity("client:live")
        .await
        .expect("read active activity")
        .expect("active activity");
    store
        .update_client_task_activity_projection(
            "client:live",
            &json!({"focusTaskId":"task:one","phase":"working"}),
            &"a".repeat(64),
            Some("task:one"),
        )
        .await
        .expect("save projection");
    store
        .mark_client_task_activity_ending("client:live", active.activity_id.as_deref())
        .await
        .expect("mark ending");
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live".to_string(),
            delivery_key: "live:end:test".to_string(),
            activity_id: active.activity_id.clone(),
            token: vec![4, 5, 6],
            environment: ApnsEnvironment::Production,
            event: LiveActivityEvent::End,
            payload: json!({"aps":{"event":"end"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue end");
    let end = store
        .claim_due_live_activity_delivery()
        .await
        .expect("claim end")
        .expect("end delivery");
    assert_eq!(end.event, LiveActivityEvent::End);
    store
        .finish_live_activity_delivery(&end, "delivered", None)
        .await
        .expect("finish end");
    assert_eq!(
        store
            .client_task_activity("client:live")
            .await
            .expect("read dismissed activity")
            .expect("activity row")
            .lifecycle,
        "dismissed"
    );
    store
        .register_client_live_activities(
            "client:live",
            &[1, 2, 3],
            ApnsEnvironment::Production,
            &[],
        )
        .await
        .expect("refresh registration");
    let dismissed = store
        .client_task_activity("client:live")
        .await
        .expect("read refreshed activity")
        .expect("refreshed activity");
    assert_eq!(dismissed.lifecycle, "dismissed");
    assert_eq!(dismissed.task_session_id, starting.task_session_id);
    assert_eq!(dismissed.activity_id, starting.activity_id);
    store
        .ensure_client_task_activity_session("client:live")
        .await
        .expect("create next session");
    let next = store
        .client_task_activity("client:live")
        .await
        .expect("read next activity")
        .expect("next activity");
    assert_eq!(next.lifecycle, "starting");
    assert_ne!(next.task_session_id, starting.task_session_id);
    assert_ne!(next.activity_id, starting.activity_id);
    assert!(next.update_token.is_none());
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live".to_string(),
            delivery_key: format!("live:start:{}", next.task_session_id),
            activity_id: next.activity_id.clone(),
            token: vec![1, 2, 3],
            environment: ApnsEnvironment::Production,
            event: LiveActivityEvent::Start,
            payload: json!({"aps":{"event":"start"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue second start");
    let second_start = store
        .claim_due_live_activity_delivery()
        .await
        .expect("claim second start")
        .expect("second start delivery");
    store
        .finish_live_activity_delivery(&second_start, "invalid_token", None)
        .await
        .expect("finish invalid start");
    store
        .register_client_live_activities(
            "client:live",
            &[7, 8, 9],
            ApnsEnvironment::Production,
            &[],
        )
        .await
        .expect("rotate after invalid start");
    let rotated = store
        .client_task_activity("client:live")
        .await
        .expect("read rotated activity")
        .expect("rotated activity");
    assert_eq!(rotated.lifecycle, "starting");
    assert_ne!(rotated.task_session_id, next.task_session_id);
    assert_ne!(rotated.activity_id, next.activity_id);
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:live".to_string(),
            delivery_key: "live:start:disable".to_string(),
            activity_id: rotated.activity_id.clone(),
            token: vec![7, 8, 9],
            environment: ApnsEnvironment::Production,
            event: LiveActivityEvent::Start,
            payload: json!({"aps":{"event":"start"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue disable start");
    store
        .disable_client_live_activities("client:live")
        .await
        .expect("disable Live Activities");
    assert!(
        store
            .claim_due_live_activity_delivery()
            .await
            .expect("claim after disable")
            .is_none()
    );
}
