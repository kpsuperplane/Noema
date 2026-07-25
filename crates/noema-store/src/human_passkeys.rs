//! Persistent passkey credential material for the built-in local human.

use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};

const LOCAL_HUMAN_ID: &str = "human:local";

impl NoemaStore {
    /// Return the serialized passkey registered to the built-in local human.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn local_human_passkey(&self) -> Result<Option<String>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT credential_json FROM human_passkeys WHERE human_id = ?1",
                [LOCAL_HUMAN_ID],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Persist the first passkey for the built-in local human atomically.
    ///
    /// Returns `false` when another enrollment already won the first-write race.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn insert_initial_local_human_passkey(
        &self,
        credential_json: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                INSERT INTO human_passkeys (human_id, credential_json)
                VALUES (?1, ?2)
                ON CONFLICT(human_id) DO NOTHING
                "#,
                params![LOCAL_HUMAN_ID, credential_json],
            )? == 1)
        })
        .await
    }

    /// Replace a passkey only when it still matches the authentication snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn compare_and_swap_local_human_passkey(
        &self,
        expected_json: &str,
        replacement_json: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                UPDATE human_passkeys
                SET credential_json = ?1,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE human_id = ?2 AND credential_json = ?3
                "#,
                params![replacement_json, LOCAL_HUMAN_ID, expected_json],
            )? == 1)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::StoreConfig;

    #[tokio::test]
    async fn initial_passkey_is_first_write_wins_and_updates_are_compare_and_swap() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");

        assert!(
            store
                .insert_initial_local_human_passkey(r#"{"credential":"first"}"#)
                .await
                .expect("first insert")
        );
        assert!(
            !store
                .insert_initial_local_human_passkey(r#"{"credential":"second"}"#)
                .await
                .expect("second insert")
        );
        assert!(
            store
                .compare_and_swap_local_human_passkey(
                    r#"{"credential":"first"}"#,
                    r#"{"credential":"updated"}"#,
                )
                .await
                .expect("current update")
        );
    }
}
