use std::collections::HashSet;

use crate::NewWebPushSubscription;

use super::test_store;

fn subscription(endpoint: &str) -> NewWebPushSubscription {
    NewWebPushSubscription {
        owner_human_id: "human:local".to_string(),
        browser_session_hash: [3; 32],
        endpoint: endpoint.to_string(),
        p256dh: "p".repeat(87),
        auth_secret: "a".repeat(22),
    }
}

#[tokio::test]
async fn web_push_identity_and_endpoint_registration_are_stable() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let identity = store
        .get_or_insert_web_push_identity(&[1; 32], &[2; 65])
        .await
        .expect("insert Web Push identity");
    assert_eq!(identity.private_key, vec![1; 32]);
    let reopened = store
        .get_or_insert_web_push_identity(&[3; 32], &[4; 65])
        .await
        .expect("reopen Web Push identity");
    assert_eq!(reopened, identity);

    let first = store
        .register_web_push_subscription(subscription("https://push.example/one"))
        .await
        .expect("register browser");
    let refreshed = store
        .register_web_push_subscription(subscription("https://push.example/one"))
        .await
        .expect("refresh browser");
    assert_eq!(refreshed.subscription_id, first.subscription_id);
}

#[tokio::test]
async fn browser_session_revocation_removes_only_its_push_authority() {
    let store = test_store().await;
    let first = store
        .register_web_push_subscription(subscription("https://push.example/one"))
        .await
        .expect("register first browser");
    let mut second_input = subscription("https://push.example/two");
    second_input.browser_session_hash = [4; 32];
    let second = store
        .register_web_push_subscription(second_input)
        .await
        .expect("register second browser");

    assert_eq!(
        store
            .remove_web_push_for_session([3; 32])
            .await
            .expect("revoke first session"),
        1
    );
    assert!(
        store
            .web_push_subscription_for_endpoint("human:local", [3; 32], &first.endpoint,)
            .await
            .expect("first lookup")
            .is_none()
    );
    assert!(
        store
            .web_push_subscription_for_endpoint("human:local", [4; 32], &second.endpoint,)
            .await
            .expect("second lookup")
            .is_some()
    );
}

#[tokio::test]
async fn queue_suppresses_only_the_visible_subscription() {
    let store = test_store().await;
    store
        .get_or_insert_web_push_identity(&[1; 32], &[2; 65])
        .await
        .expect("insert Web Push identity");
    let visible = store
        .register_web_push_subscription(subscription("https://push.example/visible"))
        .await
        .expect("register visible browser");
    let other = store
        .register_web_push_subscription(subscription("https://push.example/other"))
        .await
        .expect("register other browser");

    store
        .queue_notification_fanout(
            "human:local",
            "chat-turn:one",
            "Noema",
            "Finished",
            "normal",
            3600,
            &HashSet::from([visible.subscription_id.clone()]),
            &HashSet::new(),
            false,
        )
        .await
        .expect("queue notification");

    let statuses = store
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT subscription_id, status FROM web_push_deliveries ORDER BY subscription_id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read delivery status");
    assert!(statuses.contains(&(visible.subscription_id, "suppressed".to_string())));
    assert!(statuses.contains(&(other.subscription_id, "pending".to_string())));
}

#[tokio::test]
async fn primary_checkpoint_is_monotonic_until_the_conversation_changes() {
    let store = test_store().await;
    let primary = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("ensure primary conversation");
    store
        .advance_notification_primary_checkpoint(&primary.conversation_id, 8)
        .await
        .expect("advance checkpoint");
    store
        .advance_notification_primary_checkpoint(&primary.conversation_id, 3)
        .await
        .expect("ignore older checkpoint");
    assert_eq!(
        store
            .with_connection(|connection| Ok(connection.changes()))
            .await
            .expect("checkpoint changes"),
        0
    );
    let checkpoint = store
        .notification_primary_checkpoint()
        .await
        .expect("read checkpoint");
    assert_eq!(checkpoint.sequence, 8);
}
