use super::test_store;
use crate::{
    ApnsEnvironment, LiveActivityEvent, NewLiveActivityDelivery, NotificationDeliveryOutcome,
};
use serde_json::json;

#[tokio::test]
async fn live_activity_client_callbacks_form_a_secret_free_timeline() {
    let store = test_store().await;
    store
        .insert_client("client:timeline", "human:local", "Phone", [1; 32])
        .await
        .expect("insert client");
    store
        .register_client_live_activities(
            "client:timeline",
            &[1, 2, 3],
            ApnsEnvironment::Development,
            &["live_activity:observed:test".to_string()],
        )
        .await
        .expect("register snapshot");
    let activity = store
        .client_task_activity("client:timeline")
        .await
        .expect("read activity")
        .expect("starting activity");
    let activity_id = activity.activity_id.expect("activity id");
    assert!(
        store
            .register_client_live_activity_update("client:timeline", &activity_id, &[4, 5, 6],)
            .await
            .expect("register update token")
    );
    assert!(
        store
            .dismiss_client_live_activity("client:timeline", &activity_id, true)
            .await
            .expect("dismiss activity")
    );

    let observations = store
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT event, activity_id, active_activity_ids_json FROM live_activity_observations WHERE client_id = ?1 ORDER BY rowid",
            )?;
            let rows = statement.query_map(["client:timeline"], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read observations");
    assert_eq!(
        observations,
        vec![
            (
                "snapshot".to_string(),
                None,
                "[\"live_activity:observed:test\"]".to_string(),
            ),
            (
                "update_token".to_string(),
                Some(activity_id.clone()),
                "[]".to_string(),
            ),
            ("dismissed".to_string(), Some(activity_id), "[]".to_string(),),
        ]
    );
    assert!(!format!("{observations:?}").contains("1, 2, 3"));
    assert!(!format!("{observations:?}").contains("4, 5, 6"));
}

#[tokio::test]
async fn terminal_live_activity_delivery_keeps_apns_id_for_thirty_days() {
    let store = test_store().await;
    store
        .insert_client("client:delivery", "human:local", "Phone", [1; 32])
        .await
        .expect("insert client");
    store
        .register_client_live_activities(
            "client:delivery",
            &[1, 2, 3],
            ApnsEnvironment::Production,
            &[],
        )
        .await
        .expect("register Live Activities");
    let activity = store
        .client_task_activity("client:delivery")
        .await
        .expect("read activity")
        .expect("activity");
    store
        .queue_live_activity_delivery(NewLiveActivityDelivery {
            client_id: "client:delivery".to_string(),
            delivery_key: "live:start:retention".to_string(),
            activity_id: activity.activity_id,
            token: vec![1, 2, 3],
            environment: ApnsEnvironment::Production,
            event: LiveActivityEvent::Start,
            payload: json!({"aps":{"event":"start"}}),
            urgency: "high".to_string(),
            ttl_seconds: 600,
        })
        .await
        .expect("queue delivery");
    let delivery = store
        .claim_due_live_activity_delivery()
        .await
        .expect("claim delivery")
        .expect("delivery");
    store
        .finish_live_activity_delivery(
            &delivery,
            NotificationDeliveryOutcome::Delivered,
            Some("apns-retained"),
        )
        .await
        .expect("finish delivery");
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE live_activity_deliveries SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-29 days') WHERE client_id = 'client:delivery'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("age recent delivery");
    assert!(
        store
            .claim_due_live_activity_delivery()
            .await
            .expect("run recent retention")
            .is_none()
    );
    let apns_id = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT apns_id FROM live_activity_deliveries WHERE client_id = 'client:delivery'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read APNs identifier");
    assert_eq!(apns_id, "apns-retained");

    store
        .with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE live_activity_deliveries SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-31 days') WHERE client_id = 'client:delivery'",
                [],
            )?;
            assert_eq!(changed, 1);
            Ok(())
        })
        .await
        .expect("age expired delivery");
    assert!(
        store
            .claim_due_live_activity_delivery()
            .await
            .expect("run expired retention")
            .is_none()
    );
    let count = store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM live_activity_deliveries WHERE client_id = 'client:delivery'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("count retained deliveries");
    assert_eq!(count, 0);
}
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
async fn disabling_live_activities_clears_a_dismissed_session() {
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
        .expect("register Live Activities");
    let activity = store
        .client_task_activity("client:live")
        .await
        .expect("read activity")
        .expect("starting activity");
    store
        .update_client_task_activity_projection(
            "client:live",
            &json!({"focusTaskId":"task:one"}),
            &"a".repeat(64),
            Some("task:one"),
        )
        .await
        .expect("save projection");
    store
        .dismiss_client_live_activity(
            "client:live",
            activity.activity_id.as_deref().expect("activity id"),
            false,
        )
        .await
        .expect("dismiss activity");

    store
        .disable_client_live_activities("client:live")
        .await
        .expect("disable Live Activities");

    let activity = store
        .client_task_activity("client:live")
        .await
        .expect("reload activity")
        .expect("dismissed activity");
    assert_eq!(activity.lifecycle, "dismissed");
    assert!(activity.latest_projection_signature.is_empty());
    assert!(activity.focused_task_id.is_none());
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE notification_projection_state SET task_notification_sequence = 10, updated_at = '2000-01-01T00:00:00.000Z' WHERE state_id = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("seed unchanged checkpoint");
    assert!(
        !store
            .clear_client_task_activity_dismissal("client:live")
            .await
            .expect("repeat dismissal clear")
    );
    store
        .advance_notification_task_checkpoint(10)
        .await
        .expect("repeat checkpoint");
    assert_eq!(
        store
            .with_connection(|connection| connection
                .query_row(
                    "SELECT updated_at FROM notification_projection_state WHERE state_id = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(crate::StoreError::Sqlite))
            .await
            .expect("unchanged checkpoint"),
        "2000-01-01T00:00:00.000Z"
    );

    store
        .register_client_live_activities(
            "client:live",
            &[1, 2, 3],
            ApnsEnvironment::Production,
            &[],
        )
        .await
        .expect("enable Live Activities");
    assert!(
        store
            .ensure_client_task_activity_session("client:live")
            .await
            .expect("create replacement session")
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
        .finish_live_activity_delivery(
            &start,
            NotificationDeliveryOutcome::Delivered,
            Some("apns-start"),
        )
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
        .finish_live_activity_delivery(
            &end,
            NotificationDeliveryOutcome::Delivered,
            Some("apns-end"),
        )
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
        .finish_live_activity_delivery(
            &second_start,
            NotificationDeliveryOutcome::InvalidToken,
            None,
        )
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
