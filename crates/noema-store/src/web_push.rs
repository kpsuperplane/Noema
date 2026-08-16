use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, ids::allocate_id};

/// Durable VAPID identity for one Noema installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPushIdentity {
    /// Private P-256 scalar retained only by the daemon.
    pub private_key: Vec<u8>,
    /// Uncompressed P-256 public key shared with browsers.
    pub public_key: Vec<u8>,
}

/// Durable scan position for primary-conversation notification projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPushPrimaryCheckpoint {
    /// Conversation whose sequence the checkpoint describes.
    pub conversation_id: Option<String>,
    /// Greatest conversation item sequence already projected.
    pub sequence: i64,
}

/// Browser-provided Web Push subscription input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewWebPushSubscription {
    /// Human who owns the subscription.
    pub owner_human_id: String,
    /// Digest of the browser session that authorized this subscription.
    pub browser_session_hash: [u8; 32],
    /// Browser-issued, bearer-like push service endpoint.
    pub endpoint: String,
    /// Browser P-256 Diffie-Hellman public key.
    pub p256dh: String,
    /// Browser authentication secret.
    pub auth_secret: String,
}

/// Opaque registered Web Push client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPushSubscription {
    /// Server-issued opaque client identifier.
    pub subscription_id: String,
    /// Human who owns the subscription.
    pub owner_human_id: String,
    /// Browser-issued push service endpoint.
    pub endpoint: String,
    /// Browser P-256 Diffie-Hellman public key.
    pub p256dh: String,
    /// Browser authentication secret.
    pub auth_secret: String,
}

/// One due Web Push delivery with its private subscription material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimedWebPushDelivery {
    /// Target browser subscription and encryption material.
    pub subscription: WebPushSubscription,
    /// Stable logical notification identifier.
    pub event_key: String,
    /// Human-visible notification title.
    pub title: String,
    /// Human-visible notification preview.
    pub body: String,
    /// Same-origin destination opened by the notification.
    pub navigate_path: String,
    /// Web Push urgency header value.
    pub urgency: String,
    /// Web Push time-to-live in seconds.
    pub ttl_seconds: u32,
    /// One-based delivery attempt number.
    pub attempt_count: u32,
}

impl NoemaStore {
    /// Return the durable VAPID identity, inserting caller-generated material once.
    ///
    /// # Errors
    /// Returns an invariant or storage error when the key material is invalid or unavailable.
    pub async fn get_or_insert_web_push_identity(
        &self,
        private_key: &[u8],
        public_key: &[u8],
    ) -> Result<WebPushIdentity, StoreError> {
        if private_key.len() != 32 || public_key.len() != 65 {
            return Err(StoreError::InvariantViolation {
                message: "invalid Web Push identity material".to_string(),
            });
        }
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO web_push_identity (identity_id, private_key, public_key) VALUES (1, ?1, ?2) ON CONFLICT(identity_id) DO NOTHING",
                params![private_key, public_key],
            )?;
            conn.query_row(
                "SELECT private_key, public_key FROM web_push_identity WHERE identity_id = 1",
                [],
                |row| Ok(WebPushIdentity { private_key: row.get(0)?, public_key: row.get(1)? }),
            )
            .map_err(StoreError::Sqlite)
        }).await
    }

    /// Return the durable primary-conversation projection checkpoint.
    ///
    /// # Errors
    /// Returns a storage error when the checkpoint cannot be read.
    pub async fn notification_primary_checkpoint(
        &self,
    ) -> Result<WebPushPrimaryCheckpoint, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT primary_conversation_id, primary_sequence FROM notification_projection_state WHERE state_id = 1",
                [],
                |row| {
                    Ok(WebPushPrimaryCheckpoint {
                        conversation_id: row.get(0)?,
                        sequence: row.get(1)?,
                    })
                },
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Advance the primary-conversation projection checkpoint monotonically.
    ///
    /// # Errors
    /// Returns an invariant or storage error when the checkpoint is invalid or cannot be saved.
    pub async fn advance_notification_primary_checkpoint(
        &self,
        conversation_id: &str,
        sequence: i64,
    ) -> Result<(), StoreError> {
        if conversation_id.trim().is_empty() || sequence < 0 {
            return Err(StoreError::InvariantViolation {
                message: "invalid Web Push primary checkpoint".to_string(),
            });
        }
        self.with_connection(|conn| {
            conn.execute(
                r#"UPDATE notification_projection_state
                   SET primary_conversation_id = ?1,
                       primary_sequence = CASE
                         WHEN primary_conversation_id = ?1 THEN max(primary_sequence, ?2)
                         ELSE ?2
                       END,
                       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                   WHERE state_id = 1
                     AND (primary_conversation_id IS NOT ?1 OR primary_sequence < ?2)"#,
                params![conversation_id, sequence],
            )?;
            Ok(())
        })
        .await
    }

    /// Register or refresh one browser subscription by its capability endpoint.
    ///
    /// # Errors
    /// Returns an invariant or storage error when the subscription is invalid or cannot be saved.
    pub async fn register_web_push_subscription(
        &self,
        input: NewWebPushSubscription,
    ) -> Result<WebPushSubscription, StoreError> {
        if input.owner_human_id.trim().is_empty()
            || input.endpoint.len() > 2048
            || !(40..=256).contains(&input.p256dh.len())
            || !(16..=128).contains(&input.auth_secret.len())
        {
            return Err(StoreError::InvariantViolation {
                message: "invalid Web Push subscription".to_string(),
            });
        }
        let subscription_id = allocate_id("push_subscription");
        self.with_connection(|conn| {
            conn.execute(
                r#"INSERT INTO web_push_subscriptions
                   (subscription_id, owner_human_id, browser_session_hash, endpoint, p256dh, auth_secret)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                   ON CONFLICT(endpoint) DO UPDATE SET
                     owner_human_id = excluded.owner_human_id,
                     browser_session_hash = excluded.browser_session_hash,
                     p256dh = excluded.p256dh,
                     auth_secret = excluded.auth_secret,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
                params![
                    subscription_id,
                    input.owner_human_id,
                    input.browser_session_hash.as_slice(),
                    input.endpoint,
                    input.p256dh,
                    input.auth_secret
                ],
            )?;
            load_subscription_by_endpoint(
                conn,
                &input.owner_human_id,
                input.browser_session_hash,
                &input.endpoint,
            )?
            .ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "registered Web Push subscription disappeared".to_string(),
                }
            })
        })
        .await
    }

    /// Find the caller-owned subscription registered for an endpoint.
    ///
    /// # Errors
    /// Returns a storage error when the subscription cannot be read.
    pub async fn web_push_subscription_for_endpoint(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        endpoint: &str,
    ) -> Result<Option<WebPushSubscription>, StoreError> {
        self.with_connection(|conn| {
            load_subscription_by_endpoint(conn, owner_human_id, browser_session_hash, endpoint)
        })
        .await
    }

    /// Remove one caller-owned subscription and its delivery history.
    ///
    /// # Errors
    /// Returns a storage error when the subscription cannot be removed.
    pub async fn remove_web_push_subscription(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        subscription_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            Ok(conn.execute(
                "DELETE FROM web_push_subscriptions WHERE subscription_id = ?1 AND owner_human_id = ?2 AND browser_session_hash = ?3",
                params![subscription_id, owner_human_id, browser_session_hash.as_slice()],
            )? == 1)
        }).await
    }

    /// List opaque client ids for one human without exposing subscription secrets.
    ///
    /// # Errors
    /// Returns a storage error when the subscriptions cannot be read.
    pub async fn web_push_subscription_ids(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
    ) -> Result<Vec<String>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT subscription_id FROM web_push_subscriptions WHERE owner_human_id = ?1 AND browser_session_hash = ?2 ORDER BY created_at, subscription_id",
            )?;
            let rows = statement.query_map(
                params![owner_human_id, browser_session_hash.as_slice()],
                |row| row.get(0),
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        }).await
    }

    /// Delete Web Push authority bound to one revoked browser session.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot delete the subscriptions.
    pub async fn remove_web_push_for_session(
        &self,
        browser_session_hash: [u8; 32],
    ) -> Result<usize, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM web_push_subscriptions WHERE browser_session_hash = ?1",
                [browser_session_hash.as_slice()],
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Delete all browser Push authority after global browser revocation.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if storage cannot delete the subscriptions.
    pub async fn remove_all_web_push_subscriptions(&self) -> Result<usize, StoreError> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM web_push_subscriptions", [])
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Queue one logical notification for every active subscription exactly once.
    ///
    /// # Errors
    /// Returns an invariant or storage error when the notification is invalid or cannot be queued.
    /// Atomically return the next due delivery and increment its attempt count.
    ///
    /// # Errors
    /// Returns a storage error when the next delivery cannot be claimed.
    pub async fn claim_due_web_push_delivery(
        &self,
    ) -> Result<Option<ClaimedWebPushDelivery>, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let key = transaction.query_row(
                "SELECT subscription_id, event_key FROM web_push_deliveries WHERE status = 'pending' AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ORDER BY available_at, subscription_id, event_key LIMIT 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ).optional()?;
            let Some((subscription_id, event_key)) = key else { return Ok(None); };
            transaction.execute(
                "UPDATE web_push_deliveries SET attempt_count = attempt_count + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE subscription_id = ?1 AND event_key = ?2",
                params![subscription_id, event_key],
            )?;
            let delivery = transaction.query_row(
                r#"SELECT s.subscription_id, s.owner_human_id, s.endpoint, s.p256dh, s.auth_secret,
                          d.event_key, d.title, d.body, d.navigate_path, d.urgency, d.ttl_seconds, d.attempt_count
                   FROM web_push_deliveries d JOIN web_push_subscriptions s USING (subscription_id)
                   WHERE d.subscription_id = ?1 AND d.event_key = ?2"#,
                params![subscription_id, event_key],
                |row| Ok(ClaimedWebPushDelivery {
                    subscription: WebPushSubscription { subscription_id: row.get(0)?, owner_human_id: row.get(1)?, endpoint: row.get(2)?, p256dh: row.get(3)?, auth_secret: row.get(4)? },
                    event_key: row.get(5)?, title: row.get(6)?, body: row.get(7)?, navigate_path: row.get(8)?, urgency: row.get(9)?,
                    ttl_seconds: row.get::<_, u32>(10)?, attempt_count: row.get::<_, u32>(11)?,
                }),
            )?;
            transaction.commit()?;
            Ok(Some(delivery))
        }).await
    }

    /// Mark one delivery complete, suppressed, retryable, terminal, or expired.
    ///
    /// # Errors
    /// Returns an invariant or storage error when the disposition is invalid or cannot be saved.
    pub async fn finish_web_push_delivery(
        &self,
        subscription_id: &str,
        event_key: &str,
        disposition: &str,
        error_code: Option<&str>,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            match disposition {
                "expired" => { conn.execute("DELETE FROM web_push_subscriptions WHERE subscription_id = ?1", [subscription_id])?; }
                "retry" => {
                    let attempt: u32 = conn.query_row(
                        "SELECT attempt_count FROM web_push_deliveries WHERE subscription_id = ?1 AND event_key = ?2",
                        params![subscription_id, event_key], |row| row.get(0),
                    )?;
                    let delay = match attempt { 0 | 1 => 60, 2 => 300, _ => 1800 };
                    let status = if attempt >= 4 { "failed" } else { "pending" };
                    conn.execute(
                        "UPDATE web_push_deliveries SET status = ?3, available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?4 || ' seconds'), last_error_code = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE subscription_id = ?1 AND event_key = ?2",
                        params![subscription_id, event_key, status, delay, error_code],
                    )?;
                }
                "delivered" | "suppressed" | "failed" => {
                    conn.execute(
                        "UPDATE web_push_deliveries SET status = ?3, last_error_code = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE subscription_id = ?1 AND event_key = ?2",
                        params![subscription_id, event_key, disposition, error_code],
                    )?;
                }
                _ => return Err(StoreError::InvariantViolation { message: "invalid Web Push delivery disposition".to_string() }),
            }
            Ok(())
        }).await
    }

    /// Insert an observed attention key, returning whether it is new.
    ///
    /// # Errors
    /// Returns a storage error when the attention key cannot be recorded.
    pub async fn observe_notification_attention(
        &self,
        attention_key: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|conn| Ok(conn.execute(
                "INSERT INTO notification_attention_seen (attention_key) VALUES (?1) ON CONFLICT(attention_key) DO NOTHING",
            [attention_key],
        )? == 1)).await
    }

    /// Read and update first-run attention seeding state.
    ///
    /// # Errors
    /// Returns a storage error when the seed state cannot be read.
    pub async fn notification_attention_seeded(&self) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT attention_seeded FROM notification_projection_state WHERE state_id = 1",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Mark the existing intervention set as seeded so only later cards notify.
    ///
    /// # Errors
    /// Returns a storage error when the seed state cannot be saved.
    pub async fn mark_notification_attention_seeded(&self) -> Result<(), StoreError> {
        self.with_connection(|conn| { conn.execute(
            "UPDATE notification_projection_state SET attention_seeded = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE state_id = 1", [],
        )?; Ok(()) }).await
    }
}

fn load_subscription_by_endpoint(
    conn: &rusqlite::Connection,
    owner_human_id: &str,
    browser_session_hash: [u8; 32],
    endpoint: &str,
) -> Result<Option<WebPushSubscription>, StoreError> {
    conn.query_row(
        "SELECT subscription_id, owner_human_id, endpoint, p256dh, auth_secret FROM web_push_subscriptions WHERE endpoint = ?1 AND owner_human_id = ?2 AND browser_session_hash = ?3",
        params![endpoint, owner_human_id, browser_session_hash.as_slice()],
        |row| Ok(WebPushSubscription { subscription_id: row.get(0)?, owner_human_id: row.get(1)?, endpoint: row.get(2)?, p256dh: row.get(3)?, auth_secret: row.get(4)? }),
    ).optional().map_err(StoreError::Sqlite)
}
