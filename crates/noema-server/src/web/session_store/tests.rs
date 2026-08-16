use super::*;

fn record(expiry_date: OffsetDateTime) -> Record {
    Record {
        id: Id::default(),
        data: HashMap::new(),
        expiry_date,
    }
}

#[tokio::test]
async fn capacity_rejects_new_sessions_without_evicting_active_sessions() {
    let store = BoundedSessionStore::new(1);
    let mut first = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut first).await.expect("first session");
    let mut second = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);

    assert!(store.create(&mut second).await.is_err());
    assert!(store.load(&first.id).await.expect("load first").is_some());

    store.with_entries(|entries| {
        entries
            .get_mut(&first.id)
            .expect("first entry")
            .record
            .expiry_date = OffsetDateTime::now_utc() - Duration::SECOND;
    });
    store.create(&mut second).await.expect("reclaimed session");
    assert!(store.load(&first.id).await.expect("load expired").is_none());
    assert!(store.load(&second.id).await.expect("load second").is_some());
}

#[tokio::test]
async fn idle_and_absolute_expiry_remove_sessions_and_announce_revocation() {
    let store = BoundedSessionStore::new(2);
    let mut idle = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    let mut absolute = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut idle).await.expect("idle session");
    store.create(&mut absolute).await.expect("absolute session");
    let mut revocations = store.subscribe_revocations();
    store.with_entries(|entries| {
        entries
            .get_mut(&idle.id)
            .expect("idle entry")
            .record
            .expiry_date = OffsetDateTime::now_utc() - Duration::SECOND;
        entries
            .get_mut(&absolute.id)
            .expect("absolute entry")
            .created_at = OffsetDateTime::now_utc() - ABSOLUTE_EXPIRY - Duration::SECOND;
    });

    store.delete_expired();

    let mut revoked = [
        revocations.try_recv().expect("first revocation"),
        revocations.try_recv().expect("second revocation"),
    ];
    revoked.sort_by_key(|id| id.0);
    let mut expected = [idle.id, absolute.id];
    expected.sort_by_key(|id| id.0);
    assert_eq!(revoked, expected);
    assert!(store.load(&idle.id).await.expect("load idle").is_none());
    assert!(
        store
            .load(&absolute.id)
            .await
            .expect("load absolute")
            .is_none()
    );
}

#[tokio::test]
async fn deletion_is_targeted_and_announces_revocation() {
    let store = BoundedSessionStore::new(2);
    let mut first = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    let mut second = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut first).await.expect("first session");
    store.create(&mut second).await.expect("second session");
    let mut revocations = store.subscribe_revocations();

    store.delete(&first.id).await.expect("delete first");

    assert_eq!(revocations.try_recv().expect("revocation"), first.id);
    assert!(store.load(&first.id).await.expect("load first").is_none());
    assert!(store.load(&second.id).await.expect("load second").is_some());
}
