//! Durable local-human client credentials.

use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};

const LOCAL_HUMAN_ID: &str = "human:local";

/// Public client metadata safe to expose to authenticated settings clients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientRecord {
    /// Opaque client identifier carried in the bearer credential.
    pub client_id: String,
    /// Owning local-human identity.
    pub owner_human_id: String,
    /// Human-visible client label.
    pub display_name: String,
    /// Creation timestamp serialized by SQLite.
    pub created_at: String,
    /// Revocation timestamp, when the credential has been revoked.
    pub revoked_at: Option<String>,
}

#[derive(Clone, Debug)]
struct ClientRow {
    record: ClientRecord,
    token_hash: Option<[u8; 32]>,
}

impl NoemaStore {
    /// Insert one client credential while retaining only its SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns a store error when the values violate their bounds or SQLite
    /// cannot persist and reload the credential metadata.
    pub async fn insert_client(
        &self,
        client_id: &str,
        owner_human_id: &str,
        display_name: &str,
        token_hash: [u8; 32],
    ) -> Result<ClientRecord, StoreError> {
        if client_id.trim() != client_id
            || client_id.is_empty()
            || client_id.len() > 128
            || owner_human_id != LOCAL_HUMAN_ID
            || display_name.trim() != display_name
            || display_name.is_empty()
            || display_name.len() > 128
        {
            return Err(StoreError::InvariantViolation {
                message: "client identity or display name is outside its bounds".to_string(),
            });
        }
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash) VALUES (?1, ?2, ?3, ?4)",
                params![client_id, owner_human_id, display_name, token_hash.as_slice()],
            )?;
            load_client(conn, owner_human_id, client_id, false)
                .and_then(|row| row.map(|row| row.record).ok_or_else(|| StoreError::InvariantViolation {
                    message: "inserted client could not be read".to_string(),
                }))
        })
        .await
    }

    /// List all client metadata for one local human, retaining revoked rows.
    ///
    /// # Errors
    ///
    /// Returns a store error when SQLite cannot read the client rows.
    pub async fn list_clients(
        &self,
        owner_human_id: &str,
    ) -> Result<Vec<ClientRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT client_id, owner_human_id, display_name, created_at, revoked_at FROM clients WHERE owner_human_id = ?1 ORDER BY created_at, client_id",
            )?;
            let rows = statement.query_map([owner_human_id], client_from_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return an active client's digest for bearer validation.
    ///
    /// # Errors
    ///
    /// Returns a store error when SQLite cannot read the credential row.
    pub async fn active_client_token_hash(
        &self,
        client_id: &str,
    ) -> Result<Option<[u8; 32]>, StoreError> {
        self.with_connection(|conn| {
            load_client(conn, LOCAL_HUMAN_ID, client_id, true)
                .map(|row| row.and_then(|row| row.token_hash))
        })
        .await
    }

    /// Revoke one client without deleting its audit-visible metadata.
    ///
    /// # Errors
    ///
    /// Returns a store error when SQLite cannot update or reload the client.
    pub async fn revoke_client(
        &self,
        owner_human_id: &str,
        client_id: &str,
    ) -> Result<Option<ClientRecord>, StoreError> {
        let (client, newly_revoked) = self
            .with_connection(|conn| {
                let changed = conn.execute(
                    "UPDATE clients SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE owner_human_id = ?1 AND client_id = ?2 AND revoked_at IS NULL",
                    params![owner_human_id, client_id],
                )?;
                if changed == 1 {
                    conn.execute(
                        "DELETE FROM client_notification_registrations WHERE client_id = ?1",
                        [client_id],
                    )?;
                }
                let client = load_client(conn, owner_human_id, client_id, false)?
                    .map(|row| row.record);
                Ok((client, changed == 1))
            })
            .await?;
        if newly_revoked && client.is_some() {
            let _ = self.client_revocations.send(client_id.to_string());
        }
        Ok(client)
    }

    /// Subscribe to revocations so active client sockets can fail closed.
    #[must_use]
    pub fn subscribe_client_revocations(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.client_revocations.subscribe()
    }
}

fn client_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClientRecord> {
    Ok(ClientRecord {
        client_id: row.get(0)?,
        owner_human_id: row.get(1)?,
        display_name: row.get(2)?,
        created_at: row.get(3)?,
        revoked_at: row.get(4)?,
    })
}

fn load_client(
    conn: &rusqlite::Connection,
    owner_human_id: &str,
    client_id: &str,
    active_only: bool,
) -> Result<Option<ClientRow>, StoreError> {
    let query = if active_only {
        "SELECT client_id, owner_human_id, display_name, created_at, revoked_at, token_hash FROM clients WHERE owner_human_id = ?1 AND client_id = ?2 AND revoked_at IS NULL"
    } else {
        "SELECT client_id, owner_human_id, display_name, created_at, revoked_at, token_hash FROM clients WHERE owner_human_id = ?1 AND client_id = ?2"
    };
    conn.query_row(query, params![owner_human_id, client_id], |row| {
        let hash = row.get_ref(5)?.as_blob().ok().and_then(|bytes| {
            (bytes.len() == 32).then(|| {
                let mut hash = [0_u8; 32];
                hash.copy_from_slice(bytes);
                hash
            })
        });
        Ok(ClientRow {
            record: ClientRecord {
                client_id: row.get(0)?,
                owner_human_id: row.get(1)?,
                display_name: row.get(2)?,
                created_at: row.get(3)?,
                revoked_at: row.get(4)?,
            },
            token_hash: hash,
        })
    })
    .optional()
    .map_err(StoreError::Sqlite)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::{ApnsEnvironment, StoreConfig};

    #[tokio::test]
    async fn client_credentials_are_hash_only_and_revocation_preserves_rows() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");
        let hash = [7_u8; 32];
        store
            .insert_client("client-one", LOCAL_HUMAN_ID, "Phone", hash)
            .await
            .expect("insert client");
        store
            .register_client_notifications("client-one", &[1, 2, 3], ApnsEnvironment::Development)
            .await
            .expect("register client notifications");
        store
            .with_connection(|conn| {
                let stored = conn.query_row(
                    "SELECT token_hash FROM clients WHERE client_id = 'client-one'",
                    [],
                    |row| row.get::<_, Vec<u8>>(0),
                )?;
                assert_eq!(stored, hash);
                Ok(())
            })
            .await
            .expect("inspect digest");
        let mut events = store.subscribe_client_revocations();
        let revoked = store
            .revoke_client(LOCAL_HUMAN_ID, "client-one")
            .await
            .expect("revoke")
            .expect("client exists");
        assert!(revoked.revoked_at.is_some());
        assert_eq!(events.recv().await.expect("revocation event"), "client-one");
        assert!(
            store
                .active_client_token_hash("client-one")
                .await
                .expect("lookup")
                .is_none()
        );
        assert!(
            store
                .client_notification_registration("client-one")
                .await
                .expect("notification registration lookup")
                .is_none()
        );
        assert_eq!(
            store
                .list_clients(LOCAL_HUMAN_ID)
                .await
                .expect("list")
                .len(),
            1
        );
    }
}
