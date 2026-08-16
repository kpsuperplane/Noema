//! Bounded process-local storage for private browser sessions.

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

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
    inner: Arc<Mutex<HashMap<Id, StoredSession>>>,
    max_sessions: usize,
    revocations: broadcast::Sender<Id>,
}

#[derive(Clone)]
struct StoredSession {
    record: Record,
    created_at: OffsetDateTime,
}

impl fmt::Debug for BoundedSessionStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedSessionStore")
            .field("max_sessions", &self.max_sessions)
            .finish_non_exhaustive()
    }
}

impl Default for BoundedSessionStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SESSIONS)
    }
}

impl BoundedSessionStore {
    fn new(max_sessions: usize) -> Self {
        let (revocations, _) = broadcast::channel(max_sessions.max(1));
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_sessions,
            revocations,
        }
    }

    pub(super) fn subscribe_revocations(&self) -> broadcast::Receiver<Id> {
        self.revocations.subscribe()
    }

    pub(super) fn delete_expired(&self) {
        let revoked =
            self.with_entries(|entries| remove_expired(entries, OffsetDateTime::now_utc()));
        self.announce(revoked);
    }

    fn with_entries<T>(&self, action: impl FnOnce(&mut HashMap<Id, StoredSession>) -> T) -> T {
        let mut entries = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        action(&mut entries)
    }

    fn announce(&self, revoked: Vec<Id>) {
        for id in revoked {
            let _ = self.revocations.send(id);
        }
    }
}

#[async_trait::async_trait]
impl SessionStore for BoundedSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let now = OffsetDateTime::now_utc();
        let (result, revoked) = self.with_entries(|entries| {
            let revoked = remove_expired(entries, now);
            if entries.len() >= self.max_sessions {
                return (Err(capacity_error()), revoked);
            }
            while entries.contains_key(&record.id) {
                record.id = Id::default();
            }
            entries.insert(
                record.id,
                StoredSession {
                    record: capped_record(record, now),
                    created_at: now,
                },
            );
            (Ok(()), revoked)
        });
        self.announce(revoked);
        result
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let now = OffsetDateTime::now_utc();
        let (result, revoked) = self.with_entries(|entries| {
            let revoked = remove_expired(entries, now);
            let result = match entries.get_mut(&record.id) {
                Some(stored) => {
                    stored.record = capped_record(record, stored.created_at);
                    Ok(())
                }
                None => Err(session_store::Error::Backend(
                    "browser session is unavailable".to_string(),
                )),
            };
            (result, revoked)
        });
        self.announce(revoked);
        result
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let (record, revoked) = self.with_entries(|entries| {
            let revoked = remove_expired(entries, OffsetDateTime::now_utc());
            (
                entries.get(session_id).map(|stored| stored.record.clone()),
                revoked,
            )
        });
        self.announce(revoked);
        Ok(record)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let removed = self.with_entries(|entries| entries.remove(session_id).is_some());
        if removed {
            let _ = self.revocations.send(*session_id);
        }
        Ok(())
    }
}

fn capped_record(record: &Record, created_at: OffsetDateTime) -> Record {
    let mut record = record.clone();
    record.expiry_date = record.expiry_date.min(created_at + ABSOLUTE_EXPIRY);
    record
}

fn remove_expired(entries: &mut HashMap<Id, StoredSession>, now: OffsetDateTime) -> Vec<Id> {
    let expired = entries
        .iter()
        .filter_map(|(id, stored)| {
            (stored.record.expiry_date <= now || stored.created_at + ABSOLUTE_EXPIRY <= now)
                .then_some(*id)
        })
        .collect::<Vec<_>>();
    for id in &expired {
        entries.remove(id);
    }
    expired
}

fn capacity_error() -> session_store::Error {
    session_store::Error::Backend("browser session capacity reached".to_string())
}

#[cfg(test)]
#[path = "session_store/tests.rs"]
mod tests;
