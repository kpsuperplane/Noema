//! Persistent passkey credentials for the built-in local human.

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{NoemaStore, StoreError};

const LOCAL_HUMAN_ID: &str = "human:local";

/// One stored passkey credential for the built-in local human.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanPasskeyRecord {
    /// Stable WebAuthn credential identifier in canonical base64url form.
    pub credential_id: String,
    /// Serialized credential state owned by the WebAuthn library.
    pub credential_json: String,
}

/// Result of an attempted local-human passkey removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanPasskeyRemoval {
    /// The selected passkey was removed.
    Removed {
        /// Browser sessions authenticated with the removed passkey.
        browser_sessions: Vec<[u8; 32]>,
    },
    /// The selected passkey does not exist.
    NotFound,
    /// The selected passkey is the final registered passkey.
    FinalPasskey,
}

impl NoemaStore {
    /// Return all passkeys registered to the built-in local human.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn local_human_passkeys(&self) -> Result<Vec<HumanPasskeyRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT credential_id, credential_json
                FROM human_passkeys
                WHERE human_id = ?1
                ORDER BY created_at, credential_id
                "#,
            )?;
            statement
                .query_map([LOCAL_HUMAN_ID], |row| {
                    Ok(HumanPasskeyRecord {
                        credential_id: row.get(0)?,
                        credential_json: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return one passkey by its stable credential identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn local_human_passkey(
        &self,
        credential_id: &str,
    ) -> Result<Option<HumanPasskeyRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT credential_id, credential_json
                FROM human_passkeys
                WHERE human_id = ?1 AND credential_id = ?2
                "#,
                params![LOCAL_HUMAN_ID, credential_id],
                |row| {
                    Ok(HumanPasskeyRecord {
                        credential_id: row.get(0)?,
                        credential_json: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return whether the built-in local human has any passkey.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn local_human_has_passkey(&self) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM human_passkeys WHERE human_id = ?1)",
                [LOCAL_HUMAN_ID],
                |row| row.get(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Persist one passkey without replacing an existing credential.
    ///
    /// Returns `false` when the credential identifier already exists.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn insert_local_human_passkey(
        &self,
        credential_id: &str,
        credential_json: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                INSERT INTO human_passkeys (credential_id, human_id, credential_json)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(credential_id) DO NOTHING
                "#,
                params![credential_id, LOCAL_HUMAN_ID, credential_json],
            )? == 1)
        })
        .await
    }

    /// Replace one credential only when its serialized snapshot still matches.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn compare_and_swap_local_human_passkey(
        &self,
        credential_id: &str,
        expected_json: &str,
        replacement_json: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                UPDATE human_passkeys
                SET credential_json = ?1,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE human_id = ?2 AND credential_id = ?3 AND credential_json = ?4
                "#,
                params![
                    replacement_json,
                    LOCAL_HUMAN_ID,
                    credential_id,
                    expected_json
                ],
            )? == 1)
        })
        .await
    }

    /// Remove one passkey while retaining at least one local-human passkey.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store transaction fails.
    pub async fn remove_local_human_passkey(
        &self,
        credential_id: &str,
    ) -> Result<HumanPasskeyRemoval, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let exists = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM human_passkeys WHERE human_id = ?1 AND credential_id = ?2)",
                params![LOCAL_HUMAN_ID, credential_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Ok(HumanPasskeyRemoval::NotFound);
            }
            let count = transaction.query_row(
                "SELECT COUNT(*) FROM human_passkeys WHERE human_id = ?1",
                [LOCAL_HUMAN_ID],
                |row| row.get::<_, u64>(0),
            )?;
            if count <= 1 {
                return Ok(HumanPasskeyRemoval::FinalPasskey);
            }
            transaction.execute(
                "DELETE FROM human_passkeys WHERE human_id = ?1 AND credential_id = ?2",
                params![LOCAL_HUMAN_ID, credential_id],
            )?;
            let browser_sessions =
                super::browser_sessions::delete_browser_sessions_for_passkey_in(
                    &transaction,
                    credential_id,
                )?;
            transaction.commit()?;
            Ok(HumanPasskeyRemoval::Removed { browser_sessions })
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
    async fn passkeys_are_additive_updates_are_targeted_and_final_removal_is_blocked() {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");

        assert!(
            store
                .insert_local_human_passkey("credential-one", r#"{"credential":"first"}"#)
                .await
                .expect("first insert")
        );
        assert!(
            store
                .insert_local_human_passkey("credential-two", r#"{"credential":"second"}"#)
                .await
                .expect("second insert")
        );
        assert!(
            !store
                .insert_local_human_passkey("credential-one", r#"{"credential":"duplicate"}"#)
                .await
                .expect("duplicate insert")
        );
        assert!(
            store
                .compare_and_swap_local_human_passkey(
                    "credential-one",
                    r#"{"credential":"first"}"#,
                    r#"{"credential":"updated"}"#,
                )
                .await
                .expect("current update")
        );

        let passkeys = store.local_human_passkeys().await.expect("passkeys");
        assert_eq!(passkeys.len(), 2);
        assert_eq!(passkeys[0].credential_json, r#"{"credential":"updated"}"#);
        assert_eq!(passkeys[1].credential_json, r#"{"credential":"second"}"#);

        for (credential_id, expected) in [
            ("missing", HumanPasskeyRemoval::NotFound),
            (
                "credential-one",
                HumanPasskeyRemoval::Removed {
                    browser_sessions: Vec::new(),
                },
            ),
            ("credential-two", HumanPasskeyRemoval::FinalPasskey),
        ] {
            assert_eq!(
                store
                    .remove_local_human_passkey(credential_id)
                    .await
                    .expect("passkey removal"),
                expected
            );
        }
    }
}
