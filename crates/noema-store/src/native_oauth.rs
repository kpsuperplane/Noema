#![allow(missing_docs)]

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{NoemaStore, StoreError, clients::revoke_client_dependents};

const LOCAL_HUMAN_ID: &str = "human:local";
const OAUTH_SCOPE: &str = "noema";
const OAUTH_AUDIENCE: &str = "noema";
pub const NATIVE_OAUTH_RETRY_SECONDS: i64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOAuthGrant {
    pub client_id: String,
    pub owner_human_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub pkce_value: Option<String>,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNativeOAuthCode<'a> {
    pub client_id: &'a str,
    pub display_name: &'a str,
    pub redirect_uri: &'a str,
    pub pkce_value: &'a str,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNativeOAuthFamily<'a> {
    pub family_id: &'a str,
    pub client_id: &'a str,
    pub access_hash: [u8; 32],
    pub refresh_hash: [u8; 32],
    pub issued_at: i64,
    pub access_expires_at: i64,
    pub idle_expires_at: i64,
    pub absolute_expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOAuthRefreshGrant {
    pub family_id: String,
    pub client_id: String,
    pub owner_human_id: String,
    pub scope: String,
    pub absolute_expires_at: i64,
    pub sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeOAuthRefreshLookup {
    Active(NativeOAuthRefreshGrant),
    Retryable { access_expires_at: i64 },
    Invalid,
    ReplayRevoked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOAuthRetryProof {
    pub access_hash: [u8; 32],
    pub refresh_hash: [u8; 32],
    pub issued_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOAuthRotation {
    pub access_hash: [u8; 32],
    pub refresh_hash: [u8; 32],
    pub issued_at: i64,
    pub access_expires_at: i64,
    pub idle_expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOAuthAccess {
    pub client_id: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeOAuthRotationOutcome {
    Rotated,
    Invalid,
    ReplayRevoked,
}

impl NoemaStore {
    /// Store one short-lived native authorization code.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if the code is invalid or storage fails.
    pub async fn insert_native_oauth_code(
        &self,
        code_hash: [u8; 32],
        code: NewNativeOAuthCode<'_>,
        now: i64,
    ) -> Result<(), StoreError> {
        validate_code(&code, now)?;
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "DELETE FROM native_oauth_codes WHERE expires_at <= ?1",
                [now],
            )?;
            let existing = tx
                .query_row(
                    "SELECT auth_kind, revoked_at FROM clients WHERE client_id = ?1",
                    [code.client_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?;
            match existing {
                None => {
                    tx.execute(
                        "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash, auth_kind) VALUES (?1, ?2, ?3, ?4, 'native_oauth')",
                        params![code.client_id, LOCAL_HUMAN_ID, code.display_name, [0_u8; 32].as_slice()],
                    )?;
                }
                Some((kind, None)) if kind == "native_oauth" => {}
                Some(_) => return Err(StoreError::InvariantViolation {
                    message: "native OAuth client identity is unavailable".to_string(),
                }),
            }
            tx.execute(
                "INSERT INTO native_oauth_codes (code_hash, client_id, owner_human_id, redirect_uri, scope, pkce_value, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![code_hash.as_slice(), code.client_id, LOCAL_HUMAN_ID, code.redirect_uri, OAUTH_SCOPE, code.pkce_value, code.expires_at, now],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Consume one native authorization code atomically.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot complete the transaction.
    pub async fn consume_native_oauth_code(
        &self,
        code_hash: [u8; 32],
        now: i64,
    ) -> Result<Option<NativeOAuthGrant>, StoreError> {
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let grant = tx
                .query_row(
                    "SELECT client_id, owner_human_id, redirect_uri, scope, pkce_value, expires_at FROM native_oauth_codes WHERE code_hash = ?1",
                    [code_hash.as_slice()],
                    |row| {
                        Ok(NativeOAuthGrant {
                            client_id: row.get(0)?,
                            owner_human_id: row.get(1)?,
                            redirect_uri: row.get(2)?,
                            scope: row.get(3)?,
                            pkce_value: Some(row.get(4)?),
                            expires_at: row.get(5)?,
                        })
                    },
                )
                .optional()?;
            tx.execute(
                "DELETE FROM native_oauth_codes WHERE code_hash = ?1",
                [code_hash.as_slice()],
            )?;
            tx.commit()?;
            Ok(grant.filter(|grant| grant.expires_at > now))
        })
        .await
    }

    /// Store one native refresh family and its first credentials.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if the family is invalid or storage fails.
    pub async fn insert_native_oauth_family(
        &self,
        family: NewNativeOAuthFamily<'_>,
    ) -> Result<(), StoreError> {
        validate_family(&family)?;
        self.with_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let active = tx
                .query_row(
                    "SELECT 1 FROM clients WHERE client_id = ?1 AND owner_human_id = ?2 AND auth_kind = 'native_oauth' AND revoked_at IS NULL",
                    params![family.client_id, LOCAL_HUMAN_ID],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !active {
                return Err(StoreError::InvariantViolation {
                    message: "native OAuth client is not active".to_string(),
                });
            }
            tx.execute(
                "INSERT INTO native_oauth_families (family_id, client_id, owner_human_id, audience, scope, created_at, last_used_at, idle_expires_at, absolute_expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8)",
                params![family.family_id, family.client_id, LOCAL_HUMAN_ID, OAUTH_AUDIENCE, OAUTH_SCOPE, family.issued_at, family.idle_expires_at, family.absolute_expires_at],
            )?;
            tx.execute(
                "INSERT INTO native_oauth_refresh_tokens (token_hash, family_id, sequence, status, issued_at) VALUES (?1, ?2, 0, 'active', ?3)",
                params![family.refresh_hash.as_slice(), family.family_id, family.issued_at],
            )?;
            tx.execute(
                "INSERT INTO native_oauth_access_tokens (token_hash, family_id, issued_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                params![family.access_hash.as_slice(), family.family_id, family.issued_at, family.access_expires_at],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Read an active refresh grant and revoke detected replay.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot complete the lookup.
    pub async fn native_oauth_refresh_grant(
        &self,
        refresh_hash: [u8; 32],
        retry: Option<NativeOAuthRetryProof>,
        now: i64,
    ) -> Result<NativeOAuthRefreshLookup, StoreError> {
        let (outcome, revoked_client) = self
            .with_connection(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let row = load_refresh(&tx, refresh_hash)?;
                let Some(row) = row else {
                    tx.commit()?;
                    return Ok((NativeOAuthRefreshLookup::Invalid, None));
                };
                if row.revoked_at.is_some()
                    || row.idle_expires_at <= now
                    || row.absolute_expires_at <= now
                {
                    if row.revoked_at.is_none() {
                        revoke_family(&tx, &row.family_id, &row.client_id, now, "expired")?;
                    }
                    tx.commit()?;
                    return Ok((NativeOAuthRefreshLookup::Invalid, Some(row.client_id)));
                }
                if row.status == "used" {
                    if let Some(access_expires_at) = match retry.as_ref() {
                        Some(proof) => retryable_access_expiry(&tx, &row, proof, now)?,
                        None => None,
                    } {
                        tx.commit()?;
                        return Ok((
                            NativeOAuthRefreshLookup::Retryable { access_expires_at },
                            None,
                        ));
                    }
                    revoke_family(&tx, &row.family_id, &row.client_id, now, "replay")?;
                    tx.commit()?;
                    return Ok((NativeOAuthRefreshLookup::ReplayRevoked, Some(row.client_id)));
                }
                let grant = NativeOAuthRefreshGrant {
                    family_id: row.family_id,
                    client_id: row.client_id,
                    owner_human_id: row.owner_human_id,
                    scope: row.scope,
                    absolute_expires_at: row.absolute_expires_at,
                    sequence: row.sequence,
                };
                tx.commit()?;
                Ok((NativeOAuthRefreshLookup::Active(grant), None))
            })
            .await?;
        if let Some(client_id) = revoked_client {
            let _ = self.client_revocations.send(client_id);
        }
        Ok(outcome)
    }

    /// Rotate one refresh credential and its access credential atomically.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if validation or storage fails.
    pub async fn rotate_native_oauth_refresh(
        &self,
        refresh_hash: [u8; 32],
        rotation: NativeOAuthRotation,
        now: i64,
    ) -> Result<NativeOAuthRotationOutcome, StoreError> {
        let (outcome, revoked_client) = self
            .with_connection(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let Some(row) = load_refresh(&tx, refresh_hash)? else {
                    tx.commit()?;
                    return Ok((NativeOAuthRotationOutcome::Invalid, None));
                };
                if row.status != "active" {
                    revoke_family(&tx, &row.family_id, &row.client_id, now, "replay")?;
                    tx.commit()?;
                    return Ok((NativeOAuthRotationOutcome::ReplayRevoked, Some(row.client_id)));
                }
                if row.revoked_at.is_some()
                    || row.idle_expires_at <= now
                    || row.absolute_expires_at <= now
                {
                    if row.revoked_at.is_none() {
                        revoke_family(&tx, &row.family_id, &row.client_id, now, "expired")?;
                    }
                    tx.commit()?;
                    return Ok((NativeOAuthRotationOutcome::Invalid, Some(row.client_id)));
                }
                let changed = tx.execute(
                    "UPDATE native_oauth_refresh_tokens SET status = 'used', used_at = ?2 WHERE token_hash = ?1 AND status = 'active'",
                    params![refresh_hash.as_slice(), now],
                )?;
                if changed != 1 {
                    revoke_family(&tx, &row.family_id, &row.client_id, now, "replay")?;
                    tx.commit()?;
                    return Ok((NativeOAuthRotationOutcome::ReplayRevoked, Some(row.client_id)));
                }
                let idle_expires_at = rotation.idle_expires_at.min(row.absolute_expires_at);
                tx.execute(
                    "INSERT INTO native_oauth_refresh_tokens (token_hash, family_id, sequence, status, issued_at) VALUES (?1, ?2, ?3, 'active', ?4)",
                    params![rotation.refresh_hash.as_slice(), row.family_id, row.sequence + 1, rotation.issued_at],
                )?;
                tx.execute(
                    "INSERT INTO native_oauth_access_tokens (token_hash, family_id, issued_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                    params![rotation.access_hash.as_slice(), row.family_id, rotation.issued_at, rotation.access_expires_at],
                )?;
                tx.execute(
                    "UPDATE native_oauth_families SET last_used_at = ?2, idle_expires_at = ?3 WHERE family_id = ?1",
                    params![row.family_id, now, idle_expires_at],
                )?;
                tx.commit()?;
                Ok((NativeOAuthRotationOutcome::Rotated, None))
            })
            .await?;
        if let Some(client_id) = revoked_client {
            let _ = self.client_revocations.send(client_id);
        }
        Ok(outcome)
    }

    /// Resolve one active native access credential.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot complete the lookup.
    pub async fn active_native_oauth_client(
        &self,
        access_hash: [u8; 32],
        now: i64,
    ) -> Result<Option<NativeOAuthAccess>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT family.client_id, min(access.expires_at, family.idle_expires_at, family.absolute_expires_at) FROM native_oauth_access_tokens AS access JOIN native_oauth_families AS family ON family.family_id = access.family_id JOIN clients ON clients.client_id = family.client_id WHERE access.token_hash = ?1 AND access.revoked_at IS NULL AND access.expires_at > ?2 AND family.revoked_at IS NULL AND family.idle_expires_at > ?2 AND family.absolute_expires_at > ?2 AND clients.revoked_at IS NULL AND clients.auth_kind = 'native_oauth'",
                params![access_hash.as_slice(), now],
                |row| {
                    Ok(NativeOAuthAccess {
                        client_id: row.get(0)?,
                        expires_at: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Revoke every native refresh family that has expired.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot commit all revocations.
    pub async fn expire_native_oauth_families(&self, now: i64) -> Result<usize, StoreError> {
        let revoked_clients = self
            .with_connection(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let expired = {
                    let mut statement = tx.prepare(
                        "SELECT family_id, client_id FROM native_oauth_families WHERE revoked_at IS NULL AND (idle_expires_at <= ?1 OR absolute_expires_at <= ?1)",
                    )?;
                    let rows = statement.query_map([now], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                for (family_id, client_id) in &expired {
                    revoke_family(&tx, family_id, client_id, now, "expired")?;
                }
                tx.commit()?;
                Ok(expired
                    .into_iter()
                    .map(|(_, client_id)| client_id)
                    .collect::<std::collections::BTreeSet<_>>())
            })
            .await?;
        for client_id in &revoked_clients {
            let _ = self.client_revocations.send(client_id.clone());
        }
        Ok(revoked_clients.len())
    }

    /// Revoke the refresh family that owns one refresh credential.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot commit the revocation.
    pub async fn revoke_native_oauth_family(
        &self,
        refresh_hash: [u8; 32],
        now: i64,
    ) -> Result<(), StoreError> {
        let client_id = self
            .with_connection(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let row = load_refresh(&tx, refresh_hash)?;
                if let Some(row) = row {
                    revoke_family(&tx, &row.family_id, &row.client_id, now, "client")?;
                    tx.commit()?;
                    Ok(Some(row.client_id))
                } else {
                    tx.commit()?;
                    Ok(None)
                }
            })
            .await?;
        if let Some(client_id) = client_id {
            let _ = self.client_revocations.send(client_id);
        }
        Ok(())
    }
}

struct RefreshRow {
    family_id: String,
    client_id: String,
    owner_human_id: String,
    scope: String,
    absolute_expires_at: i64,
    idle_expires_at: i64,
    revoked_at: Option<i64>,
    sequence: i64,
    status: String,
    used_at: Option<i64>,
}

fn load_refresh(
    tx: &Transaction<'_>,
    token_hash: [u8; 32],
) -> Result<Option<RefreshRow>, StoreError> {
    tx.query_row(
        "SELECT family.family_id, family.client_id, family.owner_human_id, family.scope, family.absolute_expires_at, family.idle_expires_at, family.revoked_at, refresh.sequence, refresh.status, refresh.used_at FROM native_oauth_refresh_tokens AS refresh JOIN native_oauth_families AS family ON family.family_id = refresh.family_id WHERE refresh.token_hash = ?1",
        [token_hash.as_slice()],
        |row| {
            Ok(RefreshRow {
                family_id: row.get(0)?,
                client_id: row.get(1)?,
                owner_human_id: row.get(2)?,
                scope: row.get(3)?,
                absolute_expires_at: row.get(4)?,
                idle_expires_at: row.get(5)?,
                revoked_at: row.get(6)?,
                sequence: row.get(7)?,
                status: row.get(8)?,
                used_at: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(StoreError::Sqlite)
}

fn retryable_access_expiry(
    tx: &Transaction<'_>,
    used: &RefreshRow,
    proof: &NativeOAuthRetryProof,
    now: i64,
) -> Result<Option<i64>, StoreError> {
    let Some(used_at) = used.used_at else {
        return Ok(None);
    };
    if now < used_at || now - used_at > NATIVE_OAUTH_RETRY_SECONDS {
        return Ok(None);
    }
    tx.query_row(
        "SELECT access.expires_at FROM native_oauth_refresh_tokens AS active JOIN native_oauth_access_tokens AS access ON access.family_id = active.family_id AND access.issued_at = active.issued_at WHERE active.family_id = ?1 AND active.sequence = ?2 AND active.status = 'active' AND active.token_hash = ?3 AND active.issued_at = ?4 AND access.token_hash = ?5 AND access.revoked_at IS NULL AND access.expires_at > ?6",
        params![
            used.family_id,
            used.sequence + 1,
            proof.refresh_hash.as_slice(),
            proof.issued_at,
            proof.access_hash.as_slice(),
            now,
        ],
        |row| row.get(0),
    )
    .optional()
    .map_err(StoreError::Sqlite)
}

fn revoke_family(
    tx: &Transaction<'_>,
    family_id: &str,
    client_id: &str,
    now: i64,
    reason: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE native_oauth_families SET revoked_at = ?2, revoke_reason = ?3 WHERE family_id = ?1 AND revoked_at IS NULL",
        params![family_id, now, reason],
    )?;
    tx.execute(
        "UPDATE native_oauth_access_tokens SET revoked_at = ?2 WHERE family_id = ?1 AND revoked_at IS NULL",
        params![family_id, now],
    )?;
    revoke_client_dependents(tx, client_id)?;
    Ok(())
}

fn validate_code(code: &NewNativeOAuthCode<'_>, now: i64) -> Result<(), StoreError> {
    if !valid_client_id(code.client_id)
        || code.display_name.trim() != code.display_name
        || code.display_name.is_empty()
        || code.display_name.len() > 128
        || code.redirect_uri.is_empty()
        || code.redirect_uri.len() > 2048
        || !(44..=256).contains(&code.pkce_value.len())
        || code.expires_at <= now
    {
        return Err(StoreError::InvariantViolation {
            message: "native OAuth code is outside its bounds".to_string(),
        });
    }
    Ok(())
}

fn validate_family(family: &NewNativeOAuthFamily<'_>) -> Result<(), StoreError> {
    if !valid_client_id(family.client_id)
        || family.family_id.len() != 32
        || !family
            .family_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || family.access_expires_at <= family.issued_at
        || family.idle_expires_at <= family.issued_at
        || family.absolute_expires_at < family.idle_expires_at
    {
        return Err(StoreError::InvariantViolation {
            message: "native OAuth family is outside its bounds".to_string(),
        });
    }
    Ok(())
}

fn valid_client_id(client_id: &str) -> bool {
    client_id.trim() == client_id && !client_id.is_empty() && client_id.len() <= 128
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::StoreConfig;

    async fn store() -> (TempDir, NoemaStore) {
        let home = TempDir::new().expect("store root");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("noema.sqlite3")))
            .await
            .expect("store");
        (home, store)
    }

    async fn insert_code(store: &NoemaStore, code_hash: [u8; 32], now: i64) {
        let pkce_value = "a".repeat(43) + "S";
        store
            .insert_native_oauth_code(
                code_hash,
                NewNativeOAuthCode {
                    client_id: "noema-desktop:installation",
                    display_name: "Noema Desktop",
                    redirect_uri: "http://127.0.0.1:49152/oauth/callback",
                    pkce_value: &pkce_value,
                    expires_at: now + 600,
                },
                now,
            )
            .await
            .expect("insert code");
    }

    #[tokio::test]
    async fn authorization_codes_store_only_digests_and_are_single_use() {
        let (_home, store) = store().await;
        let now = 1_700_000_000;
        let hash = [7_u8; 32];
        insert_code(&store, hash, now).await;

        store
            .with_connection(|conn| {
                let stored =
                    conn.query_row("SELECT code_hash FROM native_oauth_codes", [], |row| {
                        row.get::<_, Vec<u8>>(0)
                    })?;
                assert_eq!(stored, hash);
                Ok(())
            })
            .await
            .expect("inspect digest");
        assert!(
            store
                .consume_native_oauth_code(hash, now)
                .await
                .expect("consume")
                .is_some()
        );
        assert!(
            store
                .consume_native_oauth_code(hash, now)
                .await
                .expect("consume twice")
                .is_none()
        );
    }

    #[tokio::test]
    async fn refresh_retry_survives_response_loss_but_later_replay_revokes_family() {
        let (_home, store) = store().await;
        let now = 1_700_000_000;
        insert_code(&store, [1_u8; 32], now).await;
        store
            .consume_native_oauth_code([1_u8; 32], now)
            .await
            .expect("consume code");
        store
            .insert_native_oauth_family(NewNativeOAuthFamily {
                family_id: "0123456789abcdef0123456789abcdef",
                client_id: "noema-desktop:installation",
                access_hash: [2_u8; 32],
                refresh_hash: [3_u8; 32],
                issued_at: now,
                access_expires_at: now + 900,
                idle_expires_at: now + 2_592_000,
                absolute_expires_at: now + 15_552_000,
            })
            .await
            .expect("family");
        assert!(matches!(
            store
                .native_oauth_refresh_grant([3_u8; 32], None, now + 1)
                .await
                .expect("refresh grant"),
            NativeOAuthRefreshLookup::Active(_)
        ));
        assert_eq!(
            store
                .rotate_native_oauth_refresh(
                    [3_u8; 32],
                    NativeOAuthRotation {
                        access_hash: [4_u8; 32],
                        refresh_hash: [5_u8; 32],
                        issued_at: now + 1,
                        access_expires_at: now + 901,
                        idle_expires_at: now + 2_592_001,
                    },
                    now + 1,
                )
                .await
                .expect("rotate"),
            NativeOAuthRotationOutcome::Rotated
        );
        assert_eq!(
            store
                .native_oauth_refresh_grant(
                    [3_u8; 32],
                    Some(NativeOAuthRetryProof {
                        access_hash: [4_u8; 32],
                        refresh_hash: [5_u8; 32],
                        issued_at: now + 1,
                    }),
                    now + 2,
                )
                .await
                .expect("retry"),
            NativeOAuthRefreshLookup::Retryable {
                access_expires_at: now + 901
            }
        );
        assert!(
            store
                .active_native_oauth_client([4_u8; 32], now + 2)
                .await
                .expect("access lookup")
                .is_some()
        );
        assert!(matches!(
            store
                .native_oauth_refresh_grant([5_u8; 32], None, now + 2)
                .await
                .expect("successor grant"),
            NativeOAuthRefreshLookup::Active(_)
        ));
        assert_eq!(
            store
                .rotate_native_oauth_refresh(
                    [5_u8; 32],
                    NativeOAuthRotation {
                        access_hash: [6_u8; 32],
                        refresh_hash: [7_u8; 32],
                        issued_at: now + 2,
                        access_expires_at: now + 902,
                        idle_expires_at: now + 2_592_002,
                    },
                    now + 2,
                )
                .await
                .expect("rotate successor"),
            NativeOAuthRotationOutcome::Rotated
        );
        assert_eq!(
            store
                .native_oauth_refresh_grant(
                    [3_u8; 32],
                    Some(NativeOAuthRetryProof {
                        access_hash: [4_u8; 32],
                        refresh_hash: [5_u8; 32],
                        issued_at: now + 1,
                    }),
                    now + 3,
                )
                .await
                .expect("late replay"),
            NativeOAuthRefreshLookup::ReplayRevoked
        );
        assert!(
            store
                .active_native_oauth_client([6_u8; 32], now + 3)
                .await
                .expect("revoked access lookup")
                .is_none()
        );
        store
            .with_connection(|conn| {
                let members = conn.query_row(
                    "SELECT count(*) FROM native_oauth_refresh_tokens",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!(members, 3);
                Ok(())
            })
            .await
            .expect("used members remain");
    }

    #[tokio::test]
    async fn expiry_cleanup_revokes_access_and_notifies_the_client() {
        let (_home, store) = store().await;
        let now = 1_700_000_000;
        insert_code(&store, [11_u8; 32], now).await;
        store
            .consume_native_oauth_code([11_u8; 32], now)
            .await
            .expect("consume code");
        store
            .insert_native_oauth_family(NewNativeOAuthFamily {
                family_id: "1123456789abcdef0123456789abcdef",
                client_id: "noema-desktop:installation",
                access_hash: [12_u8; 32],
                refresh_hash: [13_u8; 32],
                issued_at: now,
                access_expires_at: now + 900,
                idle_expires_at: now + 10,
                absolute_expires_at: now + 20,
            })
            .await
            .expect("family");
        let mut revocations = store.subscribe_client_revocations();

        assert_eq!(
            store
                .expire_native_oauth_families(now + 10)
                .await
                .expect("expire families"),
            1
        );
        assert_eq!(
            revocations.recv().await.expect("revocation event"),
            "noema-desktop:installation"
        );
        assert!(
            store
                .active_native_oauth_client([12_u8; 32], now + 10)
                .await
                .expect("access lookup")
                .is_none()
        );
    }
}
