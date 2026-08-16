#![allow(missing_docs)]

use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientRecord {
    pub client_id: String,
    pub owner_human_id: String,
    pub display_name: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

impl NoemaStore {
    #[cfg(test)]
    pub async fn insert_client(
        &self,
        client_id: &str,
        owner_human_id: &str,
        display_name: &str,
        token_hash: [u8; 32],
    ) -> Result<ClientRecord, StoreError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash, auth_kind) VALUES (?1, ?2, ?3, ?4, 'native_oauth')",
                params![client_id, owner_human_id, display_name, token_hash.as_slice()],
            )?;
            load_client(connection, owner_human_id, client_id, false)?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "inserted test client could not be read".to_string(),
                }
            })
        })
        .await
    }

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
                        "UPDATE native_oauth_families SET revoked_at = unixepoch(), revoke_reason = 'client' WHERE client_id = ?1 AND revoked_at IS NULL",
                        [client_id],
                    )?;
                    conn.execute(
                        "UPDATE native_oauth_access_tokens SET revoked_at = unixepoch() WHERE family_id IN (SELECT family_id FROM native_oauth_families WHERE client_id = ?1) AND revoked_at IS NULL",
                        [client_id],
                    )?;
                    revoke_client_dependents(conn, client_id)?;
                }
                let client = load_client(conn, owner_human_id, client_id, false)?;
                Ok((client, changed == 1))
            })
            .await?;
        if newly_revoked && client.is_some() {
            let _ = self.client_revocations.send(client_id.to_string());
        }
        Ok(client)
    }

    #[must_use]
    pub fn subscribe_client_revocations(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.client_revocations.subscribe()
    }
}

pub(super) fn revoke_client_dependents(
    conn: &rusqlite::Connection,
    client_id: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'client_revoked', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND event <> 'end' AND status = 'pending'",
        [client_id],
    )?;
    conn.execute(
        "UPDATE apns_deliveries SET status = 'failed', last_error_code = 'client_revoked', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND status = 'pending'",
        [client_id],
    )?;
    conn.execute(
        "DELETE FROM client_notification_registrations WHERE client_id = ?1",
        [client_id],
    )?;
    conn.execute(
        "DELETE FROM client_live_activity_registrations WHERE client_id = ?1",
        [client_id],
    )?;
    conn.execute(
        "DELETE FROM client_task_activities WHERE client_id = ?1",
        [client_id],
    )?;
    conn.execute(
        "DELETE FROM live_activity_deliveries WHERE client_id = ?1 AND event <> 'end'",
        [client_id],
    )?;
    Ok(())
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
) -> Result<Option<ClientRecord>, StoreError> {
    let query = if active_only {
        "SELECT client_id, owner_human_id, display_name, created_at, revoked_at FROM clients WHERE owner_human_id = ?1 AND client_id = ?2 AND revoked_at IS NULL"
    } else {
        "SELECT client_id, owner_human_id, display_name, created_at, revoked_at FROM clients WHERE owner_human_id = ?1 AND client_id = ?2"
    };
    conn.query_row(query, params![owner_human_id, client_id], |row| {
        Ok(ClientRecord {
            client_id: row.get(0)?,
            owner_human_id: row.get(1)?,
            display_name: row.get(2)?,
            created_at: row.get(3)?,
            revoked_at: row.get(4)?,
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
    async fn client_revocation_preserves_rows_and_removes_dependents() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");
        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash, auth_kind) VALUES ('client-one', 'human:local', 'Phone', zeroblob(32), 'native_oauth')",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("insert client");
        store
            .register_client_notifications("client-one", &[1, 2, 3], ApnsEnvironment::Development)
            .await
            .expect("register client notifications");
        let mut events = store.subscribe_client_revocations();
        let revoked = store
            .revoke_client("human:local", "client-one")
            .await
            .expect("revoke")
            .expect("client exists");
        assert!(revoked.revoked_at.is_some());
        assert_eq!(events.recv().await.expect("revocation event"), "client-one");
        assert!(
            store
                .client_notification_registration("client-one")
                .await
                .expect("notification registration lookup")
                .is_none()
        );
        assert_eq!(
            store.list_clients("human:local").await.expect("list").len(),
            1
        );
    }
}
