//! Bounded storage for private browser sessions.

use std::fmt;

use time::{Duration, OffsetDateTime};
use tokio::sync::broadcast;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store,
};

pub(super) const IDLE_EXPIRY: Duration = Duration::hours(24);
const ABSOLUTE_EXPIRY: Duration = Duration::days(30);
const DEFAULT_MAX_SESSIONS: usize = 1_024;

#[derive(Clone)]
pub(super) struct BoundedSessionStore {
    store: noema_store::NoemaStore,
    max_sessions: usize,
    revocations: broadcast::Sender<[u8; 32]>,
}

impl fmt::Debug for BoundedSessionStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedSessionStore")
            .field("max_sessions", &self.max_sessions)
            .finish_non_exhaustive()
    }
}

impl BoundedSessionStore {
    pub(super) fn persistent(store: noema_store::NoemaStore) -> Self {
        Self::new(store, DEFAULT_MAX_SESSIONS)
    }

    fn new(store: noema_store::NoemaStore, max_sessions: usize) -> Self {
        let (revocations, _) = broadcast::channel(max_sessions.max(1));
        Self {
            store,
            max_sessions,
            revocations,
        }
    }

    pub(super) fn subscribe_revocations(&self) -> broadcast::Receiver<[u8; 32]> {
        self.revocations.subscribe()
    }

    pub(super) async fn delete_expired(&self) -> session_store::Result<()> {
        self.delete_expired_at(OffsetDateTime::now_utc()).await
    }

    async fn delete_expired_at(&self, now: OffsetDateTime) -> session_store::Result<()> {
        let revoked = self
            .store
            .delete_expired_browser_sessions(now.unix_timestamp())
            .await
            .map_err(store_error)?;
        self.announce(revoked);
        Ok(())
    }

    pub(super) async fn revoke_all(&self) -> session_store::Result<()> {
        let revoked = self
            .store
            .delete_all_browser_sessions()
            .await
            .map_err(store_error)?;
        self.announce(revoked);
        Ok(())
    }

    pub(super) fn announce_passkey_revocations(&self, revoked: Vec<[u8; 32]>) {
        self.announce(revoked);
    }

    fn announce(&self, revoked: Vec<[u8; 32]>) {
        for hash in revoked {
            let _ = self.revocations.send(hash);
        }
    }
}

#[async_trait::async_trait]
impl SessionStore for BoundedSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let now = OffsetDateTime::now_utc();
        loop {
            let capped = capped_record(record, now);
            let stored = noema_store::BrowserSessionRecord {
                data_json: serde_json::to_string(&capped.data).map_err(store_error)?,
                created_at: now.unix_timestamp(),
                expires_at: capped.expiry_date.unix_timestamp(),
            };
            let hash = super::session::session_id_hash(record.id);
            match self
                .store
                .insert_browser_session(hash, &stored, self.max_sessions, now.unix_timestamp())
                .await
                .map_err(store_error)?
            {
                noema_store::BrowserSessionInsert::Inserted(expired) => {
                    self.announce(expired);
                    return Ok(());
                }
                noema_store::BrowserSessionInsert::Collision(expired) => {
                    self.announce(expired);
                    record.id = Id::default();
                }
                noema_store::BrowserSessionInsert::Full(expired) => {
                    self.announce(expired);
                    return Err(capacity_error());
                }
            }
        }
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = serde_json::to_string(&record.data).map_err(store_error)?;
        self.store
            .save_browser_session(
                super::session::session_id_hash(record.id),
                &data,
                record.expiry_date.unix_timestamp(),
            )
            .await
            .map_err(store_error)?
            .then_some(())
            .ok_or_else(unavailable_error)
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        self.store
            .load_browser_session(
                super::session::session_id_hash(*session_id),
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .map_err(store_error)?
            .map(|stored| {
                Ok(Record {
                    id: *session_id,
                    data: serde_json::from_str(&stored.data_json).map_err(store_error)?,
                    expiry_date: OffsetDateTime::from_unix_timestamp(stored.expires_at)
                        .map_err(store_error)?,
                })
            })
            .transpose()
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let hash = super::session::session_id_hash(*session_id);
        if self
            .store
            .delete_browser_session(hash)
            .await
            .map_err(store_error)?
        {
            let _ = self.revocations.send(hash);
        }
        Ok(())
    }
}

fn capped_record(record: &Record, created_at: OffsetDateTime) -> Record {
    let mut record = record.clone();
    record.expiry_date = record.expiry_date.min(created_at + ABSOLUTE_EXPIRY);
    record
}

fn capacity_error() -> session_store::Error {
    session_store::Error::Backend("browser session capacity reached".to_string())
}

fn unavailable_error() -> session_store::Error {
    session_store::Error::Backend("browser session is unavailable".to_string())
}

fn store_error(error: impl std::fmt::Display) -> session_store::Error {
    session_store::Error::Backend(format!("browser session storage failed: {error}"))
}

#[cfg(test)]
#[path = "session_store/tests.rs"]
mod tests;
