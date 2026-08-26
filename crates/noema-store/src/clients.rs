#![allow(missing_docs)]

use rusqlite::{OptionalExtension, TransactionBehavior, params};

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
    /// Insert one native client for tests.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage rejects the client.
    #[cfg(any(test, feature = "test-support"))]
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

    /// List all clients owned by one human.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot read the clients.
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

    /// Revoke one client and its dependent authority.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot commit the revocation.
    pub async fn revoke_client(
        &self,
        owner_human_id: &str,
        client_id: &str,
    ) -> Result<Option<ClientRecord>, StoreError> {
        let (client, newly_revoked) = self
            .with_connection(|conn| {
                let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let changed = transaction.execute(
                    "UPDATE clients SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE owner_human_id = ?1 AND client_id = ?2 AND revoked_at IS NULL",
                    params![owner_human_id, client_id],
                )?;
                if changed == 1 {
                    revoke_client_authority(&transaction, client_id, "client")?;
                }
                let client = load_client(&transaction, owner_human_id, client_id, false)?;
                transaction.commit()?;
                Ok((client, changed == 1))
            })
            .await?;
        if newly_revoked && client.is_some() {
            let _ = self.client_revocations.send(client_id.to_string());
        }
        Ok(client)
    }

    /// Revoke all active native clients owned by one human.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot commit every revocation.
    pub async fn revoke_all_native_clients(
        &self,
        owner_human_id: &str,
    ) -> Result<usize, StoreError> {
        let revoked = self
            .with_connection(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let client_ids = {
                    let mut statement = tx.prepare(
                        "SELECT client_id FROM clients WHERE owner_human_id = ?1 AND auth_kind = 'native_oauth' AND revoked_at IS NULL",
                    )?;
                    let rows = statement.query_map([owner_human_id], |row| row.get::<_, String>(0))?;
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                for client_id in &client_ids {
                    tx.execute(
                        "UPDATE clients SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND revoked_at IS NULL",
                        [client_id],
                    )?;
                    revoke_client_authority(&tx, client_id, "global")?;
                }
                tx.commit()?;
                Ok(client_ids)
            })
            .await?;
        for client_id in &revoked {
            let _ = self.client_revocations.send(client_id.clone());
        }
        Ok(revoked.len())
    }

    #[must_use]
    pub fn subscribe_client_revocations(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.client_revocations.subscribe()
    }
}

fn revoke_client_authority(
    conn: &rusqlite::Connection,
    client_id: &str,
    reason: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE native_oauth_families SET revoked_at = unixepoch(), revoke_reason = ?2 WHERE client_id = ?1 AND revoked_at IS NULL",
        params![client_id, reason],
    )?;
    conn.execute(
        "UPDATE native_oauth_access_tokens SET revoked_at = unixepoch() WHERE family_id IN (SELECT family_id FROM native_oauth_families WHERE client_id = ?1) AND revoked_at IS NULL",
        [client_id],
    )?;
    revoke_client_dependents(conn, client_id)
}

pub(super) fn revoke_client_dependents(
    conn: &rusqlite::Connection,
    client_id: &str,
) -> Result<(), StoreError> {
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

    #[tokio::test]
    async fn global_revocation_revokes_each_active_native_client() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");
        for (client_id, name) in [("client-one", "Phone"), ("client-two", "Desktop")] {
            store
                .insert_client(client_id, "human:local", name, [4_u8; 32])
                .await
                .expect("insert client");
        }
        let mut events = store.subscribe_client_revocations();

        assert_eq!(
            store
                .revoke_all_native_clients("human:local")
                .await
                .expect("revoke all"),
            2
        );
        let mut revoked_events = vec![
            events.recv().await.expect("first event"),
            events.recv().await.expect("second event"),
        ];
        revoked_events.sort();
        assert_eq!(revoked_events, ["client-one", "client-two"]);
        assert!(
            store
                .list_clients("human:local")
                .await
                .expect("list clients")
                .into_iter()
                .all(|client| client.revoked_at.is_some())
        );
    }

    #[tokio::test]
    async fn client_revocation_failure_rolls_back_and_sends_no_event() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");
        store
            .insert_client("client-one", "human:local", "Phone", [4_u8; 32])
            .await
            .expect("insert client");
        store
            .register_client_notifications("client-one", &[1, 2, 3], ApnsEnvironment::Development)
            .await
            .expect("register notifications");
        store
            .with_connection(|connection| {
                connection.execute_batch(
                    r#"
                    CREATE TEMP TRIGGER fail_client_dependent_revocation
                    BEFORE DELETE ON client_notification_registrations
                    BEGIN
                      SELECT RAISE(ABORT, 'forced client revocation failure');
                    END;
                    "#,
                )?;
                Ok(())
            })
            .await
            .expect("install failure trigger");
        let mut events = store.subscribe_client_revocations();

        let error = store
            .revoke_client("human:local", "client-one")
            .await
            .expect_err("dependent failure must abort revocation");

        assert!(matches!(error, StoreError::Sqlite(_)));
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        let client = store
            .list_clients("human:local")
            .await
            .expect("list clients")
            .pop()
            .expect("client remains");
        assert_eq!(client.revoked_at, None);
        assert!(
            store
                .client_notification_registration("client-one")
                .await
                .expect("registration lookup")
                .is_some()
        );
    }
}
