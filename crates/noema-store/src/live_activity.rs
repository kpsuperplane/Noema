#![allow(missing_docs)]
use super::{ApnsEnvironment, NoemaStore, StoreError, ids::allocate_id};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
#[derive(Clone, PartialEq, Eq)]
pub struct ClientLiveActivityRegistration {
    pub client_id: String,
    pub push_to_start_token: Option<Vec<u8>>,
    pub environment: Option<ApnsEnvironment>,
    pub enabled: bool,
}
impl std::fmt::Debug for ClientLiveActivityRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientLiveActivityRegistration")
            .field("client_id", &self.client_id)
            .field("environment", &self.environment)
            .field("enabled", &self.enabled)
            .finish()
    }
}
#[derive(Clone, PartialEq)]
pub struct ClientTaskActivityRecord {
    pub client_id: String,
    pub activity_id: Option<String>,
    pub task_session_id: String,
    pub lifecycle: String,
    pub update_token: Option<Vec<u8>>,
    pub latest_projection: Value,
    pub latest_projection_signature: String,
    pub focused_task_id: Option<String>,
    pub session_started_at: String,
    pub suppressed: bool,
    pub dismissed_at: Option<String>,
}
impl std::fmt::Debug for ClientTaskActivityRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientTaskActivityRecord")
            .field("client_id", &self.client_id)
            .field("activity_id", &self.activity_id)
            .field("task_session_id", &self.task_session_id)
            .field("lifecycle", &self.lifecycle)
            .field("focused_task_id", &self.focused_task_id)
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveActivityEvent {
    Start,
    Update,
    End,
}
impl LiveActivityEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Update => "update",
            Self::End => "end",
        }
    }
}
#[derive(Clone, PartialEq)]
pub struct ClaimedLiveActivityDelivery {
    pub client_id: String,
    pub delivery_key: String,
    pub activity_id: Option<String>,
    pub token: Vec<u8>,
    pub environment: ApnsEnvironment,
    pub event: LiveActivityEvent,
    pub payload: Value,
    pub urgency: String,
    pub ttl_seconds: u32,
    pub created_at: String,
}
impl std::fmt::Debug for ClaimedLiveActivityDelivery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimedLiveActivityDelivery")
            .field("client_id", &self.client_id)
            .field("delivery_key", &self.delivery_key)
            .field("activity_id", &self.activity_id)
            .field("environment", &self.environment)
            .field("event", &self.event)
            .field("urgency", &self.urgency)
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct TaskNotificationAlert {
    pub notification_id: String,
    pub event_sequence: u64,
    pub notification_kind: String,
    pub payload: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LiveActivityTarget {
    pub registration: ClientLiveActivityRegistration,
    pub activity: Option<ClientTaskActivityRecord>,
}
pub struct NewLiveActivityDelivery {
    pub client_id: String,
    pub delivery_key: String,
    pub activity_id: Option<String>,
    pub token: Vec<u8>,
    pub environment: ApnsEnvironment,
    pub event: LiveActivityEvent,
    pub payload: Value,
    pub urgency: String,
    pub ttl_seconds: u32,
}
impl NoemaStore {
    pub async fn client_live_activity_registration(
        &self,
        client_id: &str,
    ) -> Result<Option<ClientLiveActivityRegistration>, StoreError> {
        self.with_connection(|connection| load_registration(connection, client_id))
            .await
    }
    pub async fn register_client_live_activities(
        &self,
        client_id: &str,
        token: &[u8],
        environment: ApnsEnvironment,
        active_activity_ids: &[String],
    ) -> Result<ClientLiveActivityRegistration, StoreError> {
        validate_token(client_id, token)?;
        for activity_id in active_activity_ids {
            validate_activity_id(activity_id)?;
        }
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let old_clients = {
                let mut statement = transaction.prepare(
                    "SELECT client_id FROM client_live_activity_registrations WHERE environment = ?1 AND push_to_start_token = ?2 AND client_id <> ?3",
                )?;
                statement
                    .query_map(params![environment.as_str(), token, client_id], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for old_client_id in old_clients {
                transaction.execute(
                    "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'token_transferred', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND status = 'pending'",
                    [&old_client_id],
                )?;
                transaction.execute(
                    "DELETE FROM client_task_activities WHERE client_id = ?1",
                    [&old_client_id],
                )?;
                transaction.execute(
                    "DELETE FROM client_live_activity_registrations WHERE client_id = ?1",
                    [&old_client_id],
                )?;
            }
            let changed = transaction.execute(
                "INSERT INTO client_live_activity_registrations (client_id, push_to_start_token, environment, enabled) SELECT ?1, ?2, ?3, 1 FROM clients WHERE client_id = ?1 AND revoked_at IS NULL ON CONFLICT(client_id) DO UPDATE SET push_to_start_token = excluded.push_to_start_token, environment = excluded.environment, enabled = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![client_id, token, environment.as_str()],
            )?;
            if changed != 1 {
                return Err(invalid(
                    "paired client is unavailable for Live Activity registration",
                ));
            }
            let existing = load_activity(&transaction, client_id)?;
            let replace_failed_start = if let Some(activity) = existing.as_ref()
                && activity.lifecycle == "starting"
            {
                transaction.query_row(
                    "SELECT EXISTS (SELECT 1 FROM live_activity_deliveries WHERE client_id = ?1 AND activity_id = ?2 AND event = 'start' AND status IN ('failed', 'suppressed'))",
                    params![client_id, activity.activity_id.as_deref()],
                    |row| row.get::<_, bool>(0),
                )?
            } else {
                false
            };
            let replace_missing_active = existing.as_ref().is_some_and(|activity| {
                activity.lifecycle == "active"
                    && activity.activity_id.as_ref().is_none_or(|activity_id| {
                        !active_activity_ids.contains(activity_id)
                    })
            });
            if existing.is_none() || replace_failed_start || replace_missing_active {
                if replace_missing_active {
                    transaction.execute(
                        "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'activity_missing', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND status = 'pending'",
                        [client_id],
                    )?;
                }
                let session_id = allocate_id("task_activity");
                let activity_id = allocate_id("live_activity");
                transaction.execute(
                    "INSERT INTO client_task_activities (client_id, activity_id, task_session_id, lifecycle, latest_projection_json, latest_projection_signature, suppressed) VALUES (?1, ?2, ?3, 'starting', '{}', '', 0) ON CONFLICT(client_id) DO UPDATE SET activity_id = excluded.activity_id, task_session_id = excluded.task_session_id, lifecycle = 'starting', update_token = NULL, latest_projection_json = '{}', latest_projection_signature = '', focused_task_id = NULL, session_started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), suppressed = 0, dismissed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                    params![client_id, activity_id, session_id],
                )?;
            }
            let registration = load_registration(&transaction, client_id)?
                .ok_or_else(|| invalid("registered Live Activity disappeared"))?;
            transaction.commit()?;
            Ok(registration)
        })
        .await
    }
    pub async fn disable_client_live_activities(
        &self,
        client_id: &str,
    ) -> Result<bool, StoreError> {
        if client_id.trim().is_empty() {
            return Err(invalid("invalid Live Activity client"));
        }
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let registration_changed = transaction.execute(
                "INSERT INTO client_live_activity_registrations (client_id, enabled) SELECT client_id, 0 FROM clients WHERE client_id = ?1 AND revoked_at IS NULL ON CONFLICT(client_id) DO UPDATE SET push_to_start_token = NULL, environment = NULL, enabled = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                [client_id],
            )?;
            if registration_changed != 1 {
                return Err(invalid("paired client is unavailable for Live Activity disablement"));
            }
            let activity_changed = transaction.execute(
                "UPDATE client_task_activities SET lifecycle = 'dismissed', suppressed = 1, latest_projection_json = '{}', latest_projection_signature = '', focused_task_id = NULL, dismissed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1",
                [client_id],
            )?;
            transaction.execute(
                "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'activity_disabled', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND event <> 'end' AND status = 'pending'",
                [client_id],
            )?;
            transaction.commit()?;
            Ok(registration_changed == 1 || activity_changed == 1)
        })
        .await
    }
    pub async fn client_task_activity(
        &self,
        client_id: &str,
    ) -> Result<Option<ClientTaskActivityRecord>, StoreError> {
        self.with_connection(|connection| load_activity(connection, client_id))
            .await
    }
    pub async fn update_client_task_activity_projection(
        &self,
        client_id: &str,
        projection: &Value,
        signature: &str,
        focused_task_id: Option<&str>,
    ) -> Result<bool, StoreError> {
        let bytes = serde_json::to_vec(projection)?;
        if bytes.len() > 4096
            || (!signature.is_empty()
                && (signature.len() != 64
                    || signature.bytes().any(|byte| !byte.is_ascii_hexdigit())
                    || signature.bytes().any(|byte| byte.is_ascii_uppercase())))
        {
            return Err(invalid("invalid Live Activity projection"));
        }
        let projection_json =
            String::from_utf8(bytes).map_err(|_| invalid("invalid Live Activity projection"))?;
        let changed = self
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE client_task_activities SET latest_projection_json = ?2, latest_projection_signature = ?3, focused_task_id = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND lifecycle IN ('starting', 'active') AND suppressed = 0",
                        params![client_id, projection_json, signature, focused_task_id],
                    )
                    .map(|changed| changed == 1)
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        Ok(changed)
    }
    pub async fn ensure_client_task_activity_session(
        &self,
        client_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|connection| {
            let session_id = allocate_id("task_activity");
            let activity_id = allocate_id("live_activity");
            let changed = connection.execute(
                "UPDATE client_task_activities SET activity_id = ?2, task_session_id = ?3, lifecycle = 'starting', update_token = NULL, latest_projection_json = '{}', latest_projection_signature = '', focused_task_id = NULL, session_started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), suppressed = 0, dismissed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND lifecycle = 'dismissed' AND latest_projection_signature = '' AND EXISTS (SELECT 1 FROM client_live_activity_registrations WHERE client_id = ?1 AND enabled = 1)",
                params![client_id, activity_id, session_id],
            )?;
            Ok(changed == 1)
        })
        .await
    }
    pub async fn mark_client_task_activity_ending(
        &self,
        client_id: &str,
        activity_id: Option<&str>,
    ) -> Result<bool, StoreError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE client_task_activities SET lifecycle = 'ending', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND lifecycle IN ('starting', 'active') AND (?2 IS NULL OR activity_id = ?2)",
                params![client_id, activity_id],
            )?;
            Ok(changed == 1)
        })
        .await
    }
    pub async fn register_client_live_activity_update(
        &self,
        client_id: &str,
        activity_id: &str,
        token: &[u8],
    ) -> Result<bool, StoreError> {
        validate_activity_id(activity_id)?;
        validate_token(client_id, token)?;
        self.with_connection(|connection| {
            let changed = connection.execute(
                r#"UPDATE client_task_activities
                   SET activity_id = ?2, update_token = ?3, lifecycle = 'active',
                       suppressed = 0, dismissed_at = NULL,
                       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                   WHERE client_id = ?1 AND activity_id = ?2
                     AND lifecycle IN ('starting', 'active')
                     AND EXISTS (
                       SELECT 1 FROM client_live_activity_registrations
                       WHERE client_id = ?1 AND enabled = 1
                     )"#,
                params![client_id, activity_id, token],
            )?;
            Ok(changed == 1)
        })
        .await
    }
    pub async fn clear_client_task_activity_dismissal(
        &self,
        client_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE client_task_activities SET latest_projection_json = '{}', latest_projection_signature = '', focused_task_id = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND lifecycle = 'dismissed' AND suppressed = 1",
                [client_id],
            )?;
            Ok(changed == 1)
        })
        .await
    }
    pub async fn dismiss_client_live_activity(
        &self,
        client_id: &str,
        activity_id: &str,
        user_requested: bool,
    ) -> Result<bool, StoreError> {
        validate_activity_id(activity_id)?;
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let user_marker = "0".repeat(64);
            let changed = transaction.execute(
                "UPDATE client_task_activities SET lifecycle = 'dismissed', suppressed = 1, latest_projection_signature = CASE WHEN ?3 = 1 AND latest_projection_signature = '' THEN ?4 ELSE latest_projection_signature END, dismissed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND activity_id = ?2 AND lifecycle <> 'dismissed'",
                params![client_id, activity_id, user_requested, user_marker],
            )?;
            if changed == 1 {
                transaction.execute(
                    "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'activity_dismissed', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND activity_id = ?2 AND event <> 'end' AND status = 'pending'",
                    params![client_id, activity_id],
                )?;
            }
            transaction.commit()?;
            Ok(changed == 1)
        })
        .await
    }
    pub async fn live_activity_targets(&self) -> Result<Vec<LiveActivityTarget>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT r.client_id, r.push_to_start_token, r.environment, r.enabled, a.client_id, a.activity_id, a.task_session_id, a.lifecycle, a.update_token, a.latest_projection_json, a.latest_projection_signature, a.focused_task_id, a.session_started_at, a.suppressed, a.dismissed_at FROM client_live_activity_registrations r JOIN clients c USING (client_id) LEFT JOIN client_task_activities a USING (client_id) WHERE c.revoked_at IS NULL AND r.enabled = 1 ORDER BY r.client_id",
            )?;
            let rows = statement.query_map([], |row| {
                let registration = decode_registration(row, 0)?;
                let activity = if matches!(row.get_ref(4)?, rusqlite::types::ValueRef::Null) {
                    None
                } else {
                    Some(decode_activity(row, 4)?)
                };
                Ok(LiveActivityTarget {
                    registration,
                    activity,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
    pub async fn queue_live_activity_delivery(
        &self,
        delivery: NewLiveActivityDelivery,
    ) -> Result<(), StoreError> {
        validate_delivery(&delivery)?;
        let payload_json = serde_json::to_string(&delivery.payload)?;
        self.with_connection(|connection| {
            let status = if delivery.event == LiveActivityEvent::End
                || connection.query_row(
                    "SELECT EXISTS (SELECT 1 FROM client_task_activities a JOIN client_live_activity_registrations r USING (client_id) JOIN clients c USING (client_id) WHERE c.revoked_at IS NULL AND r.enabled = 1 AND r.environment = ?4 AND a.client_id = ?1 AND a.activity_id = ?2 AND a.suppressed = 0 AND ((?5 = 'start' AND a.lifecycle = 'starting' AND r.push_to_start_token = ?3) OR (?5 = 'update' AND a.lifecycle = 'active' AND a.update_token = ?3)))",
                    params![delivery.client_id, delivery.activity_id, delivery.token, delivery.environment.as_str(), delivery.event.as_str()],
                    |row| row.get::<_, bool>(0),
                )?
            {
                "pending"
            } else {
                "suppressed"
            };
            connection.execute(
                "INSERT INTO live_activity_deliveries (client_id, delivery_key, activity_id, token, environment, event, payload_json, urgency, ttl_seconds, status, available_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(client_id, delivery_key) DO UPDATE SET activity_id = CASE WHEN status = 'pending' THEN excluded.activity_id ELSE activity_id END, token = CASE WHEN status = 'pending' THEN excluded.token ELSE token END, environment = CASE WHEN status = 'pending' THEN excluded.environment ELSE environment END, event = CASE WHEN status = 'pending' THEN excluded.event ELSE event END, payload_json = CASE WHEN status = 'pending' THEN excluded.payload_json ELSE payload_json END, urgency = CASE WHEN status = 'pending' THEN excluded.urgency ELSE urgency END, ttl_seconds = CASE WHEN status = 'pending' THEN excluded.ttl_seconds ELSE ttl_seconds END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![
                    delivery.client_id,
                    delivery.delivery_key,
                    delivery.activity_id,
                    delivery.token,
                    delivery.environment.as_str(),
                    delivery.event.as_str(),
                    payload_json,
                    delivery.urgency,
                    i64::from(delivery.ttl_seconds),
                    status,
                ],
            )?;
            Ok(())
        })
        .await
    }
    pub async fn claim_due_live_activity_delivery(
        &self,
    ) -> Result<Option<ClaimedLiveActivityDelivery>, StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "DELETE FROM live_activity_deliveries WHERE event = 'end' AND created_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-600 seconds')",
                [],
            )?;
            transaction.execute(
                "UPDATE live_activity_deliveries SET status = 'suppressed', last_error_code = 'stale_delivery', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE status = 'pending' AND event <> 'end' AND NOT EXISTS (SELECT 1 FROM client_task_activities a JOIN client_live_activity_registrations r USING (client_id) JOIN clients c USING (client_id) WHERE c.revoked_at IS NULL AND r.enabled = 1 AND r.environment = live_activity_deliveries.environment AND a.client_id = live_activity_deliveries.client_id AND a.activity_id = live_activity_deliveries.activity_id AND a.suppressed = 0 AND ((live_activity_deliveries.event = 'start' AND a.lifecycle = 'starting' AND r.push_to_start_token = live_activity_deliveries.token) OR (live_activity_deliveries.event = 'update' AND a.lifecycle = 'active' AND a.update_token = live_activity_deliveries.token)))",
                [],
            )?;
            let key = transaction
                .query_row(
                    "SELECT client_id, delivery_key FROM live_activity_deliveries WHERE status = 'pending' AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ORDER BY available_at, client_id, delivery_key LIMIT 1",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((client_id, delivery_key)) = key else {
                return Ok(None);
            };
            transaction.execute(
                "UPDATE live_activity_deliveries SET attempt_count = attempt_count + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND delivery_key = ?2 AND status = 'pending'",
                params![client_id, delivery_key],
            )?;
            let delivery = transaction.query_row(
                "SELECT client_id, delivery_key, activity_id, token, environment, event, payload_json, urgency, ttl_seconds, created_at FROM live_activity_deliveries WHERE client_id = ?1 AND delivery_key = ?2 AND status = 'pending'",
                params![client_id, delivery_key],
                decode_delivery,
            )?;
            transaction.commit()?;
            Ok(Some(delivery))
        })
        .await
    }
    pub async fn finish_live_activity_delivery(
        &self,
        delivery: &ClaimedLiveActivityDelivery,
        disposition: &str,
        error_code: Option<&str>,
    ) -> Result<(), StoreError> {
        if !matches!(
            disposition,
            "delivered" | "suppressed" | "failed" | "retry" | "invalid_token"
        ) {
            return Err(invalid("invalid Live Activity delivery disposition"));
        }
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            match disposition {
                "invalid_token" => {
                    if delivery.event == LiveActivityEvent::Start {
                        transaction.execute(
                            "UPDATE client_live_activity_registrations SET push_to_start_token = NULL, environment = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND push_to_start_token = ?2",
                            params![delivery.client_id, delivery.token],
                        )?;
                    } else {
                        transaction.execute(
                            "UPDATE client_task_activities SET update_token = NULL, lifecycle = 'ending', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND update_token = ?2",
                            params![delivery.client_id, delivery.token],
                        )?;
                    }
                    terminal_delivery_tx(&transaction, delivery, "failed", Some("invalid_device_token"))?;
                }
                "retry" => {
                    let attempt: u32 = transaction.query_row(
                        "SELECT attempt_count FROM live_activity_deliveries WHERE client_id = ?1 AND delivery_key = ?2",
                        params![delivery.client_id, delivery.delivery_key],
                        |row| row.get(0),
                    )?;
                    let delay = match attempt {
                        0 | 1 => 60,
                        2 => 300,
                        _ => 1800,
                    };
                    let status = if attempt >= 4 { "failed" } else { "pending" };
                    transaction.execute(
                        "UPDATE live_activity_deliveries SET status = ?3, available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?4 || ' seconds'), last_error_code = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND delivery_key = ?2 AND status = 'pending' AND token = ?6",
                        params![delivery.client_id, delivery.delivery_key, status, delay, error_code, delivery.token],
                    )?;
                }
                _ => terminal_delivery_tx(&transaction, delivery, disposition, error_code)?,
            }
            if disposition == "delivered" && delivery.event == LiveActivityEvent::End {
                transaction.execute(
                    "UPDATE client_task_activities SET lifecycle = 'dismissed', suppressed = 1, latest_projection_json = CASE WHEN lifecycle = 'ending' THEN '{}' ELSE latest_projection_json END, latest_projection_signature = CASE WHEN lifecycle = 'ending' THEN '' ELSE latest_projection_signature END, focused_task_id = CASE WHEN lifecycle = 'ending' THEN NULL ELSE focused_task_id END, dismissed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND activity_id = ?2",
                    params![delivery.client_id, delivery.activity_id],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }
    pub async fn fail_pending_live_activity_deliveries(
        &self,
        error_code: &str,
    ) -> Result<(), StoreError> {
        if error_code.is_empty() || error_code.len() > 128 || error_code.trim() != error_code {
            return Err(invalid("invalid Live Activity delivery diagnostic"));
        }
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE live_activity_deliveries SET status = 'failed', last_error_code = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE status = 'pending'",
                [error_code],
            )?;
            Ok(())
        })
        .await
    }
    pub async fn notification_task_checkpoint(&self) -> Result<u64, StoreError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT task_notification_sequence FROM notification_projection_state WHERE state_id = 1",
                    [],
                    |row| row.get::<_, u64>(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
    }
    pub async fn list_task_notification_alerts(
        &self,
        after_sequence: u64,
        limit: u32,
    ) -> Result<Vec<TaskNotificationAlert>, StoreError> {
        let after = i64::try_from(after_sequence)
            .map_err(|_| invalid("notification sequence is outside SQLite range"))?;
        let limit = i64::from(limit.clamp(1, 100));
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT notification_id, event_sequence, notification_kind, payload_json FROM work_notification_outbox WHERE event_sequence > ?1 AND notification_kind IN ('task_waiting', 'task_recovery') ORDER BY event_sequence LIMIT ?2",
            )?;
            let rows = statement.query_map(params![after, limit], |row| {
                Ok(TaskNotificationAlert {
                    notification_id: row.get(0)?,
                    event_sequence: u64::try_from(row.get::<_, i64>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    notification_kind: row.get(2)?,
                    payload: serde_json::from_str(&row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        })
        .await
    }
    pub async fn advance_notification_task_checkpoint(
        &self,
        sequence: u64,
    ) -> Result<(), StoreError> {
        let sequence = i64::try_from(sequence)
            .map_err(|_| invalid("notification sequence is outside SQLite range"))?;
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE notification_projection_state SET task_notification_sequence = MAX(task_notification_sequence, ?1), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE state_id = 1",
                [sequence],
            )?;
            Ok(())
        })
        .await
    }
}

fn load_registration(
    connection: &rusqlite::Connection,
    client_id: &str,
) -> Result<Option<ClientLiveActivityRegistration>, StoreError> {
    connection
        .query_row(
            "SELECT r.client_id, r.push_to_start_token, r.environment, r.enabled FROM client_live_activity_registrations r JOIN clients c USING (client_id) WHERE r.client_id = ?1",
            [client_id],
            |row| decode_registration(row, 0),
        )
        .optional()
        .map_err(StoreError::Sqlite)
}
fn decode_registration(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<ClientLiveActivityRegistration> {
    Ok(ClientLiveActivityRegistration {
        client_id: row.get(offset)?,
        push_to_start_token: row.get(offset + 1)?,
        environment: row
            .get::<_, Option<String>>(offset + 2)?
            .map(ApnsEnvironment::parse)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        enabled: row.get::<_, i64>(offset + 3)? == 1,
    })
}
fn load_activity(
    connection: &rusqlite::Connection,
    client_id: &str,
) -> Result<Option<ClientTaskActivityRecord>, StoreError> {
    connection
        .query_row(
            "SELECT client_id, activity_id, task_session_id, lifecycle, update_token, latest_projection_json, latest_projection_signature, focused_task_id, session_started_at, suppressed, dismissed_at FROM client_task_activities WHERE client_id = ?1",
            [client_id],
            |row| decode_activity(row, 0),
        )
        .optional()
        .map_err(StoreError::Sqlite)
}
fn decode_activity(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<ClientTaskActivityRecord> {
    Ok(ClientTaskActivityRecord {
        client_id: row.get(offset)?,
        activity_id: row.get(offset + 1)?,
        task_session_id: row.get(offset + 2)?,
        lifecycle: row.get(offset + 3)?,
        update_token: row.get(offset + 4)?,
        latest_projection: serde_json::from_str(&row.get::<_, String>(offset + 5)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        latest_projection_signature: row.get(offset + 6)?,
        focused_task_id: row.get(offset + 7)?,
        session_started_at: row.get(offset + 8)?,
        suppressed: row.get::<_, i64>(offset + 9)? == 1,
        dismissed_at: row.get(offset + 10)?,
    })
}
fn decode_delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClaimedLiveActivityDelivery> {
    Ok(ClaimedLiveActivityDelivery {
        client_id: row.get(0)?,
        delivery_key: row.get(1)?,
        activity_id: row.get(2)?,
        token: row.get(3)?,
        environment: ApnsEnvironment::parse(row.get(4)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        event: match row.get::<_, String>(5)?.as_str() {
            "start" => LiveActivityEvent::Start,
            "update" => LiveActivityEvent::Update,
            "end" => LiveActivityEvent::End,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        payload: serde_json::from_str(&row.get::<_, String>(6)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        urgency: row.get(7)?,
        ttl_seconds: row.get::<_, u32>(8)?,
        created_at: row.get(9)?,
    })
}
fn terminal_delivery_tx(
    transaction: &Transaction<'_>,
    delivery: &ClaimedLiveActivityDelivery,
    status: &str,
    error_code: Option<&str>,
) -> Result<(), StoreError> {
    transaction.execute(
        "UPDATE live_activity_deliveries SET status = ?3, last_error_code = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE client_id = ?1 AND delivery_key = ?2 AND status = 'pending' AND token = ?5",
        params![delivery.client_id, delivery.delivery_key, status, error_code, delivery.token],
    )?;
    Ok(())
}
fn validate_delivery(delivery: &NewLiveActivityDelivery) -> Result<(), StoreError> {
    if delivery.client_id.trim().is_empty()
        || delivery.delivery_key.trim().is_empty()
        || delivery.delivery_key.len() > 256
        || delivery.token.is_empty()
        || delivery.token.len() > 1024
        || !matches!(delivery.urgency.as_str(), "normal" | "high")
        || delivery.ttl_seconds > 604_800
        || serde_json::to_vec(&delivery.payload)
            .map(|payload| payload.len() > 4096)
            .unwrap_or(true)
    {
        return Err(invalid("invalid Live Activity delivery"));
    }
    if delivery
        .activity_id
        .as_deref()
        .is_some_and(|activity_id| validate_activity_id(activity_id).is_err())
    {
        return Err(invalid("invalid Live Activity activity id"));
    }
    Ok(())
}

fn validate_activity_id(value: &str) -> Result<(), StoreError> {
    if !value.starts_with("live_activity:") || value.len() > 256 || value.trim() != value {
        Err(invalid("invalid Live Activity activity id"))
    } else {
        Ok(())
    }
}

fn validate_token(client_id: &str, token: &[u8]) -> Result<(), StoreError> {
    if client_id.trim().is_empty() || !(1..=1024).contains(&token.len()) {
        Err(invalid("invalid Live Activity token"))
    } else {
        Ok(())
    }
}

fn invalid(message: &str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
