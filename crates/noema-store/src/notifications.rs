//! Shared notification projection state, paired-client registrations, and APNs delivery state.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};

/// Apple Push Notification service destination environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApnsEnvironment {
    /// Apple development gateway.
    Development,
    /// Apple production gateway.
    Production,
}

impl ApnsEnvironment {
    #[must_use]
    /// Return the canonical persisted environment value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }

    fn parse(value: String) -> Result<Self, StoreError> {
        match value.as_str() {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            _ => Err(StoreError::InvalidEnum {
                kind: "APNs environment",
                value,
            }),
        }
    }
}

/// A paired client's governed APNs registration.
#[derive(Clone, PartialEq, Eq)]
pub struct ClientNotificationRecord {
    /// Paired client identifier owning this registration.
    pub client_id: String,
    /// Opaque APNs token bytes.
    pub device_token: Vec<u8>,
    /// APNs gateway environment for the token.
    pub environment: ApnsEnvironment,
}

impl std::fmt::Debug for ClientNotificationRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientNotificationRecord")
            .field("client_id", &self.client_id)
            .field("device_token", &"<redacted>")
            .field("environment", &self.environment)
            .finish()
    }
}

/// A claimed APNs delivery and its bound registration.
#[derive(Clone, PartialEq, Eq)]
pub struct ClaimedApnsDelivery {
    /// Registration bound when the delivery was claimed.
    pub client: ClientNotificationRecord,
    /// Stable notification event key.
    pub event_key: String,
    /// Notification title.
    pub title: String,
    /// Bounded notification preview.
    pub body: String,
    /// Delivery urgency.
    pub urgency: String,
    /// Event lifetime in seconds.
    pub ttl_seconds: u32,
    /// Original queue timestamp used for absolute expiry.
    pub created_at: String,
}

impl std::fmt::Debug for ClaimedApnsDelivery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimedApnsDelivery")
            .field("client", &self.client)
            .field("event_key", &self.event_key)
            .field("title", &self.title)
            .field("body", &self.body)
            .field("urgency", &self.urgency)
            .field("ttl_seconds", &self.ttl_seconds)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl NoemaStore {
    #[doc = "Atomically fan one projected event out to eligible Web Push and APNs destinations.\n\n# Errors\nReturns a store error when validation or the shared transaction fails."]
    #[allow(
        clippy::too_many_arguments,
        reason = "one atomic fan-out owns both transports"
    )]
    pub async fn queue_notification_fanout(
        &self,
        owner_human_id: &str,
        event_key: &str,
        title: &str,
        body: &str,
        urgency: &str,
        ttl_seconds: u32,
        visible_web: &HashSet<String>,
        visible_clients: &HashSet<String>,
        apns_enabled: bool,
    ) -> Result<(), StoreError> {
        if owner_human_id.trim().is_empty()
            || event_key.trim().is_empty()
            || title.trim().is_empty()
            || body.len() > 2048
            || !matches!(urgency, "normal" | "high")
            || ttl_seconds > 604_800
        {
            return Err(invalid("invalid notification fan-out"));
        }
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let mut statement = transaction.prepare(
                "SELECT subscription_id FROM web_push_subscriptions WHERE owner_human_id = ?1",
            )?;
            let web_ids = statement
                .query_map([owner_human_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for subscription_id in web_ids {
                let status = if visible_web.contains(&subscription_id) {
                    "suppressed"
                } else {
                    "pending"
                };
                transaction.execute(
                    r#"INSERT INTO web_push_deliveries
                       (subscription_id, event_key, title, body, navigate_path, urgency, ttl_seconds, status, available_at)
                       VALUES (?1, ?2, ?3, ?4, '/', ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 second'))
                       ON CONFLICT(subscription_id, event_key) DO UPDATE SET
                         title = CASE WHEN status = 'pending' THEN excluded.title ELSE title END,
                         body = CASE WHEN status = 'pending' THEN excluded.body ELSE body END,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
                    params![subscription_id, event_key, title, body, urgency, i64::from(ttl_seconds), status],
                )?;
            }
            if apns_enabled {
                let mut statement = transaction.prepare(
                    "SELECT r.client_id FROM client_notification_registrations r JOIN clients c USING (client_id) WHERE c.owner_human_id = ?1 AND c.revoked_at IS NULL",
                )?;
                let client_ids = statement
                    .query_map([owner_human_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                drop(statement);
                for client_id in client_ids {
                    let status = if visible_clients.contains(&client_id) {
                        "suppressed"
                    } else {
                        "pending"
                    };
                    transaction.execute(
                        r#"INSERT INTO apns_deliveries
                           (client_id, event_key, title, body, urgency, ttl_seconds, status, available_at)
                           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 second'))
                           ON CONFLICT(client_id, event_key) DO UPDATE SET
                             title = CASE WHEN status = 'pending' THEN excluded.title ELSE title END,
                             body = CASE WHEN status = 'pending' THEN excluded.body ELSE body END,
                             urgency = CASE WHEN status = 'pending' THEN excluded.urgency ELSE urgency END,
                             ttl_seconds = CASE WHEN status = 'pending' THEN excluded.ttl_seconds ELSE ttl_seconds END,
                             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
                        params![client_id, event_key, title, body, urgency, i64::from(ttl_seconds), status],
                    )?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    #[doc = "Return one active paired-client notification registration.\n\n# Errors\nReturns a store error when the registration cannot be read."]
    pub async fn client_notification_registration(
        &self,
        client_id: &str,
    ) -> Result<Option<ClientNotificationRecord>, StoreError> {
        self.with_connection(|conn| load_client_notification(conn, client_id))
            .await
    }

    #[doc = "Register or rotate the authenticated paired client's APNs token.\n\n# Errors\nReturns a store error when validation or the atomic registration transfer fails."]
    pub async fn register_client_notifications(
        &self,
        client_id: &str,
        device_token: &[u8],
        environment: ApnsEnvironment,
    ) -> Result<ClientNotificationRecord, StoreError> {
        if client_id.trim().is_empty() || !(1..=1024).contains(&device_token.len()) {
            return Err(invalid("invalid APNs client registration"));
        }
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            transaction.execute(
                "DELETE FROM apns_deliveries WHERE client_id = ?1 AND EXISTS (SELECT 1 FROM client_notification_registrations WHERE client_id = ?1 AND (device_token <> ?2 OR environment <> ?3))",
                params![client_id, device_token, environment.as_str()],
            )?;
            transaction.execute(
                "DELETE FROM client_notification_registrations WHERE environment = ?1 AND device_token = ?2 AND client_id <> ?3",
                params![environment.as_str(), device_token, client_id],
            )?;
            let changed = transaction.execute(
                r#"INSERT INTO client_notification_registrations
                   (client_id, device_token, environment)
                   SELECT ?1, ?2, ?3 FROM clients
                   WHERE client_id = ?1 AND revoked_at IS NULL
                   ON CONFLICT(client_id) DO UPDATE SET
                     device_token = excluded.device_token,
                     environment = excluded.environment,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
                params![client_id, device_token, environment.as_str()],
            )?;
            if changed != 1 {
                return Err(invalid("paired client is unavailable for APNs registration"));
            }
            let record = load_client_notification(&transaction, client_id)?
                .ok_or_else(|| invalid("registered APNs client disappeared"))?;
            transaction.commit()?;
            Ok(record)
        })
        .await
    }

    #[doc = "Remove a paired client's APNs registration and pending deliveries.\n\n# Errors\nReturns a store error when the atomic removal fails."]
    pub async fn disable_client_notifications(&self, client_id: &str) -> Result<bool, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let changed = transaction.execute(
                "UPDATE apns_deliveries SET status = 'failed', last_error_code = 'client_disabled', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND status = 'pending'",
                [client_id],
            )?;
            let registration_changed = transaction.execute(
                "DELETE FROM client_notification_registrations WHERE client_id = ?1",
                [client_id],
            )?;
            transaction.commit()?;
            Ok(registration_changed == 1 || changed > 0)
        })
        .await
    }

    #[doc = "Claim the next due APNs delivery.\n\n# Errors\nReturns a store error when the claim transaction fails."]
    pub async fn claim_due_apns_delivery(&self) -> Result<Option<ClaimedApnsDelivery>, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let key = transaction
                .query_row(
                    "SELECT client_id, event_key FROM apns_deliveries WHERE status = 'pending' AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ORDER BY available_at, client_id, event_key LIMIT 1",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((client_id, event_key)) = key else {
                return Ok(None);
            };
            transaction.execute(
                "UPDATE apns_deliveries SET attempt_count = attempt_count + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND event_key = ?2",
                params![client_id, event_key],
            )?;
            let delivery = transaction.query_row(
                r#"SELECT r.client_id, r.device_token, r.environment,
                          d.event_key, d.title, d.body, d.urgency, d.ttl_seconds, d.created_at
                   FROM apns_deliveries d
                   JOIN client_notification_registrations r USING (client_id)
                   JOIN clients c USING (client_id)
                   WHERE d.client_id = ?1 AND d.event_key = ?2"#,
                params![client_id, event_key],
                |row| {
                    Ok(ClaimedApnsDelivery {
                        client: ClientNotificationRecord {
                            client_id: row.get(0)?,
                            device_token: row.get(1)?,
                            environment: ApnsEnvironment::parse(row.get(2)?)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        },
                        event_key: row.get(3)?,
                        title: row.get(4)?,
                        body: row.get(5)?,
                        urgency: row.get(6)?,
                        ttl_seconds: row.get::<_, u32>(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )?;
            transaction.commit()?;
            Ok(Some(delivery))
        })
        .await
    }

    #[doc = "Complete a claimed APNs delivery only while its token binding remains current.\n\n# Errors\nReturns a store error for invalid dispositions or failed persistence."]
    pub async fn finish_apns_delivery(
        &self,
        client_id: &str,
        event_key: &str,
        expected_device_token: &[u8],
        disposition: &str,
        error_code: Option<&str>,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            match disposition {
                "invalid_token" => {
                    conn.execute(
                        "DELETE FROM client_notification_registrations WHERE client_id = ?1 AND device_token = ?2",
                        params![client_id, expected_device_token],
                    )?;
                }
                "retry" => {
                    let attempt: u32 = conn.query_row(
                        "SELECT attempt_count FROM apns_deliveries WHERE client_id = ?1 AND event_key = ?2",
                        params![client_id, event_key],
                        |row| row.get(0),
                    )?;
                    let delay = match attempt {
                        0 | 1 => 60,
                        2 => 300,
                        _ => 1800,
                    };
                    let status = if attempt >= 4 { "failed" } else { "pending" };
                    conn.execute(
                        "UPDATE apns_deliveries SET status = ?4, available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?5 || ' seconds'), last_error_code = ?6, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND event_key = ?2 AND status = 'pending' AND EXISTS (SELECT 1 FROM client_notification_registrations WHERE client_id = ?1 AND device_token = ?3)",
                        params![client_id, event_key, expected_device_token, status, delay, error_code],
                    )?;
                }
                "delivered" | "suppressed" | "failed" => {
                    conn.execute(
                        "UPDATE apns_deliveries SET status = ?4, last_error_code = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND event_key = ?2 AND status = 'pending' AND EXISTS (SELECT 1 FROM client_notification_registrations WHERE client_id = ?1 AND device_token = ?3)",
                        params![client_id, event_key, expected_device_token, disposition, error_code],
                    )?;
                }
                _ => return Err(invalid("invalid APNs delivery disposition")),
            }
            Ok(())
        })
        .await
    }

    #[doc = "Terminally fail every pending APNs delivery with a bounded diagnostic.\n\n# Errors\nReturns a store error for invalid diagnostics or failed persistence."]
    pub async fn fail_pending_apns_deliveries(&self, error_code: &str) -> Result<(), StoreError> {
        if error_code.is_empty() || error_code.len() > 128 || error_code.trim() != error_code {
            return Err(invalid("invalid APNs delivery diagnostic"));
        }
        self.with_connection(|conn| {
            conn.execute(
                "UPDATE apns_deliveries SET status = 'failed', last_error_code = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE status = 'pending'",
                [error_code],
            )?;
            Ok(())
        })
        .await
    }
}

fn load_client_notification(
    conn: &rusqlite::Connection,
    client_id: &str,
) -> Result<Option<ClientNotificationRecord>, StoreError> {
    conn.query_row(
        "SELECT r.client_id, r.device_token, r.environment FROM client_notification_registrations r JOIN clients c USING (client_id) WHERE r.client_id = ?1 AND c.revoked_at IS NULL",
        [client_id],
        |row| {
            Ok(ClientNotificationRecord {
                client_id: row.get(0)?,
                device_token: row.get(1)?,
                environment: ApnsEnvironment::parse(row.get(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            })
        },
    )
    .optional()
    .map_err(StoreError::Sqlite)
}

fn invalid(message: &str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_store;

    const LOCAL_HUMAN_ID: &str = "human:local";

    #[tokio::test]
    async fn registration_transfer_removes_old_owner_and_redacts_tokens() {
        let store = test_store().await;
        store
            .insert_client("client:one", LOCAL_HUMAN_ID, "Phone", [1; 32])
            .await
            .expect("insert first client");
        store
            .insert_client("client:two", LOCAL_HUMAN_ID, "Tablet", [2; 32])
            .await
            .expect("insert second client");
        let first = store
            .register_client_notifications("client:one", &[1, 2, 3], ApnsEnvironment::Development)
            .await
            .expect("register first client");
        assert!(!format!("{first:?}").contains("1, 2, 3"));
        store
            .register_client_notifications("client:two", &[1, 2, 3], ApnsEnvironment::Development)
            .await
            .expect("transfer token");
        assert!(
            store
                .client_notification_registration("client:one")
                .await
                .expect("read first registration")
                .is_none()
        );
        assert_eq!(
            store
                .client_notification_registration("client:two")
                .await
                .expect("read second registration")
                .expect("transferred registration")
                .environment,
            ApnsEnvironment::Development
        );
        store
            .queue_notification_fanout(
                LOCAL_HUMAN_ID,
                "chat-turn:before-token-rotation",
                "Noema",
                "Finished",
                "normal",
                3600,
                &HashSet::new(),
                &HashSet::new(),
                true,
            )
            .await
            .expect("queue delivery for old token");
        store
            .register_client_notifications("client:two", &[4, 5, 6], ApnsEnvironment::Development)
            .await
            .expect("rotate token");
        store
            .finish_apns_delivery(
                "client:two",
                "chat-turn:before-token-rotation",
                &[1, 2, 3],
                "invalid_token",
                Some("invalid_device_token"),
            )
            .await
            .expect("ignore stale invalid-token response");
        assert_eq!(
            store
                .client_notification_registration("client:two")
                .await
                .expect("read rotated registration")
                .expect("rotated registration remains")
                .device_token,
            [4, 5, 6]
        );
    }

    #[tokio::test]
    async fn configured_fanout_inserts_apns_rows_with_transport_visibility() {
        let store = test_store().await;
        store
            .insert_client("client:one", LOCAL_HUMAN_ID, "Phone", [1; 32])
            .await
            .expect("insert client");
        store
            .register_client_notifications("client:one", &[9, 8, 7], ApnsEnvironment::Production)
            .await
            .expect("register client");
        store
            .queue_notification_fanout(
                LOCAL_HUMAN_ID,
                "chat-turn:one",
                "Noema",
                "Finished",
                "high",
                3600,
                &HashSet::new(),
                &HashSet::from(["client:one".to_string()]),
                true,
            )
            .await
            .expect("queue fanout");
        let row = store
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT status, urgency, ttl_seconds FROM apns_deliveries WHERE client_id = 'client:one' AND event_key = 'chat-turn:one'",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?)),
                )
                .map_err(StoreError::Sqlite)
            })
            .await
            .expect("read APNs row");
        assert_eq!(row, ("suppressed".to_string(), "high".to_string(), 3600));

        store
            .queue_notification_fanout(
                LOCAL_HUMAN_ID,
                "chat-turn:two",
                "Noema",
                "Finished again",
                "normal",
                3600,
                &HashSet::new(),
                &HashSet::new(),
                true,
            )
            .await
            .expect("queue pending delivery");
        store
            .fail_pending_apns_deliveries("provider_unconfigured")
            .await
            .expect("terminalize pending delivery");
        store
            .finish_apns_delivery(
                "client:one",
                "chat-turn:two",
                &[9, 8, 7],
                "retry",
                Some("remote_retry"),
            )
            .await
            .expect("ignore stale send completion");
        let status = store
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT status FROM apns_deliveries WHERE client_id = 'client:one' AND event_key = 'chat-turn:two'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(StoreError::Sqlite)
            })
            .await
            .expect("read terminalized delivery");
        assert_eq!(status, "failed");
    }
}
