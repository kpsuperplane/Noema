#![allow(missing_docs)]

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{NoemaStore, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserSessionRecord {
    pub data_json: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserSessionInsert {
    Inserted(Vec<[u8; 32]>),
    Collision(Vec<[u8; 32]>),
    Full(Vec<[u8; 32]>),
}

impl NoemaStore {
    /// Inserts a browser session after deleting expired sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if validation or database access fails.
    pub async fn insert_browser_session(
        &self,
        session_hash: [u8; 32],
        record: &BrowserSessionRecord,
        max_sessions: usize,
        now: i64,
    ) -> Result<BrowserSessionInsert, StoreError> {
        validate_record(record)?;
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let expired = delete_expired(&tx, now)?;
            let exists = tx
                .query_row(
                    "SELECT 1 FROM browser_sessions WHERE session_hash = ?1",
                    [session_hash.as_slice()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            let outcome = if exists {
                BrowserSessionInsert::Collision(expired)
            } else {
                let count = tx.query_row("SELECT count(*) FROM browser_sessions", [], |row| {
                    row.get::<_, usize>(0)
                })?;
                if count >= max_sessions {
                    BrowserSessionInsert::Full(expired)
                } else {
                    tx.execute(
                        "INSERT INTO browser_sessions (session_hash, data_json, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                        params![session_hash.as_slice(), record.data_json, record.created_at, record.expires_at],
                    )?;
                    BrowserSessionInsert::Inserted(expired)
                }
            };
            tx.commit()?;
            Ok(outcome)
        })
        .await
    }

    /// Updates an existing browser session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session data is invalid or database access fails.
    pub async fn save_browser_session(
        &self,
        session_hash: [u8; 32],
        data_json: &str,
        expires_at: i64,
    ) -> Result<bool, StoreError> {
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(data_json)?;
        self.with_connection(|conn| {
            let changed = conn.execute(
                "UPDATE browser_sessions SET data_json = ?2, expires_at = min(?3, created_at + ?4) WHERE session_hash = ?1 AND ?3 > created_at",
                params![session_hash.as_slice(), data_json, expires_at, 30 * 24 * 60 * 60],
            )?;
            Ok(changed == 1)
        })
        .await
    }

    /// Loads an active browser session.
    ///
    /// # Errors
    ///
    /// Returns an error if database access fails.
    pub async fn load_browser_session(
        &self,
        session_hash: [u8; 32],
        now: i64,
    ) -> Result<Option<BrowserSessionRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT data_json, created_at, expires_at FROM browser_sessions WHERE session_hash = ?1 AND expires_at > ?2",
                params![session_hash.as_slice(), now],
                |row| {
                    Ok(BrowserSessionRecord {
                        data_json: row.get(0)?,
                        created_at: row.get(1)?,
                        expires_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Deletes one browser session.
    ///
    /// # Errors
    ///
    /// Returns an error if database access fails.
    pub async fn delete_browser_session(&self, session_hash: [u8; 32]) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM browser_sessions WHERE session_hash = ?1",
                [session_hash.as_slice()],
            )
            .map(|changed| changed == 1)
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Deletes all expired browser sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if database access fails.
    pub async fn delete_expired_browser_sessions(
        &self,
        now: i64,
    ) -> Result<Vec<[u8; 32]>, StoreError> {
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let expired = delete_expired(&tx, now)?;
            tx.commit()?;
            Ok(expired)
        })
        .await
    }

    /// Deletes all browser sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if database access fails.
    pub async fn delete_all_browser_sessions(&self) -> Result<Vec<[u8; 32]>, StoreError> {
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let hashes = session_hashes(&tx, "SELECT session_hash FROM browser_sessions", [])?;
            tx.execute("DELETE FROM browser_sessions", [])?;
            tx.commit()?;
            Ok(hashes)
        })
        .await
    }
}

pub(crate) fn delete_browser_sessions_for_passkey_in(
    tx: &Transaction<'_>,
    credential_id: &str,
) -> Result<Vec<[u8; 32]>, StoreError> {
    let hashes = session_hashes(
        tx,
        "SELECT session_hash FROM browser_sessions WHERE json_extract(data_json, '$.passkey_id') = ?1",
        [credential_id],
    )?;
    tx.execute(
        "DELETE FROM browser_sessions WHERE json_extract(data_json, '$.passkey_id') = ?1",
        [credential_id],
    )?;
    Ok(hashes)
}

fn validate_record(record: &BrowserSessionRecord) -> Result<(), StoreError> {
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&record.data_json)?;
    if record.expires_at <= record.created_at {
        return Err(StoreError::InvariantViolation {
            message: "browser session expiry must follow creation".to_string(),
        });
    }
    Ok(())
}

fn delete_expired(tx: &Transaction<'_>, now: i64) -> Result<Vec<[u8; 32]>, StoreError> {
    let hashes = session_hashes(
        tx,
        "SELECT session_hash FROM browser_sessions WHERE expires_at <= ?1",
        [now],
    )?;
    tx.execute("DELETE FROM browser_sessions WHERE expires_at <= ?1", [now])?;
    Ok(hashes)
}

fn session_hashes<P>(
    tx: &Transaction<'_>,
    sql: &str,
    params: P,
) -> Result<Vec<[u8; 32]>, StoreError>
where
    P: rusqlite::Params,
{
    let mut statement = tx.prepare(sql)?;
    let rows = statement.query_map(params, |row| row.get::<_, Vec<u8>>(0))?;
    rows.map(|row| {
        let value = row?;
        value
            .try_into()
            .map_err(|_| StoreError::InvariantViolation {
                message: "stored browser session digest has an invalid length".to_string(),
            })
    })
    .collect()
}
