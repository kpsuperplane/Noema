use super::*;

fn record(expiry_date: OffsetDateTime) -> Record {
    Record {
        id: Id::default(),
        data: Default::default(),
        expiry_date,
    }
}

async fn store(max_sessions: usize) -> (tempfile::TempDir, BoundedSessionStore) {
    let home = tempfile::tempdir().expect("session home");
    let noema = noema_store::NoemaStore::open(&noema_store::StoreConfig::new(
        home.path().join("db/noema.sqlite3"),
    ))
    .await
    .expect("session store");
    (home, BoundedSessionStore::new(noema, max_sessions))
}

#[tokio::test]
async fn capacity_rejects_new_sessions_without_evicting_active_sessions() {
    let (_home, store) = store(1).await;
    let mut first = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut first).await.expect("first session");
    let mut second = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);

    assert!(store.create(&mut second).await.is_err());
    assert!(store.load(&first.id).await.expect("load first").is_some());
}

#[tokio::test]
async fn expiry_cleanup_removes_sessions_and_announces_revocation() {
    let (_home, store) = store(2).await;
    let mut session = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut session).await.expect("session");
    let mut revocations = store.subscribe_revocations();

    store
        .delete_expired_at(OffsetDateTime::now_utc() + IDLE_EXPIRY + Duration::SECOND)
        .await
        .expect("delete expired");

    assert_eq!(
        revocations.try_recv().expect("revocation"),
        super::super::session::session_id_hash(session.id)
    );
    assert!(store.load(&session.id).await.expect("load").is_none());
}

#[tokio::test]
async fn deletion_is_targeted_and_announces_revocation() {
    let (_home, store) = store(2).await;
    let mut first = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    let mut second = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    store.create(&mut first).await.expect("first session");
    store.create(&mut second).await.expect("second session");
    let mut revocations = store.subscribe_revocations();

    store.delete(&first.id).await.expect("delete first");

    assert_eq!(
        revocations.try_recv().expect("revocation"),
        super::super::session::session_id_hash(first.id)
    );
    assert!(store.load(&first.id).await.expect("load first").is_none());
    assert!(store.load(&second.id).await.expect("load second").is_some());
}

#[tokio::test]
async fn persistent_store_loads_the_same_session_after_reconstruction() {
    let home = tempfile::tempdir().expect("session home");
    let database = home.path().join("db/noema.sqlite3");
    let mut saved = record(OffsetDateTime::now_utc() + IDLE_EXPIRY);
    saved.data.insert("authenticated".to_string(), true.into());
    {
        let noema = noema_store::NoemaStore::open(&noema_store::StoreConfig::new(&database))
            .await
            .expect("first store");
        BoundedSessionStore::persistent(noema)
            .create(&mut saved)
            .await
            .expect("create session");
    }

    let noema = noema_store::NoemaStore::open(&noema_store::StoreConfig::new(&database))
        .await
        .expect("restarted store");
    let loaded = BoundedSessionStore::persistent(noema)
        .load(&saved.id)
        .await
        .expect("load session")
        .expect("stored session");

    assert_eq!(loaded.id, saved.id);
    assert_eq!(loaded.data, saved.data);
}
