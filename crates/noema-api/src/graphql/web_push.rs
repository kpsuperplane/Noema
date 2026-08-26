//! Installed-web notification registration, presence, projection, and delivery.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    apns::{
        APNS_TOPIC, ApnsCredential, ApnsSendError, GraphqlApnsProviderStatus,
        GraphqlClientLiveActivityStatus, GraphqlClientNotificationPresenceEvent,
        GraphqlClientNotificationStatus, GraphqlConfigureApnsProviderInput,
        GraphqlRegisterClientLiveActivitiesInput, GraphqlRegisterClientLiveActivityUpdateInput,
        GraphqlRegisterClientNotificationsInput, LIVE_ACTIVITY_ATTRIBUTES_TYPE,
        LIVE_ACTIVITY_TOPIC, client_status, delivery_outcome_apns, hex_digest, is_pkcs8_pem,
        live_activity_status, now_timestamp, positive_revision, read_apns_credential,
        validate_apns_credential, validate_apns_identifier, write_apns_credential,
    },
    errors::graphql_error,
    human_interventions::{self, GraphqlHumanIntervention},
    runtime_state::GraphqlState,
};
use async_graphql::{InputObject, Result, SimpleObject};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::Stream;
use noema_capabilities::web::url_policy::{is_public_ip, validate_public_url};
use noema_conversations::{ConversationItemKind, ReplayMode};
use noema_home::NoemaPaths;
use noema_runtime::{ConversationRuntimeEvent, TaskRuntimeEvent, WorkRuntimeEvent};
use noema_store::{
    ApnsEnvironment, ClaimedApnsDelivery, ClaimedLiveActivityDelivery, ClaimedWebPushDelivery,
    ClientLiveActivityRegistration, LiveActivityEvent, LiveActivityTarget, NewLiveActivityDelivery,
    NewWebPushSubscription, NoemaStore, NotificationDeliveryOutcome, WorkPageSize,
    WorkRunItemOwnerScope, WorkRunItemQuery, WorkTaskCursor, WorkTaskQuery, WorkTaskScope,
    WorkTaskSummary,
};
use noema_tasks::{
    AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, RunKind, RunStatus, TaskId,
    WorkflowStageBehavior,
};
use noema_workspaces::WorkspaceId;
use pulldown_cmark::{Event, Options, Parser, TagEnd};
use reqwest::{Client, redirect::Policy};
use ring::digest;
use serde_json::Value;
use tokio::sync::Notify;
use url::{Host, Url};
use web_push_native::{
    Auth, WebPushBuilder,
    jwt_simple::{
        algorithms::{ECDSAP256KeyPairLike, ECDSAP256PublicKeyLike, ES256KeyPair},
        claims::Claims,
    },
    p256::{PublicKey, elliptic_curve::sec1::ToEncodedPoint},
};
const LOCAL_HUMAN_ID: &str = "human:local";
const MAX_PREVIEW_BYTES: usize = 600;
const MAX_LIVE_UPDATE_BYTES: usize = 120;
const LIVE_ACTIVITY_DETAIL_DEBOUNCE: Duration = Duration::from_millis(500);
const NOTIFICATION_RECOVERY_INTERVAL: Duration = Duration::from_secs(30);

/// Browser-visible Web Push capability and registration state.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebPushStatus")]
pub struct GraphqlWebPushStatus {
    pub available: bool,
    pub blocker: Option<String>,
    pub application_server_key: Option<String>,
    pub subscription_id: Option<String>,
}
impl GraphqlWebPushStatus {
    pub(super) fn unavailable() -> Self {
        Self {
            available: false,
            blocker: Some("Web Push requires an HTTPS public origin".to_string()),
            application_server_key: None,
            subscription_id: None,
        }
    }
}
/// Exact browser subscription material returned by `PushSubscription.toJSON()`.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "RegisterWebPushSubscriptionInput")]
pub struct GraphqlRegisterWebPushSubscriptionInput {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}
/// Presence acknowledgement; the stream lifetime is the visibility lease.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebPushPresenceEvent")]
pub struct GraphqlWebPushPresenceEvent {
    pub subscription_id: String,
    pub ready: bool,
}
/// Installation-scoped notification projection, presence, and delivery service.
#[derive(Clone)]
pub struct NotificationCoordinator {
    inner: Arc<WebPushInner>,
}
struct WebPushInner {
    store: NoemaStore,
    paths: NoemaPaths,
    public_origin: String,
    identity: noema_store::WebPushIdentity,
    visible: Mutex<HashMap<PresenceKey, usize>>,
    apns_mutation: tokio::sync::Mutex<()>,
    live_activity_mutation: tokio::sync::Mutex<()>,
    apns_client: Client,
    apns_jwt: Mutex<Option<(u64, String, std::time::Instant)>>,
    wake: Notify,
    #[cfg(test)]
    live_reconciled: tokio::sync::Semaphore,
}
struct ApnsRequest<'a> {
    environment: ApnsEnvironment,
    device_token: &'a [u8],
    topic: &'a str,
    push_type: &'a str,
    urgency: &'a str,
    ttl_seconds: u32,
    created_at: &'a str,
    jwt: &'a str,
    payload: &'a serde_json::Value,
    collapse_id: Option<&'a str>,
}
impl std::fmt::Debug for NotificationCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NotificationCoordinator")
    }
}
impl NotificationCoordinator {
    #[doc = "Build the shared notification coordinator with its protected-file authority.\n\n# Errors\nReturns a bounded error when origin, identity, or HTTP client initialization fails."]
    pub async fn new_with_paths(
        store: NoemaStore,
        public_origin: String,
        paths: NoemaPaths,
    ) -> std::result::Result<Self, String> {
        let origin = Url::parse(&public_origin).map_err(|_| "invalid Web Push origin")?;
        if origin.scheme() != "https" || origin.host_str().is_none() {
            return Err("Web Push requires an HTTPS public origin".to_string());
        }
        let generated = ES256KeyPair::generate();
        let public_key = PublicKey::from_sec1_bytes(&generated.public_key().to_bytes())
            .map_err(|_| "invalid generated Web Push identity")?
            .to_encoded_point(false);
        let identity = store
            .get_or_insert_web_push_identity(&generated.to_bytes(), public_key.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        let apns_client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .http2_adaptive_window(true)
            .build()
            .map_err(|_| "APNs client unavailable")?;
        Ok(Self {
            inner: Arc::new(WebPushInner {
                store,
                paths,
                public_origin,
                identity,
                visible: Mutex::new(HashMap::new()),
                apns_mutation: tokio::sync::Mutex::new(()),
                live_activity_mutation: tokio::sync::Mutex::new(()),
                apns_client,
                apns_jwt: Mutex::new(None),
                wake: Notify::new(),
                #[cfg(test)]
                live_reconciled: tokio::sync::Semaphore::new(0),
            }),
        })
    }

    pub(super) async fn status(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        endpoint: Option<&str>,
    ) -> Result<GraphqlWebPushStatus> {
        let subscription = match endpoint {
            Some(endpoint) => self
                .inner
                .store
                .web_push_subscription_for_endpoint(owner_human_id, browser_session_hash, endpoint)
                .await
                .map_err(graphql_error)?,
            None => None,
        };
        Ok(GraphqlWebPushStatus {
            available: true,
            blocker: None,
            application_server_key: Some(URL_SAFE_NO_PAD.encode(&self.inner.identity.public_key)),
            subscription_id: subscription.map(|record| record.subscription_id),
        })
    }

    pub(super) async fn register(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        input: GraphqlRegisterWebPushSubscriptionInput,
    ) -> Result<GraphqlWebPushStatus> {
        validate_subscription_material(&input)?;
        let endpoint = validate_push_endpoint(&input.endpoint)?;
        self.inner
            .store
            .register_web_push_subscription(NewWebPushSubscription {
                owner_human_id: owner_human_id.to_string(),
                browser_session_hash,
                endpoint: endpoint.to_string(),
                p256dh: input.p256dh,
                auth_secret: input.auth,
            })
            .await
            .map_err(graphql_error)?;
        self.status(
            owner_human_id,
            browser_session_hash,
            Some(endpoint.as_str()),
        )
        .await
    }

    pub(super) async fn remove(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        subscription_id: &str,
    ) -> Result<bool> {
        self.inner
            .store
            .remove_web_push_subscription(owner_human_id, browser_session_hash, subscription_id)
            .await
            .map_err(graphql_error)
    }

    pub(super) async fn presence(
        &self,
        owner_human_id: &str,
        browser_session_hash: [u8; 32],
        subscription_id: String,
    ) -> Result<impl Stream<Item = GraphqlWebPushPresenceEvent> + use<>> {
        let ids = self
            .inner
            .store
            .web_push_subscription_ids(owner_human_id, browser_session_hash)
            .await
            .map_err(graphql_error)?;
        if !ids.iter().any(|candidate| candidate == &subscription_id) {
            return Err(async_graphql::Error::new(
                "Web Push subscription is unavailable",
            ));
        }
        let lease = VisibilityLease::new(
            self.inner.clone(),
            PresenceKey::Browser(subscription_id.clone()),
        );
        Ok(async_stream::stream! {
            let _lease = lease;
            yield GraphqlWebPushPresenceEvent { subscription_id, ready: true };
            std::future::pending::<()>().await;
        })
    }

    pub(super) async fn apns_provider_status(&self) -> Result<GraphqlApnsProviderStatus> {
        let credential = read_apns_credential(&self.inner.paths).map_err(graphql_error)?;
        Ok(credential.into())
    }

    pub(super) async fn configure_apns_provider(
        &self,
        input: GraphqlConfigureApnsProviderInput,
    ) -> Result<GraphqlApnsProviderStatus> {
        let expected_revision = positive_revision(input.expected_revision)?;
        let team_id = input.team_id.trim();
        let key_id = input.key_id.trim();
        validate_apns_identifier(team_id, "team ID")?;
        validate_apns_identifier(key_id, "key ID")?;
        if input.private_key_pem.len() > 16 * 1024 {
            return Err(async_graphql::Error::new("APNs private key is too large"));
        }
        if !is_pkcs8_pem(&input.private_key_pem) {
            return Err(async_graphql::Error::new("APNs private key is invalid"));
        }
        let key_pair = ES256KeyPair::from_pem(input.private_key_pem.trim())
            .map_err(|_| async_graphql::Error::new("APNs private key is invalid"))?;
        let fingerprint = hex_digest(
            digest::digest(
                &digest::SHA256,
                key_pair
                    .public_key()
                    .public_key()
                    .to_bytes_uncompressed()
                    .as_ref(),
            )
            .as_ref(),
        );
        let _guard = self.inner.apns_mutation.lock().await;
        let current = read_apns_credential(&self.inner.paths).map_err(graphql_error)?;
        if current.revision != expected_revision {
            return Err(async_graphql::Error::new("APNs provider revision conflict"));
        }
        let next = ApnsCredential {
            configured: true,
            team_id: Some(team_id.to_string()),
            key_id: Some(key_id.to_string()),
            topic: APNS_TOPIC.to_string(),
            key_fingerprint: Some(fingerprint),
            private_key_pem: Some(input.private_key_pem.trim().to_string()),
            revision: current.revision.saturating_add(1),
            updated_at: Some(now_timestamp()),
            last_error_code: None,
            last_error_at: None,
        };
        validate_apns_credential(&next).map_err(graphql_error)?;
        write_apns_credential(&self.inner.paths, &next).map_err(graphql_error)?;
        Ok(next.into())
    }

    pub(super) async fn remove_apns_provider(
        &self,
        expected_revision: i64,
    ) -> Result<GraphqlApnsProviderStatus> {
        let expected_revision = positive_revision(expected_revision)?;
        let _guard = self.inner.apns_mutation.lock().await;
        let current = read_apns_credential(&self.inner.paths).map_err(graphql_error)?;
        if current.revision != expected_revision {
            return Err(async_graphql::Error::new("APNs provider revision conflict"));
        }
        let next = ApnsCredential {
            configured: false,
            team_id: None,
            key_id: None,
            topic: APNS_TOPIC.to_string(),
            key_fingerprint: None,
            private_key_pem: None,
            revision: current.revision.saturating_add(1),
            updated_at: Some(now_timestamp()),
            last_error_code: None,
            last_error_at: None,
        };
        write_apns_credential(&self.inner.paths, &next).map_err(graphql_error)?;
        self.inner
            .store
            .fail_pending_apns_deliveries("provider_unconfigured")
            .await
            .map_err(graphql_error)?;
        self.inner
            .store
            .fail_pending_live_activity_deliveries("provider_unconfigured")
            .await
            .map_err(graphql_error)?;
        Ok(next.into())
    }

    pub(super) async fn client_notification_status(
        &self,
        client_id: &str,
    ) -> Result<GraphqlClientNotificationStatus> {
        let credential = read_apns_credential(&self.inner.paths).map_err(graphql_error)?;
        let registration = self
            .inner
            .store
            .client_notification_registration(client_id)
            .await
            .map_err(graphql_error)?;
        Ok(client_status(&credential, registration.as_ref()))
    }

    pub(super) async fn register_client_notifications(
        &self,
        client_id: &str,
        input: GraphqlRegisterClientNotificationsInput,
    ) -> Result<GraphqlClientNotificationStatus> {
        let token = URL_SAFE_NO_PAD
            .decode(input.device_token.as_bytes())
            .map_err(|_| async_graphql::Error::new("device token is invalid"))?;
        if token.is_empty() || token.len() > 1024 {
            return Err(async_graphql::Error::new("device token is invalid"));
        }
        self.inner
            .store
            .register_client_notifications(client_id, &token, input.environment.into())
            .await
            .map_err(graphql_error)?;
        self.client_notification_status(client_id).await
    }

    pub(super) async fn disable_client_notifications(
        &self,
        client_id: &str,
    ) -> Result<GraphqlClientNotificationStatus> {
        self.inner
            .store
            .disable_client_notifications(client_id)
            .await
            .map_err(graphql_error)?;
        self.client_notification_status(client_id).await
    }

    pub(super) async fn client_live_activity_status(
        &self,
        client_id: &str,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let credential = read_apns_credential(&self.inner.paths).map_err(graphql_error)?;
        let registration = self
            .inner
            .store
            .client_live_activity_registration(client_id)
            .await
            .map_err(graphql_error)?;
        Ok(live_activity_status(&credential, registration.as_ref()))
    }

    pub(super) async fn register_client_live_activities(
        &self,
        client_id: &str,
        input: GraphqlRegisterClientLiveActivitiesInput,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let _guard = self.inner.live_activity_mutation.lock().await;
        let token = decode_live_token(&input.push_to_start_token)?;
        self.inner
            .store
            .register_client_live_activities(
                client_id,
                &token,
                input.environment.into(),
                &input.active_activity_ids,
            )
            .await
            .map_err(graphql_error)?;
        self.reconcile_live_activities_for_client_locked(client_id)
            .await;
        self.client_live_activity_status(client_id).await
    }

    pub(super) async fn register_client_live_activity_update(
        &self,
        client_id: &str,
        input: GraphqlRegisterClientLiveActivityUpdateInput,
    ) -> Result<bool> {
        let _guard = self.inner.live_activity_mutation.lock().await;
        let token = decode_live_token(&input.update_token)?;
        let changed = self
            .inner
            .store
            .register_client_live_activity_update(client_id, &input.activity_id, &token)
            .await
            .map_err(graphql_error)?;
        if changed {
            self.reconcile_live_activities_for_client_locked(client_id)
                .await;
        }
        Ok(changed)
    }

    pub(super) async fn dismiss_client_live_activity(
        &self,
        client_id: &str,
        activity_id: &str,
    ) -> Result<bool> {
        let _guard = self.inner.live_activity_mutation.lock().await;
        let activity = self
            .inner
            .store
            .client_task_activity(client_id)
            .await
            .map_err(graphql_error)?;
        let changed = self
            .inner
            .store
            .dismiss_client_live_activity(client_id, activity_id, true)
            .await
            .map_err(graphql_error)?;
        if changed {
            if let Some(activity) = activity
                && let Ok(Some(registration)) = self
                    .inner
                    .store
                    .client_live_activity_registration(client_id)
                    .await
            {
                self.queue_live_end_for_registration(&registration, &activity)
                    .await;
            }
            self.inner.wake.notify_one();
        }
        Ok(changed)
    }

    pub(super) async fn disable_client_live_activities(
        &self,
        client_id: &str,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let _guard = self.inner.live_activity_mutation.lock().await;
        let registration = self
            .inner
            .store
            .client_live_activity_registration(client_id)
            .await
            .map_err(graphql_error)?;
        let activity = self
            .inner
            .store
            .client_task_activity(client_id)
            .await
            .map_err(graphql_error)?;
        if let (Some(registration), Some(activity)) = (registration.as_ref(), activity.as_ref()) {
            self.queue_live_end_for_registration(registration, activity)
                .await;
        }
        self.inner
            .store
            .disable_client_live_activities(client_id)
            .await
            .map_err(graphql_error)?;
        self.client_live_activity_status(client_id).await
    }

    pub(super) async fn revoke_client(
        &self,
        owner_human_id: &str,
        client_id: &str,
    ) -> std::result::Result<Option<noema_store::ClientRecord>, noema_store::StoreError> {
        let _guard = self.inner.live_activity_mutation.lock().await;
        let Ok(Some(registration)) = self
            .inner
            .store
            .client_live_activity_registration(client_id)
            .await
        else {
            return self
                .inner
                .store
                .revoke_client(owner_human_id, client_id)
                .await;
        };
        let Ok(Some(activity)) = self.inner.store.client_task_activity(client_id).await else {
            return self
                .inner
                .store
                .revoke_client(owner_human_id, client_id)
                .await;
        };
        self.queue_live_end_for_registration(&registration, &activity)
            .await;
        self.inner
            .store
            .revoke_client(owner_human_id, client_id)
            .await
    }

    pub(super) async fn client_presence(
        &self,
        client_id: String,
    ) -> Result<impl Stream<Item = GraphqlClientNotificationPresenceEvent> + use<>> {
        if self
            .inner
            .store
            .client_notification_registration(&client_id)
            .await
            .map_err(graphql_error)?
            .is_none()
        {
            return Err(async_graphql::Error::new(
                "client notification registration is unavailable",
            ));
        }
        let lease =
            VisibilityLease::new(self.inner.clone(), PresenceKey::Client(client_id.clone()));
        Ok(async_stream::stream! {
            let _lease = lease;
            yield GraphqlClientNotificationPresenceEvent { client_id, ready: true };
            std::future::pending::<()>().await;
        })
    }
    /// Project and deliver notification events until the runtime stream closes.
    pub async fn run(
        self,
        state: GraphqlState,
        mut events: tokio::sync::broadcast::Receiver<ConversationRuntimeEvent>,
        mut task_events: tokio::sync::broadcast::Receiver<TaskRuntimeEvent>,
        mut work_events: tokio::sync::broadcast::Receiver<WorkRuntimeEvent>,
    ) {
        let _ = self.reconcile_primary_chat(&state).await;
        let _ = self.reconcile_interventions(&state).await;
        self.reconcile_live_activities().await;
        let mut recovery = tokio::time::interval_at(
            tokio::time::Instant::now() + NOTIFICATION_RECOVERY_INTERVAL,
            NOTIFICATION_RECOVERY_INTERVAL,
        );
        recovery.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let detail_delay = tokio::time::sleep(LIVE_ACTIVITY_DETAIL_DEBOUNCE);
        tokio::pin!(detail_delay);
        let mut detail_dirty = false;
        loop {
            let reconcile_all = tokio::select! {
                event = events.recv() => match event {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                event = task_events.recv() => match event {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        detail_dirty = true;
                        detail_delay.as_mut().reset(
                            tokio::time::Instant::now() + LIVE_ACTIVITY_DETAIL_DEBOUNCE,
                        );
                        false
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                event = work_events.recv() => match event {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                () = self.inner.wake.notified() => true,
                _ = recovery.tick() => true,
                () = &mut detail_delay, if detail_dirty => {
                    let mut received_more = false;
                    loop {
                        match task_events.try_recv() {
                            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                                received_more = true;
                            }
                            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                        }
                    }
                    if received_more {
                        detail_delay.as_mut().reset(
                            tokio::time::Instant::now() + LIVE_ACTIVITY_DETAIL_DEBOUNCE,
                        );
                    } else {
                        detail_dirty = false;
                        self.reconcile_live_activities().await;
                        self.drain_due().await;
                    }
                    false
                },
            };
            if !reconcile_all {
                continue;
            }
            while matches!(
                task_events.try_recv(),
                Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
            ) {}
            detail_dirty = false;
            let _ = self.reconcile_primary_chat(&state).await;
            let _ = self.reconcile_interventions(&state).await;
            self.reconcile_live_activities().await;
            self.drain_due().await;
        }
    }
    async fn reconcile_primary_chat(&self, state: &GraphqlState) -> Result<()> {
        let conversation = self
            .inner
            .store
            .primary_conversation_for_human(LOCAL_HUMAN_ID)
            .await
            .map_err(graphql_error)?;
        let Some(conversation) = conversation else {
            return Ok(());
        };
        let items = self
            .inner
            .store
            .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
            .await
            .map_err(graphql_error)?;
        let latest_sequence = items.last().map_or(0, |item| item.sequence_index);
        let checkpoint = self
            .inner
            .store
            .notification_primary_checkpoint()
            .await
            .map_err(graphql_error)?;
        if checkpoint.conversation_id.as_deref() != Some(&conversation.conversation_id) {
            self.inner
                .store
                .advance_notification_primary_checkpoint(
                    &conversation.conversation_id,
                    latest_sequence,
                )
                .await
                .map_err(graphql_error)?;
            return Ok(());
        }
        let title = primary_agent_name(state).await;
        let visible = self.visible_subscriptions();
        for item in items
            .iter()
            .filter(|item| item.sequence_index > checkpoint.sequence)
        {
            if let Some((event_key, body)) = primary_chat_notification(item) {
                self.queue_notification(&event_key, &title, &body, "normal", 3600, &visible)
                    .await?;
            }
        }
        self.inner
            .store
            .advance_notification_primary_checkpoint(&conversation.conversation_id, latest_sequence)
            .await
            .map_err(graphql_error)?;
        Ok(())
    }
    async fn reconcile_interventions(&self, state: &GraphqlState) -> Result<()> {
        let conversation_id = self
            .inner
            .store
            .primary_conversation_for_human(LOCAL_HUMAN_ID)
            .await
            .map_err(graphql_error)?
            .map(|record| record.conversation_id);
        let Some(conversation_id) = conversation_id else {
            return Ok(());
        };
        let interventions = human_interventions::pending_human_interventions(
            state,
            LOCAL_HUMAN_ID,
            Some(conversation_id),
            None,
            None,
            Some(100),
        )
        .await?;
        let seeded = self
            .inner
            .store
            .notification_attention_seeded()
            .await
            .map_err(graphql_error)?;
        let visible = self.visible_subscriptions();
        for intervention in interventions {
            let (key, title, body) = intervention_notification(&intervention);
            let is_new = self
                .inner
                .store
                .observe_notification_attention(&key)
                .await
                .map_err(graphql_error)?;
            if seeded && is_new {
                self.queue_notification(
                    &format!("attention:{key}"),
                    &title,
                    &body,
                    "high",
                    86_400,
                    &visible,
                )
                .await?;
            }
        }
        if !seeded {
            self.inner
                .store
                .mark_notification_attention_seeded()
                .await
                .map_err(graphql_error)?;
        }
        Ok(())
    }
    async fn queue_notification(
        &self,
        event_key: &str,
        title: &str,
        body: &str,
        urgency: &str,
        ttl_seconds: u32,
        visible_web: &HashSet<String>,
    ) -> Result<()> {
        let apns_enabled =
            read_apns_credential(&self.inner.paths).is_ok_and(|credential| credential.configured);
        let visible_clients = self.visible_clients();
        let title = notification_text(title);
        let body = notification_text(body);
        let fanout = self
            .inner
            .store
            .queue_notification_fanout(
                LOCAL_HUMAN_ID,
                event_key,
                &title,
                &body,
                urgency,
                ttl_seconds,
                visible_web,
                &visible_clients,
                apns_enabled,
            )
            .await;
        fanout.map_err(graphql_error)?;
        self.inner.wake.notify_one();
        Ok(())
    }
    async fn drain_due(&self) {
        while let Ok(Some(delivery)) = self.inner.store.claim_due_live_activity_delivery().await {
            let _guard = self.inner.live_activity_mutation.lock().await;
            if delivery.event != LiveActivityEvent::End
                && !self.live_delivery_is_current(&delivery).await
            {
                let _ = self
                    .inner
                    .store
                    .finish_live_activity_delivery(
                        &delivery,
                        NotificationDeliveryOutcome::Suppressed(Some("stale_start")),
                        None,
                    )
                    .await;
                continue;
            }
            let (revision, result, apns_id) = self.send_live_activity(delivery.clone()).await;
            let transport_error = matches!(&result, Err(ApnsSendError::Transport(_)));
            let mut outcome = delivery_outcome_apns(result);
            if delivery.event == LiveActivityEvent::Start && transport_error {
                outcome = NotificationDeliveryOutcome::Failed(outcome.error_code());
            }
            if let Some(code) = outcome.error_code()
                && matches!(
                    outcome,
                    NotificationDeliveryOutcome::Failed(_) | NotificationDeliveryOutcome::Retry(_)
                )
            {
                let _ = self.record_apns_error(code, revision).await;
            }
            let _ = self
                .inner
                .store
                .finish_live_activity_delivery(&delivery, outcome, apns_id.as_deref())
                .await;
        }
        while let Ok(Some(delivery)) = self.inner.store.claim_due_apns_delivery().await {
            if delivery.route == "chat" && self.is_client_visible(&delivery.client.client_id) {
                let _ = self
                    .inner
                    .store
                    .finish_apns_delivery(
                        &delivery.client.client_id,
                        &delivery.event_key,
                        &delivery.client.device_token,
                        NotificationDeliveryOutcome::Suppressed(None),
                    )
                    .await;
                continue;
            }
            let (revision, result, _) = self.send_apns(delivery.clone()).await;
            let outcome = delivery_outcome_apns(result);
            if let Some(code) = outcome.error_code()
                && matches!(
                    outcome,
                    NotificationDeliveryOutcome::Failed(_) | NotificationDeliveryOutcome::Retry(_)
                )
            {
                let _ = self.record_apns_error(code, revision).await;
            }
            let _ = self
                .inner
                .store
                .finish_apns_delivery(
                    &delivery.client.client_id,
                    &delivery.event_key,
                    &delivery.client.device_token,
                    outcome,
                )
                .await;
        }
        while let Ok(Some(delivery)) = self.inner.store.claim_due_web_push_delivery().await {
            if self.is_visible(&delivery.subscription.subscription_id) {
                let _ = self
                    .inner
                    .store
                    .finish_web_push_delivery(
                        &delivery.subscription.subscription_id,
                        &delivery.event_key,
                        NotificationDeliveryOutcome::Suppressed(None),
                    )
                    .await;
                continue;
            }
            let outcome = delivery_outcome(self.send(delivery.clone()).await);
            let _ = self
                .inner
                .store
                .finish_web_push_delivery(
                    &delivery.subscription.subscription_id,
                    &delivery.event_key,
                    outcome,
                )
                .await;
        }
    }
    async fn send(
        &self,
        delivery: ClaimedWebPushDelivery,
    ) -> std::result::Result<reqwest::StatusCode, String> {
        let endpoint = validate_push_endpoint(&delivery.subscription.endpoint)
            .map_err(|error| error.message)?;
        let public_key = PublicKey::from_sec1_bytes(
            &URL_SAFE_NO_PAD
                .decode(delivery.subscription.p256dh.as_bytes())
                .map_err(|_| "invalid p256dh")?,
        )
        .map_err(|_| "invalid p256dh")?;
        let auth = URL_SAFE_NO_PAD
            .decode(delivery.subscription.auth_secret.as_bytes())
            .map_err(|_| "invalid auth")?;
        let key_pair = ES256KeyPair::from_bytes(&self.inner.identity.private_key)
            .map_err(|_| "invalid VAPID key")?;
        let payload = declarative_payload(
            &delivery.title,
            &delivery.body,
            &format!("{}{}", self.inner.public_origin, delivery.navigate_path),
            &delivery.event_key,
        )?;
        let mut request = WebPushBuilder::new(
            endpoint.as_str().parse().map_err(|_| "invalid endpoint")?,
            public_key,
            Auth::clone_from_slice(&auth),
        )
        .with_vapid(&key_pair, &self.inner.public_origin)
        .build(payload)
        .map_err(|_| "push encryption failed")?;
        request
            .headers_mut()
            .insert("ttl", delivery.ttl_seconds.into());
        request.headers_mut().insert(
            "urgency",
            delivery.urgency.parse().map_err(|_| "invalid urgency")?,
        );
        let client = checked_client(&endpoint).await?;
        let (parts, body) = request.into_parts();
        client
            .request(parts.method, endpoint)
            .headers(parts.headers)
            .body(body)
            .send()
            .await
            .map(|response| response.status())
            .map_err(|_| "push transport failed".to_string())
    }
    fn visible_subscriptions(&self) -> HashSet<String> {
        self.inner.visible.lock().map_or_else(
            |_| HashSet::new(),
            |visible| {
                visible
                    .keys()
                    .filter_map(|key| match key {
                        PresenceKey::Browser(id) => Some(id.clone()),
                        PresenceKey::Client(_) => None,
                    })
                    .collect()
            },
        )
    }
    fn is_visible(&self, subscription_id: &str) -> bool {
        self.inner.visible.lock().is_ok_and(|visible| {
            visible.contains_key(&PresenceKey::Browser(subscription_id.to_string()))
        })
    }
    fn visible_clients(&self) -> HashSet<String> {
        self.inner.visible.lock().map_or_else(
            |_| HashSet::new(),
            |visible| {
                visible
                    .keys()
                    .filter_map(|key| match key {
                        PresenceKey::Browser(_) => None,
                        PresenceKey::Client(id) => Some(id.clone()),
                    })
                    .collect()
            },
        )
    }
    fn is_client_visible(&self, client_id: &str) -> bool {
        self.inner
            .visible
            .lock()
            .is_ok_and(|visible| visible.contains_key(&PresenceKey::Client(client_id.to_string())))
    }
    async fn live_delivery_is_current(&self, delivery: &ClaimedLiveActivityDelivery) -> bool {
        let Some(activity_id) = delivery.activity_id.as_deref() else {
            return false;
        };
        let Ok(Some(activity)) = self
            .inner
            .store
            .client_task_activity(&delivery.client_id)
            .await
        else {
            return false;
        };
        if activity.activity_id.as_deref() != Some(activity_id) || activity.suppressed {
            return false;
        }
        let Ok(Some(registration)) = self
            .inner
            .store
            .client_live_activity_registration(&delivery.client_id)
            .await
        else {
            return false;
        };
        registration.enabled
            && registration.environment == Some(delivery.environment)
            && match delivery.event {
                LiveActivityEvent::Start => {
                    activity.lifecycle == "starting"
                        && registration.push_to_start_token.as_deref()
                            == Some(delivery.token.as_slice())
                }
                LiveActivityEvent::Update => {
                    activity.lifecycle == "active"
                        && activity.update_token.as_deref() == Some(delivery.token.as_slice())
                }
                LiveActivityEvent::End => true,
            }
    }
    async fn send_apns(
        &self,
        delivery: ClaimedApnsDelivery,
    ) -> (
        Option<u64>,
        std::result::Result<reqwest::StatusCode, ApnsSendError>,
        Option<String>,
    ) {
        let credential = match read_apns_credential(&self.inner.paths) {
            Ok(credential) => credential,
            Err(_) => {
                return (
                    None,
                    Err(ApnsSendError::Transport("provider_unavailable")),
                    None,
                );
            }
        };
        let revision = Some(credential.revision);
        if !credential.configured {
            return (
                revision,
                Err(ApnsSendError::Provider("provider_unconfigured")),
                None,
            );
        }
        let mut payload = serde_json::json!({
            "aps": {
                "alert": {"title": delivery.title, "body": delivery.body},
                "sound": "default"
            },
            "route": delivery.route,
            "version": 1,
            "eventKey": delivery.event_key,
            "clientId": delivery.client.client_id,
        });
        if let Some(task_id) = delivery.task_id {
            payload["taskId"] = serde_json::Value::String(task_id);
        }
        let token = match self.apns_jwt(&credential) {
            Ok(token) => token,
            Err(error) => return (revision, Err(error), None),
        };
        let (result, apns_id) = self
            .send_apns_request(ApnsRequest {
                environment: delivery.client.environment,
                device_token: &delivery.client.device_token,
                topic: APNS_TOPIC,
                push_type: "alert",
                urgency: &delivery.urgency,
                ttl_seconds: delivery.ttl_seconds,
                created_at: &delivery.created_at,
                jwt: &token,
                payload: &payload,
                collapse_id: None,
            })
            .await;
        (revision, result, apns_id)
    }
    async fn send_live_activity(
        &self,
        delivery: ClaimedLiveActivityDelivery,
    ) -> (
        Option<u64>,
        std::result::Result<reqwest::StatusCode, ApnsSendError>,
        Option<String>,
    ) {
        let credential = match read_apns_credential(&self.inner.paths) {
            Ok(credential) => credential,
            Err(_) => {
                return (
                    None,
                    Err(ApnsSendError::Transport("provider_unavailable")),
                    None,
                );
            }
        };
        let revision = Some(credential.revision);
        if !credential.configured {
            return (
                revision,
                Err(ApnsSendError::Provider("provider_unconfigured")),
                None,
            );
        }
        let token = match self.apns_jwt(&credential) {
            Ok(token) => token,
            Err(error) => return (revision, Err(error), None),
        };
        let collapse_id = live_activity_collapse_id(
            delivery.event,
            &delivery.urgency,
            delivery.activity_id.as_deref(),
        );
        let (result, apns_id) = self
            .send_apns_request(ApnsRequest {
                environment: delivery.environment,
                device_token: &delivery.token,
                topic: LIVE_ACTIVITY_TOPIC,
                push_type: "liveactivity",
                urgency: &delivery.urgency,
                ttl_seconds: delivery.ttl_seconds,
                created_at: &delivery.created_at,
                jwt: &token,
                payload: &delivery.payload,
                collapse_id: collapse_id.as_deref(),
            })
            .await;
        (revision, result, apns_id)
    }
    fn apns_jwt(&self, credential: &ApnsCredential) -> std::result::Result<String, ApnsSendError> {
        let team_id = credential
            .team_id
            .as_deref()
            .ok_or(ApnsSendError::Provider("provider_metadata_invalid"))?;
        let key_id = credential
            .key_id
            .as_deref()
            .ok_or(ApnsSendError::Provider("provider_metadata_invalid"))?;
        if let Ok(cache) = self.inner.apns_jwt.lock()
            && let Some((revision, token, expires_at)) = cache.as_ref()
            && *revision == credential.revision
            && *expires_at > std::time::Instant::now()
        {
            return Ok(token.clone());
        }
        let private_key = credential
            .private_key_pem
            .as_deref()
            .ok_or(ApnsSendError::Provider("provider_key_unavailable"))?;
        let key_pair = ES256KeyPair::from_pem(private_key)
            .map_err(|_| ApnsSendError::Provider("provider_key_invalid"))?
            .with_key_id(key_id);
        let mut claims = Claims::create(Duration::from_secs(55 * 60).into()).with_issuer(team_id);
        claims.expires_at = None;
        claims.invalid_before = None;
        let token = key_pair
            .sign(claims)
            .map_err(|_| ApnsSendError::Provider("provider_token_failed"))?;
        if let Ok(mut cache) = self.inner.apns_jwt.lock() {
            *cache = Some((
                credential.revision,
                token.clone(),
                std::time::Instant::now() + Duration::from_secs(45 * 60),
            ));
        }
        Ok(token)
    }
    async fn send_apns_request(
        &self,
        apns: ApnsRequest<'_>,
    ) -> (
        std::result::Result<reqwest::StatusCode, ApnsSendError>,
        Option<String>,
    ) {
        let token_hex = apns
            .device_token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let host = match apns.environment {
            ApnsEnvironment::Development => "api.sandbox.push.apple.com",
            ApnsEnvironment::Production => "api.push.apple.com",
        };
        let expiration = if apns.ttl_seconds == 0 {
            "0".to_string()
        } else {
            let created_at = match chrono::DateTime::parse_from_rfc3339(apns.created_at) {
                Ok(value) => value.timestamp(),
                Err(_) => {
                    return (Err(ApnsSendError::Provider("invalid_delivery_time")), None);
                }
            };
            let expiration = created_at.saturating_add(i64::from(apns.ttl_seconds));
            if expiration <= chrono::Utc::now().timestamp() {
                return (Err(ApnsSendError::Provider("delivery_expired")), None);
            }
            expiration.to_string()
        };
        let mut request = self
            .inner
            .apns_client
            .post(format!("https://{host}/3/device/{token_hex}"))
            .header("authorization", format!("bearer {}", apns.jwt))
            .header("apns-topic", apns.topic)
            .header("apns-push-type", apns.push_type)
            .header(
                "apns-priority",
                if apns.urgency == "high" { "10" } else { "5" },
            )
            .header("apns-expiration", expiration);
        if let Some(collapse_id) = apns.collapse_id {
            request = request.header("apns-collapse-id", collapse_id);
        }
        let response = match request.json(apns.payload).send().await {
            Ok(response) => response,
            Err(_) => {
                return (Err(ApnsSendError::Transport("transport_unavailable")), None);
            }
        };
        let apns_id = response
            .headers()
            .get("apns-id")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty() && value.len() <= 128 && value.is_ascii())
            .map(str::to_owned);
        let status = response.status();
        if matches!(status.as_u16(), 400 | 410) {
            let reason = response
                .bytes()
                .await
                .ok()
                .and_then(|body| {
                    (body.len() <= 4096)
                        .then(|| serde_json::from_slice::<serde_json::Value>(&body).ok())
                })
                .flatten()
                .and_then(|value| {
                    value
                        .get("reason")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            if matches!(
                reason.as_str(),
                "BadDeviceToken" | "DeviceTokenNotForTopic" | "Unregistered"
            ) {
                return (Err(ApnsSendError::InvalidToken), apns_id);
            }
        }
        (Ok(status), apns_id)
    }
    async fn record_apns_error(
        &self,
        code: &str,
        revision: Option<u64>,
    ) -> Result<(), noema_store::StoreError> {
        let Some(revision) = revision else {
            return Ok(());
        };
        let _guard = self.inner.apns_mutation.lock().await;
        let mut credential = read_apns_credential(&self.inner.paths)
            .map_err(|error| noema_store::StoreError::InvariantViolation { message: error })?;
        if credential.revision != revision {
            return Ok(());
        }
        credential.last_error_code = Some(code.chars().take(128).collect());
        credential.last_error_at = Some(now_timestamp());
        credential.updated_at = credential.last_error_at.clone();
        write_apns_credential(&self.inner.paths, &credential)
            .map_err(|error| noema_store::StoreError::InvariantViolation { message: error })
    }
    async fn reconcile_live_activities_for_client_locked(&self, client_id: &str) {
        let Some(targets) = self.live_activity_targets().await else {
            return;
        };
        let Some(target) = targets
            .into_iter()
            .find(|target| target.registration.client_id == client_id)
        else {
            return;
        };
        let projection = live_projection(&self.inner.store).await;
        self.apply_live_projection(target, &projection).await;
    }
    async fn reconcile_live_activities(&self) {
        let _guard = self.inner.live_activity_mutation.lock().await;
        self.reconcile_live_activities_locked().await;
        #[cfg(test)]
        self.inner.live_reconciled.add_permits(1);
    }
    async fn reconcile_live_activities_locked(&self) {
        let Some(targets) = self.live_activity_targets().await else {
            return;
        };
        let projection = live_projection(&self.inner.store).await;
        for target in targets {
            self.apply_live_projection(target, &projection).await;
        }
        self.reconcile_task_alerts().await;
    }
    async fn live_activity_targets(&self) -> Option<Vec<LiveActivityTarget>> {
        read_apns_credential(&self.inner.paths)
            .ok()
            .filter(|credential| credential.configured)?;
        self.inner.store.live_activity_targets().await.ok()
    }
    async fn apply_live_target(
        &self,
        mut target: LiveActivityTarget,
        projection: Option<LiveProjection>,
    ) {
        let Some(projection) = projection else {
            let Some(activity) = target.activity.as_ref() else {
                return;
            };
            let Some(_environment) = target.registration.environment else {
                return;
            };
            let end_projection = self
                .terminal_projection(activity)
                .await
                .or_else(|| activity_projection(activity));
            if let Some(token) = activity.update_token.as_ref()
                && let Some(projection) = end_projection.as_ref()
                && self
                    .queue_live_end_projection(&target.registration, activity, token, projection)
                    .await
            {
                let _ = self
                    .inner
                    .store
                    .mark_client_task_activity_ending(
                        &target.registration.client_id,
                        activity.activity_id.as_deref(),
                    )
                    .await;
            } else if activity.lifecycle == "dismissed" {
                let _ = self
                    .inner
                    .store
                    .clear_client_task_activity_dismissal(&target.registration.client_id)
                    .await;
            }
            return;
        };
        loop {
            let Some(activity) = target.activity.as_ref() else {
                return;
            };
            if !matches!(activity.lifecycle.as_str(), "starting" | "active") {
                if activity.lifecycle == "ending" {
                    return;
                }
                if activity.lifecycle == "dismissed"
                    && !activity.latest_projection_signature.is_empty()
                {
                    if activity
                        .latest_projection_signature
                        .bytes()
                        .all(|byte| byte == b'0')
                        || activity.focused_task_id.as_deref()
                            == Some(projection.focus_task_id.as_str())
                    {
                        return;
                    }
                    let _ = self
                        .inner
                        .store
                        .clear_client_task_activity_dismissal(&target.registration.client_id)
                        .await;
                }
                let client_id = target.registration.client_id.clone();
                if self
                    .inner
                    .store
                    .ensure_client_task_activity_session(&client_id)
                    .await
                    .is_err()
                {
                    return;
                }
                let Ok(targets) = self.inner.store.live_activity_targets().await else {
                    return;
                };
                let Some(next_target) = targets
                    .into_iter()
                    .find(|candidate| candidate.registration.client_id == client_id)
                else {
                    return;
                };
                target = next_target;
                continue;
            }
            let _ = self
                .inner
                .store
                .update_client_task_activity_projection(
                    &target.registration.client_id,
                    &projection.content,
                    &projection.signature,
                    Some(&projection.focus_task_id),
                )
                .await;
            if let Some(token) = activity.update_token.as_ref()
                && activity.lifecycle == "active"
            {
                self.queue_live_delivery(
                    &target.registration,
                    activity,
                    token,
                    LiveActivityEvent::Update,
                    format!("live:update:{}", projection.signature),
                    &projection,
                    None,
                    3600,
                )
                .await;
            } else if let Some(token) = target.registration.push_to_start_token.as_ref()
                && activity.lifecycle == "starting"
            {
                self.queue_live_delivery(
                    &target.registration,
                    activity,
                    token,
                    LiveActivityEvent::Start,
                    format!("live:start:{}", activity.task_session_id),
                    &projection,
                    None,
                    3600,
                )
                .await;
            }
            return;
        }
    }

    async fn apply_live_projection(
        &self,
        target: LiveActivityTarget,
        projection: &std::result::Result<Option<LiveProjection>, noema_store::StoreError>,
    ) {
        let Ok(projection) = projection else {
            return;
        };
        self.apply_live_target(target, projection.clone()).await;
    }
    async fn reconcile_task_alerts(&self) {
        let Ok(mut checkpoint) = self.inner.store.notification_task_checkpoint().await else {
            return;
        };
        let Ok(targets) = self.inner.store.live_activity_targets().await else {
            return;
        };
        let fallback_projection = live_projection(&self.inner.store).await.ok().flatten();
        loop {
            let Ok(alerts) = self
                .inner
                .store
                .list_task_notification_alerts(checkpoint, 100)
                .await
            else {
                return;
            };
            if alerts.is_empty() {
                break;
            }
            for alert in &alerts {
                let task_id = alert
                    .payload
                    .get("task_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("task:unknown");
                let title = if alert.notification_kind == "task_recovery" {
                    "Task recovery needed"
                } else {
                    "Task waiting"
                };
                let body = if let Some(title) = alert
                    .payload
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                {
                    title.to_string()
                } else {
                    match TaskId::new(task_id.to_string()) {
                        Ok(task_id) => self
                            .inner
                            .store
                            .get_work_task(&task_id)
                            .await
                            .ok()
                            .flatten()
                            .map_or_else(|| task_id.to_string(), |task| task.task.title),
                        Err(_) => task_id.to_string(),
                    }
                };
                let notification_title = notification_text(title);
                let notification_body = notification_text(&body);
                if self
                    .inner
                    .store
                    .queue_task_notification_fanout(
                        LOCAL_HUMAN_ID,
                        &format!("task-alert:{}", alert.notification_id),
                        &notification_title,
                        &notification_body,
                        "high",
                        86_400,
                        task_id,
                        &self.visible_subscriptions(),
                        &HashSet::new(),
                        true,
                    )
                    .await
                    .is_err()
                {
                    return;
                }
                for target in &targets {
                    let Some(activity) = target.activity.as_ref() else {
                        continue;
                    };
                    let (event, token) = if let Some(token) = activity.update_token.as_ref()
                        && activity.lifecycle == "active"
                    {
                        (LiveActivityEvent::Update, token.clone())
                    } else {
                        continue;
                    };
                    let Some(projection) =
                        activity_projection(activity).or_else(|| fallback_projection.clone())
                    else {
                        continue;
                    };
                    self.queue_live_delivery(
                        &target.registration,
                        activity,
                        &token,
                        event,
                        format!("live:alert:{}", alert.notification_id),
                        &projection,
                        Some((title, &body, task_id)),
                        86_400,
                    )
                    .await;
                }
                checkpoint = alert.event_sequence;
            }
            let _ = self
                .inner
                .store
                .advance_notification_task_checkpoint(checkpoint)
                .await;
            if alerts.len() < 100 {
                break;
            }
        }
    }
    async fn queue_live_end_for_registration(
        &self,
        registration: &ClientLiveActivityRegistration,
        activity: &noema_store::ClientTaskActivityRecord,
    ) {
        let Some(token) = activity.update_token.as_ref() else {
            return;
        };
        let projection = LiveProjection {
            content: activity.latest_projection.clone(),
            signature: activity.latest_projection_signature.clone(),
            focus_task_id: activity
                .focused_task_id
                .clone()
                .unwrap_or_else(|| "task:unknown".to_string()),
        };
        self.queue_live_end_projection(registration, activity, token, &projection)
            .await;
    }
    async fn queue_live_end_projection(
        &self,
        registration: &ClientLiveActivityRegistration,
        activity: &noema_store::ClientTaskActivityRecord,
        token: &[u8],
        projection: &LiveProjection,
    ) -> bool {
        self.queue_live_delivery(
            registration,
            activity,
            token,
            LiveActivityEvent::End,
            format!("live:end:{}", activity.task_session_id),
            projection,
            None,
            600,
        )
        .await
    }
    async fn queue_live_delivery(
        &self,
        registration: &ClientLiveActivityRegistration,
        activity: &noema_store::ClientTaskActivityRecord,
        token: &[u8],
        event: LiveActivityEvent,
        delivery_key: String,
        projection: &LiveProjection,
        alert: Option<(&str, &str, &str)>,
        ttl_seconds: u32,
    ) -> bool {
        let Some(environment) = registration.environment else {
            return false;
        };
        let payload = live_activity_payload(
            event,
            &registration.client_id,
            activity.activity_id.as_deref(),
            &self.inner.public_origin,
            projection,
            alert,
        );
        let urgency = live_activity_urgency(event, alert.is_some());
        self.inner
            .store
            .queue_live_activity_delivery(NewLiveActivityDelivery {
                client_id: registration.client_id.clone(),
                delivery_key,
                activity_id: activity.activity_id.clone(),
                token: token.to_vec(),
                environment,
                event,
                payload,
                urgency: urgency.to_string(),
                ttl_seconds,
            })
            .await
            .is_ok()
    }

    async fn terminal_projection(
        &self,
        activity: &noema_store::ClientTaskActivityRecord,
    ) -> Option<LiveProjection> {
        let task_id = TaskId::new(activity.focused_task_id.clone()?).ok()?;
        let detail = self.inner.store.get_work_task(&task_id).await.ok()??;
        let phase = match detail.stage.system_behavior {
            WorkflowStageBehavior::TerminalSuccess => "completed",
            WorkflowStageBehavior::TerminalCancelled => "cancelled",
            _ => return None,
        };
        let agent_id = detail
            .current_run
            .as_ref()
            .map_or(detail.task.executor_agent_id.as_str(), |run| {
                run.agent_id.as_str()
            });
        let agent_name = self
            .inner
            .store
            .get_agent(agent_id)
            .await
            .ok()
            .flatten()
            .and_then(|agent| agent.display_name)
            .unwrap_or_else(|| "Agent".to_string());
        let completed_output_count =
            (!detail.artifacts.is_empty()).then_some(detail.artifacts.len());
        Some(LiveProjection::terminal(
            task_id.as_str(),
            &detail.task.title,
            detail.project.as_ref().map(|project| project.name.as_str()),
            &agent_name,
            phase,
            completed_output_count,
            parse_epoch(&detail.task.updated_at)
                .unwrap_or_else(|| chrono::Utc::now().timestamp() as f64),
        ))
    }
}

#[derive(Clone)]
struct LiveProjection {
    content: serde_json::Value,
    signature: String,
    focus_task_id: String,
}

impl LiveProjection {
    fn terminal(
        task_id: &str,
        title: &str,
        project_name: Option<&str>,
        agent_name: &str,
        phase: &str,
        completed_output_count: Option<usize>,
        updated_at: f64,
    ) -> Self {
        let content = serde_json::json!({
            "focusTaskId": task_id,
            "focusTitle": notification_text(title),
            "projectName": project_name,
            "agentName": notification_text(agent_name),
            "phase": phase,
            "statusLabel": if phase == "completed" { "Completed" } else { "Cancelled" },
            "activeTaskCount": 0,
            "startedAtEpoch": serde_json::Value::Null,
            "updatedAtEpoch": updated_at,
            "requiresAttention": false,
            "updateLabel": serde_json::Value::Null,
            "updateAtEpoch": serde_json::Value::Null,
            "completedOutputCount": completed_output_count,
            "taskSummaries": [],
        });
        Self {
            signature: hex_digest(
                digest::digest(&digest::SHA256, content.to_string().as_bytes()).as_ref(),
            ),
            content,
            focus_task_id: task_id.to_string(),
        }
    }
}

fn activity_projection(activity: &noema_store::ClientTaskActivityRecord) -> Option<LiveProjection> {
    let focus_task_id = activity
        .latest_projection
        .get("focusTaskId")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    (!activity.latest_projection_signature.is_empty()).then(|| LiveProjection {
        content: activity.latest_projection.clone(),
        signature: activity.latest_projection_signature.clone(),
        focus_task_id,
    })
}

async fn live_projection(
    store: &NoemaStore,
) -> std::result::Result<Option<LiveProjection>, noema_store::StoreError> {
    let workspace_id = WorkspaceId::new("workspace:personal").map_err(|_| {
        noema_store::StoreError::InvariantViolation {
            message: "personal workspace identity is invalid".to_string(),
        }
    })?;
    let mut after = None;
    let mut tasks = Vec::new();
    loop {
        let page = store
            .list_work_tasks(WorkTaskQuery {
                workspace_id: workspace_id.clone(),
                project_id: None,
                stage_ids: Vec::new(),
                stage_behaviors: vec![
                    WorkflowStageBehavior::Active,
                    WorkflowStageBehavior::HumanGate,
                ],
                attention_only: false,
                scope: WorkTaskScope::Active,
                first: WorkPageSize::new(100).map_err(|_| {
                    noema_store::StoreError::InvariantViolation {
                        message: "Live Activity page size is invalid".to_string(),
                    }
                })?,
                after,
            })
            .await?;
        tasks.extend(page.edges.into_iter().map(|edge| edge.node));
        if !page.page_info.has_next_page {
            break;
        }
        let Some(cursor) = page.page_info.end_cursor else {
            break;
        };
        after = Some(WorkTaskCursor::decode(&cursor).map_err(|_| {
            noema_store::StoreError::InvariantViolation {
                message: "Live Activity task cursor is invalid".to_string(),
            }
        })?);
    }
    if tasks.is_empty() {
        return Ok(None);
    }
    tasks.sort_by(|left, right| {
        live_focus_rank(left.current_run.as_ref())
            .cmp(&live_focus_rank(right.current_run.as_ref()))
            .then_with(|| {
                let left_updated = left
                    .current_run
                    .as_ref()
                    .map_or(left.task.updated_at.as_str(), |run| {
                        std::cmp::max(left.task.updated_at.as_str(), run.updated_at.as_str())
                    });
                let right_updated = right
                    .current_run
                    .as_ref()
                    .map_or(right.task.updated_at.as_str(), |run| {
                        std::cmp::max(right.task.updated_at.as_str(), run.updated_at.as_str())
                    });
                right_updated.cmp(left_updated)
            })
            .then_with(|| left.task.task_id.as_str().cmp(right.task.task_id.as_str()))
    });
    let focus = &tasks[0];
    let phase = live_task_phase(focus);
    let status_label = live_task_status(focus);
    let started_at = focus
        .current_run
        .as_ref()
        .and_then(|run| run.started_at.as_deref())
        .and_then(parse_epoch);
    let requires_attention = live_task_requires_attention(focus);
    let updated_at_value = focus
        .current_run
        .as_ref()
        .map_or(focus.task.updated_at.as_str(), |run| {
            std::cmp::max(focus.task.updated_at.as_str(), run.updated_at.as_str())
        });
    let base_updated_at =
        parse_epoch(updated_at_value).unwrap_or_else(|| chrono::Utc::now().timestamp() as f64);
    let agent_id = focus
        .current_run
        .as_ref()
        .map_or(focus.task.executor_agent_id.as_str(), |run| {
            run.agent_id.as_str()
        });
    let agent_name = store
        .get_agent(agent_id)
        .await?
        .and_then(|agent| agent.display_name)
        .unwrap_or_else(|| "Agent".to_string());
    let update = live_run_update(store, &workspace_id, focus).await;
    let updated_at = update
        .as_ref()
        .and_then(|update| update.updated_at)
        .map_or(base_updated_at, |item_updated_at| {
            base_updated_at.max(item_updated_at)
        });
    let update_label = update.as_ref().map(|update| update.label.clone());
    let task_summaries = tasks
        .iter()
        .take(2)
        .map(live_task_summary)
        .collect::<Vec<_>>();
    let content = serde_json::json!({
        "focusTaskId": focus.task.task_id.as_str(),
        "focusTitle": notification_text(&focus.task.title),
        "projectName": focus.project.as_ref().map(|project| project.name.clone()),
        "agentName": notification_text(&agent_name),
        "phase": phase,
        "statusLabel": status_label,
        "activeTaskCount": tasks.len(),
        "startedAtEpoch": started_at,
        "updatedAtEpoch": updated_at,
        "requiresAttention": requires_attention,
        "updateLabel": update_label,
        "updateAtEpoch": update_label.map(|_| updated_at),
        "completedOutputCount": serde_json::Value::Null,
        "taskSummaries": task_summaries,
    });
    let signature =
        hex_digest(digest::digest(&digest::SHA256, content.to_string().as_bytes()).as_ref());
    Ok(Some(LiveProjection {
        content,
        signature,
        focus_task_id: focus.task.task_id.to_string(),
    }))
}

fn live_task_phase(task: &WorkTaskSummary) -> &'static str {
    if task.stage.system_behavior == WorkflowStageBehavior::HumanGate {
        return "reviewing";
    }
    task.current_run
        .as_ref()
        .map_or("inProgress", |run| match run.run_kind {
            RunKind::Planner => "planning",
            RunKind::Executor => "working",
            RunKind::Reviewer => "reviewing",
        })
}

fn live_task_status(task: &WorkTaskSummary) -> &'static str {
    if task.stage.system_behavior == WorkflowStageBehavior::HumanGate {
        return "Needs You";
    }
    task.current_run
        .as_ref()
        .map_or("In progress", |run| match run.status {
            RunStatus::Running => match run.run_kind {
                RunKind::Planner => "Planning",
                RunKind::Executor => "Working",
                RunKind::Reviewer => "Reviewing",
            },
            RunStatus::Leased => "Starting",
            RunStatus::Queued => "Queued",
            RunStatus::WaitingForApproval => "Needs You",
            RunStatus::Completed => "Completed",
            RunStatus::Interrupted | RunStatus::Failed => "Needs attention",
            RunStatus::Cancelled => "Cancelled",
        })
}

fn live_task_requires_attention(task: &WorkTaskSummary) -> bool {
    task.stage.system_behavior == WorkflowStageBehavior::HumanGate
        || task.current_run.as_ref().is_some_and(|run| {
            matches!(
                run.status,
                RunStatus::WaitingForApproval | RunStatus::Interrupted | RunStatus::Failed
            )
        })
}

struct LiveRunUpdate {
    label: String,
    updated_at: Option<f64>,
}

async fn live_run_update(
    store: &NoemaStore,
    workspace_id: &WorkspaceId,
    task: &WorkTaskSummary,
) -> Option<LiveRunUpdate> {
    let run = task.current_run.as_ref()?;
    let items = store
        .list_work_run_items(WorkRunItemQuery {
            owner: WorkRunItemOwnerScope {
                workspace_id: workspace_id.clone(),
                task_id: Some(task.task.task_id.clone()),
            },
            run_id: run.run_id.clone(),
            first: WorkPageSize::new(20).expect("Live Activity transcript window is valid"),
            before: None,
        })
        .await
        .ok()
        .map(|connection| {
            connection
                .edges
                .into_iter()
                .map(|edge| edge.node)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    live_transcript_update(&items)
}

fn live_transcript_update(items: &[AgentRunItemRecord]) -> Option<LiveRunUpdate> {
    let active_tool = items.iter().find(|item| {
        item.kind == AgentRunItemKind::ToolCall && item.status == AgentRunItemStatus::Running
    });
    if let Some(tool) = active_tool
        && let Some(label) = live_tool_line(items, tool)
    {
        return Some(LiveRunUpdate {
            label,
            updated_at: run_item_epoch(tool),
        });
    }
    for item in items.iter().rev() {
        let label = match item.kind {
            AgentRunItemKind::AssistantOutput if item.status == AgentRunItemStatus::Running => {
                return None;
            }
            AgentRunItemKind::AssistantOutput if item.status == AgentRunItemStatus::Completed => {
                item.content_text.as_deref().and_then(live_activity_text)
            }
            AgentRunItemKind::ProgressNotice
                if item.status == AgentRunItemStatus::Completed
                    && item
                        .payload
                        .get("phase")
                        .and_then(serde_json::Value::as_str)
                        != Some("provider_response") =>
            {
                item.content_text.as_deref().and_then(live_activity_text)
            }
            _ => None,
        };
        if let Some(label) = label {
            return Some(LiveRunUpdate {
                label,
                updated_at: run_item_epoch(item),
            });
        }
    }
    None
}

fn run_item_epoch(item: &AgentRunItemRecord) -> Option<f64> {
    parse_epoch(&item.updated_at).or_else(|| parse_epoch(&item.created_at))
}

fn live_tool_line(items: &[AgentRunItemRecord], tool: &AgentRunItemRecord) -> Option<String> {
    live_tool_update_label(tool).or_else(|| {
        items
            .iter()
            .rev()
            .find(|item| {
                item.kind == AgentRunItemKind::AssistantOutput
                    && item.status == AgentRunItemStatus::Completed
                    && item.round_index == tool.round_index
                    && item.sequence_index < tool.sequence_index
            })
            .and_then(|item| item.content_text.as_deref())
            .and_then(live_activity_text)
    })
}

fn live_tool_update_label(item: &AgentRunItemRecord) -> Option<String> {
    let name = item.content_text.as_deref()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut action = item.payload.clone();
    action["name"] = Value::String(name.to_string());
    let label = noema_runtime::tool_marker_for_action("tool_call", item.status.as_str(), &action)
        .and_then(|marker| {
            marker
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Using {name}"));
    live_activity_text(&label)
}

fn live_activity_text(value: &str) -> Option<String> {
    let plain = notification_text(value);
    if plain.is_empty() {
        return None;
    }
    if plain.len() <= MAX_LIVE_UPDATE_BYTES {
        return Some(plain);
    }
    let mut end = MAX_LIVE_UPDATE_BYTES.saturating_sub("…".len());
    while !plain.is_char_boundary(end) {
        end -= 1;
    }
    Some(format!("{}…", plain[..end].trim_end()))
}

fn live_task_summary(task: &WorkTaskSummary) -> serde_json::Value {
    let started_at = task
        .current_run
        .as_ref()
        .and_then(|run| run.started_at.as_deref())
        .and_then(parse_epoch);
    serde_json::json!({
        "taskId": task.task.task_id.as_str(),
        "title": notification_text(&task.task.title),
        "phase": live_task_phase(task),
        "statusLabel": live_task_status(task),
        "startedAtEpoch": started_at,
        "requiresAttention": live_task_requires_attention(task),
    })
}

fn live_focus_rank(run: Option<&noema_tasks::AgentRunRecord>) -> u8 {
    match run.map(|run| run.status) {
        Some(RunStatus::Running) => 0,
        Some(RunStatus::Leased) => 1,
        Some(RunStatus::Queued) => 2,
        _ => 3,
    }
}

fn parse_epoch(value: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis() as f64 / 1000.0)
}

fn live_activity_urgency(event: LiveActivityEvent, alerts_human: bool) -> &'static str {
    match event {
        LiveActivityEvent::Start => "high",
        LiveActivityEvent::Update if alerts_human => "high",
        LiveActivityEvent::Update | LiveActivityEvent::End => "normal",
    }
}

fn live_activity_collapse_id(
    event: LiveActivityEvent,
    urgency: &str,
    activity_id: Option<&str>,
) -> Option<String> {
    if event != LiveActivityEvent::Update || urgency != "normal" {
        return None;
    }
    activity_id.map(|id| hex_digest(digest::digest(&digest::SHA256, id.as_bytes()).as_ref()))
}

fn live_activity_payload(
    event: LiveActivityEvent,
    client_id: &str,
    activity_id: Option<&str>,
    server_origin: &str,
    projection: &LiveProjection,
    alert: Option<(&str, &str, &str)>,
) -> serde_json::Value {
    let mut aps = serde_json::Map::new();
    aps.insert(
        "timestamp".to_string(),
        projection.content["updatedAtEpoch"].clone(),
    );
    aps.insert(
        "event".to_string(),
        serde_json::Value::String(event.as_str().to_string()),
    );
    if event == LiveActivityEvent::Start {
        aps.insert(
            "attributes-type".to_string(),
            serde_json::Value::String(LIVE_ACTIVITY_ATTRIBUTES_TYPE.to_string()),
        );
        aps.insert(
            "attributes".to_string(),
            serde_json::json!({
                "activityId": activity_id.unwrap_or("live_activity:pending"),
                "clientId": client_id,
                "serverOrigin": server_origin,
            }),
        );
        aps.insert(
            "alert".to_string(),
            serde_json::json!({
                "title": "Noema Tasks",
                "body": projection.content["focusTitle"]
                    .as_str()
                    .map(notification_text)
                    .unwrap_or_default(),
            }),
        );
    }
    aps.insert("content-state".to_string(), projection.content.clone());
    if let Some((title, body, _task_id)) = alert {
        aps.insert(
            "alert".to_string(),
            serde_json::json!({"title": notification_text(title), "body": notification_text(body)}),
        );
    }
    serde_json::json!({
        "aps": aps,
        "route": "task",
        "taskId": alert.map_or_else(|| projection.focus_task_id.clone(), |(_, _, task_id)| task_id.to_string()),
        "version": 1,
    })
}

fn decode_live_token(value: &str) -> Result<Vec<u8>> {
    let token = URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|_| async_graphql::Error::new("Live Activity token is invalid"))?;
    if token.is_empty() || token.len() > 1024 {
        return Err(async_graphql::Error::new("Live Activity token is invalid"));
    }
    Ok(token)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum PresenceKey {
    Browser(String),
    Client(String),
}

struct VisibilityLease {
    inner: Arc<WebPushInner>,
    key: PresenceKey,
}

impl VisibilityLease {
    fn new(inner: Arc<WebPushInner>, key: PresenceKey) -> Self {
        if let Ok(mut visible) = inner.visible.lock() {
            *visible.entry(key.clone()).or_default() += 1;
        }
        Self { inner, key }
    }
}

impl Drop for VisibilityLease {
    fn drop(&mut self) {
        if let Ok(mut visible) = self.inner.visible.lock()
            && let Some(count) = visible.get_mut(&self.key)
        {
            *count -= 1;
            if *count == 0 {
                visible.remove(&self.key);
            }
        }
    }
}

async fn primary_agent_name(state: &GraphqlState) -> String {
    match state.optional_store() {
        Some(store) => store
            .get_agent("agent:primary")
            .await
            .ok()
            .flatten()
            .and_then(|agent| agent.display_name)
            .unwrap_or_else(|| "Noema".to_string()),
        None => "Noema".to_string(),
    }
}

fn intervention_notification(intervention: &GraphqlHumanIntervention) -> (String, String, String) {
    match intervention {
        GraphqlHumanIntervention::TaskAttention(attention) => (
            format!("task:{}:{:?}", attention.task.task_id, attention.kind),
            attention.title.clone(),
            attention.summary.clone(),
        ),
        GraphqlHumanIntervention::GovernedAction(action) => (
            format!("action:{}:{}", action.action_id, action.revision),
            "Noema needs your approval".to_string(),
            action.safe_summary.clone(),
        ),
        GraphqlHumanIntervention::McpAuthentication(request) => (
            format!("mcp-auth:{}:{}", request.request_id, request.revision),
            format!("Sign in to {}", request.server_display_name),
            request.capability_name.clone(),
        ),
        GraphqlHumanIntervention::AdapterAuthentication(request) => (
            format!("adapter-auth:{}:{}", request.request_id, request.revision),
            format!("Sign in to {}", request.service_display_name),
            request.capability_name.clone(),
        ),
        GraphqlHumanIntervention::McpSetup(setup) => (
            format!("mcp-setup:{}", setup.item_id),
            format!("Finish setting up {}", setup.display_name),
            setup.description.clone().unwrap_or_default(),
        ),
        GraphqlHumanIntervention::AdapterOauthClientSetup(setup) => (
            format!("adapter-oauth-client:{}", setup.profile_digest),
            "Import one OAuth client".to_string(),
            format!(
                "Use it for {} reviewed API{}.",
                setup.dependent_definitions.len(),
                if setup.dependent_definitions.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
        ),
        GraphqlHumanIntervention::AdapterOauthAccountSetup(setup) => {
            let account = setup
                .account_label
                .as_deref()
                .unwrap_or(&setup.provider_display_name);
            (
                format!("adapter-oauth-account:{}", setup.setup_key),
                if setup.next_action.kind == "add_access" {
                    format!("Update access for {account}")
                } else {
                    format!("Connect {account}")
                },
                format!(
                    "Set up {} API{}.",
                    setup.dependent_definitions.len(),
                    if setup.dependent_definitions.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            )
        }
        GraphqlHumanIntervention::AdapterDefinition(definition) => (
            format!(
                "adapter:{}:{}",
                definition.semantic_digest, definition.definition_revision
            ),
            format!("Review {}", definition.display_name),
            definition.source_reference.clone(),
        ),
    }
}

fn notification_text(value: &str) -> String {
    let mut plain = String::with_capacity(value.len());
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_DEFINITION_LIST;
    for event in Parser::new_ext(value, options) {
        match event {
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text)
            | Event::FootnoteReference(text) => plain.push_str(&text),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote(_)
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::FootnoteDefinition
                | TagEnd::DefinitionList
                | TagEnd::DefinitionListTitle
                | TagEnd::DefinitionListDefinition
                | TagEnd::Table
                | TagEnd::TableHead
                | TagEnd::TableRow
                | TagEnd::TableCell,
            )
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Rule => plain.push(' '),
            Event::Start(_)
            | Event::End(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::TaskListMarker(_) => {}
        }
    }
    let normalized = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= MAX_PREVIEW_BYTES {
        return normalized;
    }
    let mut end = MAX_PREVIEW_BYTES.saturating_sub("…".len());
    while !normalized.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", normalized[..end].trim_end())
}

fn primary_chat_notification(
    item: &noema_conversations::ConversationItemRecord,
) -> Option<(String, String)> {
    if item.kind != ConversationItemKind::AssistantText
        || item
            .metadata
            .get("phase")
            .and_then(serde_json::Value::as_str)
            != Some("final_answer")
    {
        return None;
    }
    Some((
        format!("chat-turn:{}", item.turn_id.as_deref()?),
        item.content_text.as_deref()?.to_string(),
    ))
}

fn delivery_outcome(
    result: std::result::Result<reqwest::StatusCode, String>,
) -> NotificationDeliveryOutcome<'static> {
    match result {
        Ok(status) if status.is_success() => NotificationDeliveryOutcome::Delivered,
        Ok(status) if matches!(status.as_u16(), 404 | 410) => NotificationDeliveryOutcome::Expired,
        Ok(status) if status.as_u16() == 429 || status.is_server_error() => {
            NotificationDeliveryOutcome::Retry("remote_retry")
        }
        Ok(_) => NotificationDeliveryOutcome::Failed(Some("remote_rejected")),
        Err(_) => NotificationDeliveryOutcome::Retry("transport_unavailable"),
    }
}

fn declarative_payload(
    title: &str,
    body: &str,
    navigate: &str,
    tag: &str,
) -> std::result::Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "web_push": 8030,
        "notification": {
            "title": title,
            "body": body,
            "navigate": navigate,
            "tag": tag,
            "silent": false,
        }
    }))
    .map_err(|_| "invalid payload".to_string())
}

fn validate_subscription_material(input: &GraphqlRegisterWebPushSubscriptionInput) -> Result<()> {
    let public_key = URL_SAFE_NO_PAD
        .decode(input.p256dh.as_bytes())
        .map_err(|_| async_graphql::Error::new("Web Push subscription is invalid"))?;
    let auth = URL_SAFE_NO_PAD
        .decode(input.auth.as_bytes())
        .map_err(|_| async_graphql::Error::new("Web Push subscription is invalid"))?;
    if PublicKey::from_sec1_bytes(&public_key).is_err() || auth.len() != 16 {
        return Err(async_graphql::Error::new(
            "Web Push subscription is invalid",
        ));
    }
    Ok(())
}

fn validate_push_endpoint(value: &str) -> Result<Url> {
    let url = validate_public_url(value)
        .map_err(|_| async_graphql::Error::new("Web Push endpoint is unavailable"))?;
    if url.scheme() != "https" {
        return Err(async_graphql::Error::new(
            "Web Push endpoint is unavailable",
        ));
    }
    Ok(url)
}

async fn checked_client(url: &Url) -> std::result::Result<Client, String> {
    let port = url.port_or_known_default().ok_or("invalid endpoint")?;
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30));
    if let Host::Domain(hostname) = url.host().ok_or("invalid endpoint")? {
        let addresses = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((hostname, port)),
        )
        .await
        .map_err(|_| "endpoint DNS timed out")?
        .map_err(|_| "endpoint DNS failed")?
        .collect::<Vec<SocketAddr>>();
        if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
            return Err("endpoint is not public".to_string());
        }
        builder = builder.resolve_to_addrs(hostname, &addresses);
    }
    builder
        .build()
        .map_err(|_| "push client unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_conversations::{
        ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    };
    use reqwest::StatusCode;
    use serde_json::json;

    fn assistant_item(phase: &str) -> ConversationItemRecord {
        ConversationItemRecord {
            item_id: "item:one".to_string(),
            conversation_id: "conversation:one".to_string(),
            turn_id: Some("turn:one".to_string()),
            sequence_index: 1,
            cursor: "cursor".to_string(),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            content_text: Some("  Ready   for you.  ".to_string()),
            provider_content_text: None,
            payload_json: json!({}),
            metadata: json!({"phase": phase}),
            created_at: String::new(),
        }
    }

    fn run_item(
        sequence_index: i64,
        round_index: i64,
        kind: AgentRunItemKind,
        status: AgentRunItemStatus,
        content_text: &str,
        payload: serde_json::Value,
    ) -> AgentRunItemRecord {
        AgentRunItemRecord {
            item_id: format!("run_item:{sequence_index}"),
            run_id: "run:one".to_string(),
            sequence_index,
            round_index,
            kind,
            status,
            correlation_id: None,
            parent_item_id: None,
            content_text: Some(content_text.to_string()),
            payload,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn live_label(items: &[AgentRunItemRecord]) -> Option<String> {
        live_transcript_update(items).map(|update| update.label)
    }
    #[tokio::test]
    async fn generated_vapid_identity_uses_an_uncompressed_public_key() {
        let root = tempfile::tempdir().expect("home");
        let coordinator = NotificationCoordinator::new_with_paths(
            crate::test_support::test_store().await,
            "https://noema.example".to_string(),
            NoemaPaths::from_noema_home(root.path()).expect("paths"),
        )
        .await
        .expect("initialize Web Push");
        let status = coordinator
            .status(LOCAL_HUMAN_ID, [3; 32], None)
            .await
            .expect("read Web Push status");
        let public_key = URL_SAFE_NO_PAD
            .decode(
                status
                    .application_server_key
                    .expect("application server key"),
            )
            .expect("decode application server key");
        assert_eq!(public_key.len(), 65);
        assert_eq!(public_key[0], 4);
    }
    #[tokio::test]
    async fn idle_notification_reconciliation_does_not_wake_itself() {
        let environment = crate::test_support::test_environment();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        store
            .get_or_create_primary_conversation(LOCAL_HUMAN_ID, None, None)
            .await
            .expect("ensure primary conversation");
        let coordinator = NotificationCoordinator::new_with_paths(
            store.clone(),
            "https://noema.example".to_string(),
            NoemaPaths::from_noema_home(environment.root()).expect("paths"),
        )
        .await
        .expect("initialize notifications");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment);

        coordinator
            .reconcile_primary_chat(&state)
            .await
            .expect("reconcile chat");
        coordinator
            .reconcile_interventions(&state)
            .await
            .expect("reconcile interventions");

        assert!(
            tokio::time::timeout(Duration::from_millis(10), coordinator.inner.wake.notified())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn task_events_debounce_live_reconciliation_and_work_clears_it() {
        async fn take_reconciliation(
            coordinator: &NotificationCoordinator,
            wait: Duration,
        ) -> bool {
            let Ok(Ok(permit)) =
                tokio::time::timeout(wait, coordinator.inner.live_reconciled.acquire()).await
            else {
                return false;
            };
            permit.forget();
            true
        }

        let environment = crate::test_support::test_environment();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let coordinator = NotificationCoordinator::new_with_paths(
            store.clone(),
            "https://noema.example".to_string(),
            NoemaPaths::from_noema_home(environment.root()).expect("paths"),
        )
        .await
        .expect("initialize notifications");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment);
        let registry = noema_runtime::RuntimeEventRegistry::default();
        let runner = tokio::spawn(coordinator.clone().run(
            state,
            registry.subscribe_all_conversations(),
            registry.subscribe_all_tasks(),
            registry.subscribe_work("workspace:personal"),
        ));
        assert!(take_reconciliation(&coordinator, Duration::from_secs(1)).await);

        for _ in 0..3 {
            registry.publish_task(TaskRuntimeEvent::Changed {
                task_id: "task:one".to_string(),
                run_id: Some("run:one".to_string()),
            });
        }
        assert!(!take_reconciliation(&coordinator, Duration::from_millis(400)).await);
        assert!(take_reconciliation(&coordinator, Duration::from_millis(300)).await);

        registry.publish_task(TaskRuntimeEvent::Changed {
            task_id: "task:one".to_string(),
            run_id: Some("run:one".to_string()),
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        registry.publish_work(WorkRuntimeEvent::Committed {
            workspace_id: "workspace:personal".to_string(),
            task_id: Some("task:one".to_string()),
        });
        assert!(take_reconciliation(&coordinator, Duration::from_millis(200)).await);
        assert!(!take_reconciliation(&coordinator, Duration::from_millis(550)).await);
        runner.abort();
    }
    #[test]
    fn only_final_primary_chat_text_becomes_a_notification() {
        assert_eq!(
            primary_chat_notification(&assistant_item("final_answer")),
            Some((
                "chat-turn:turn:one".to_string(),
                "  Ready   for you.  ".to_string()
            ))
        );
        assert_eq!(
            primary_chat_notification(&assistant_item("commentary")),
            None
        );
        assert!(notification_text(&"x".repeat(MAX_PREVIEW_BYTES + 1)).len() <= MAX_PREVIEW_BYTES);
    }
    #[test]
    fn notification_preview_removes_markdown_markup() {
        for (markdown, expected) in [
            (
                "## **Ready** for [review](https://noema.example)",
                "Ready for review",
            ),
            (
                "> Use `cargo check`\n\n- first\n- second",
                "Use cargo check first second",
            ),
            (
                "![Build status](status.png) and ~~old text~~",
                "Build status and old text",
            ),
            ("<strong>Ready</strong> now", "Ready now"),
            (
                r"Keep \*literal\* but remove *emphasis*",
                "Keep *literal* but remove emphasis",
            ),
        ] {
            assert_eq!(notification_text(markdown), expected, "{markdown:?}");
        }
    }
    #[test]
    fn declarative_payload_keeps_required_fallback_fields() {
        let value: serde_json::Value = serde_json::from_slice(
            &declarative_payload("Noema", "Ready", "https://noema.example/", "chat-turn:one")
                .expect("serialize payload"),
        )
        .expect("parse payload");
        assert_eq!(value["web_push"], 8030);
        assert_eq!(value["notification"]["navigate"], "https://noema.example/");
        assert_eq!(value["notification"]["body"], "Ready");
    }
    #[test]
    fn live_activity_payload_keeps_activitykit_fields_and_task_alert_route() {
        let projection = LiveProjection {
            content: json!({
                "focusTaskId": "task:focus",
                "focusTitle": "Focus",
                "projectName": "Personal",
                "agentName": "Atlas",
                "phase": "working",
                "statusLabel": "Running",
                "activeTaskCount": 2,
                "startedAtEpoch": 1.0,
                "updatedAtEpoch": 2.0,
                "requiresAttention": true,
                "updateLabel": "Using tools",
                "updateAtEpoch": 2.0,
                "completedOutputCount": null,
                "taskSummaries": [{
                    "taskId": "task:focus",
                    "title": "Focus",
                    "phase": "working",
                    "statusLabel": "Running",
                    "startedAtEpoch": 1.0,
                }],
            }),
            signature: "a".repeat(64),
            focus_task_id: "task:focus".to_string(),
        };
        let start_payload = live_activity_payload(
            LiveActivityEvent::Start,
            "client:one",
            Some("live_activity:one"),
            "https://noema.example",
            &projection,
            None,
        );
        let alert_payload = live_activity_payload(
            LiveActivityEvent::Update,
            "client:one",
            Some("live_activity:one"),
            "https://noema.example",
            &projection,
            Some(("Task waiting", "Review Focus", "task:one")),
        );
        assert_eq!(
            LIVE_ACTIVITY_TOPIC,
            "dev.noema.app.ios.push-type.liveactivity"
        );
        assert_eq!(start_payload["route"], "task");
        assert_eq!(start_payload["taskId"], "task:focus");
        assert_eq!(start_payload["aps"]["alert"]["title"], "Noema Tasks");
        assert_eq!(start_payload["aps"]["alert"]["body"], "Focus");
        assert_eq!(
            start_payload["aps"]["attributes-type"],
            LIVE_ACTIVITY_ATTRIBUTES_TYPE
        );
        assert_eq!(start_payload["aps"]["content-state"]["activeTaskCount"], 2);
        assert_eq!(
            start_payload["aps"]["content-state"]["requiresAttention"],
            true
        );
        assert_eq!(start_payload["aps"]["content-state"]["agentName"], "Atlas");
        assert_eq!(
            start_payload["aps"]["content-state"]["taskSummaries"][0]["title"],
            "Focus"
        );
        assert_eq!(alert_payload["taskId"], "task:one");
        assert_eq!(alert_payload["aps"]["alert"]["body"], "Review Focus");
    }
    #[test]
    fn live_activity_priority_reserves_high_delivery_for_immediate_events() {
        assert_eq!(
            live_activity_urgency(LiveActivityEvent::Start, false),
            "high"
        );
        assert_eq!(
            live_activity_urgency(LiveActivityEvent::Update, true),
            "high"
        );
        assert_eq!(
            live_activity_urgency(LiveActivityEvent::Update, false),
            "normal"
        );
        assert_eq!(
            live_activity_urgency(LiveActivityEvent::End, false),
            "normal"
        );
        let collapse_id = live_activity_collapse_id(
            LiveActivityEvent::Update,
            "normal",
            Some("live_activity:one"),
        )
        .expect("ordinary update should collapse");
        assert_eq!(collapse_id.len(), 64);
        assert!(
            collapse_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        );
        assert_eq!(
            live_activity_collapse_id(
                LiveActivityEvent::Update,
                "normal",
                Some("live_activity:one"),
            ),
            Some(collapse_id)
        );
        assert_eq!(
            live_activity_collapse_id(LiveActivityEvent::Update, "high", Some("live_activity:one")),
            None
        );
        assert_eq!(
            live_activity_collapse_id(LiveActivityEvent::End, "normal", Some("live_activity:one")),
            None
        );
    }
    #[tokio::test]
    async fn live_activity_reconciliation_uses_one_mutation_lane() {
        let root = tempfile::tempdir().expect("home");
        let coordinator = NotificationCoordinator::new_with_paths(
            crate::test_support::test_store().await,
            "https://noema.example".to_string(),
            NoemaPaths::from_noema_home(root.path()).expect("paths"),
        )
        .await
        .expect("initialize notifications");
        let guard = coordinator.inner.live_activity_mutation.lock().await;
        let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
        let reconciliation = tokio::spawn({
            let coordinator = coordinator.clone();
            async move {
                let _ = started_sender.send(());
                coordinator.reconcile_live_activities().await;
            }
        });
        started_receiver.await.expect("start reconciliation");
        tokio::task::yield_now().await;
        assert!(!reconciliation.is_finished());
        drop(guard);
        tokio::time::timeout(Duration::from_secs(1), reconciliation)
            .await
            .expect("finish reconciliation")
            .expect("join reconciliation");
    }
    #[tokio::test]
    async fn live_activity_reconciliation_keeps_inflight_and_replaces_old_focus() {
        let root = tempfile::tempdir().expect("home");
        let store = crate::test_support::test_store().await;
        store
            .insert_client("client:one", LOCAL_HUMAN_ID, "iPhone")
            .await
            .expect("insert client");
        store
            .register_client_live_activities(
                "client:one",
                &[2; 32],
                ApnsEnvironment::Development,
                &[],
            )
            .await
            .expect("register Live Activities");
        let target = store
            .live_activity_targets()
            .await
            .expect("load targets")
            .pop()
            .expect("registered target");
        let coordinator = NotificationCoordinator::new_with_paths(
            store.clone(),
            "https://noema.example".to_string(),
            NoemaPaths::from_noema_home(root.path()).expect("paths"),
        )
        .await
        .expect("initialize notifications");
        let projection = Err(noema_store::StoreError::InvariantViolation {
            message: "temporary projection failure".to_string(),
        });

        coordinator.apply_live_projection(target, &projection).await;

        let activity = store
            .client_task_activity("client:one")
            .await
            .expect("load activity")
            .expect("starting activity");
        assert_eq!(activity.lifecycle, "starting");
        assert!(!activity.suppressed);

        let target = store
            .live_activity_targets()
            .await
            .expect("reload targets")
            .pop()
            .expect("registered target");
        coordinator.apply_live_projection(target, &Ok(None)).await;

        let activity = store
            .client_task_activity("client:one")
            .await
            .expect("reload activity")
            .expect("starting activity");
        assert_eq!(activity.lifecycle, "starting");
        assert!(!activity.suppressed);

        store
            .update_client_task_activity_projection(
                "client:one",
                &json!({"focusTaskId":"task:old"}),
                &"a".repeat(64),
                Some("task:old"),
            )
            .await
            .expect("save old projection");
        store
            .dismiss_client_live_activity(
                "client:one",
                activity.activity_id.as_deref().expect("activity id"),
                false,
            )
            .await
            .expect("dismiss old activity");
        let old_session = activity.task_session_id;
        let target = store
            .live_activity_targets()
            .await
            .expect("reload dismissed target")
            .pop()
            .expect("dismissed target");
        let projection = LiveProjection {
            content: json!({
                "focusTaskId": "task:new",
                "focusTitle": "New task",
                "updatedAtEpoch": 1.0,
            }),
            signature: "b".repeat(64),
            focus_task_id: "task:new".to_string(),
        };

        coordinator
            .apply_live_projection(target, &Ok(Some(projection)))
            .await;

        let activity = store
            .client_task_activity("client:one")
            .await
            .expect("load replacement")
            .expect("replacement activity");
        assert_eq!(activity.lifecycle, "starting");
        assert_eq!(activity.focused_task_id.as_deref(), Some("task:new"));
        assert_ne!(activity.task_session_id, old_session);
        assert!(!activity.suppressed);
    }
    #[test]
    fn live_activity_uses_shared_text_for_the_active_tool() {
        let items = vec![
            run_item(
                1,
                2,
                AgentRunItemKind::AssistantOutput,
                AgentRunItemStatus::Completed,
                "**Searching Apple documentation for ActivityKit updates.**",
                json!({}),
            ),
            run_item(
                2,
                2,
                AgentRunItemKind::ToolCall,
                AgentRunItemStatus::Running,
                "web.search",
                json!({"arguments": {"query": "ActivityKit updates"}}),
            ),
        ];

        assert_eq!(
            live_label(&items).as_deref(),
            Some("Searching the web for “ActivityKit updates”")
        );
    }
    #[test]
    fn live_activity_retains_the_latest_tool_without_commentary() {
        let items = vec![run_item(
            1,
            2,
            AgentRunItemKind::ToolCall,
            AgentRunItemStatus::Running,
            "web.search",
            json!({"arguments": {"query": "ActivityKit updates"}}),
        )];

        assert_eq!(
            live_label(&items).as_deref(),
            Some("Searching the web for “ActivityKit updates”")
        );
    }
    #[test]
    fn live_activity_ignores_partial_commentary_and_completed_tools() {
        let items = vec![
            run_item(
                1,
                2,
                AgentRunItemKind::AssistantOutput,
                AgentRunItemStatus::Running,
                "Partial task commentary",
                json!({}),
            ),
            run_item(
                2,
                2,
                AgentRunItemKind::ToolCall,
                AgentRunItemStatus::Completed,
                "web.search",
                json!({"arguments": {"query": "stale result"}}),
            ),
        ];

        assert_eq!(live_label(&items), None);
    }
    #[test]
    fn live_activity_uses_the_first_running_tool_and_its_timestamp() {
        let mut first = run_item(
            1,
            2,
            AgentRunItemKind::ToolCall,
            AgentRunItemStatus::Running,
            "web.search",
            json!({"arguments": {"query": "current work"}}),
        );
        first.updated_at = "2026-08-15T12:00:01Z".to_string();
        let mut second = run_item(
            2,
            2,
            AgentRunItemKind::ToolCall,
            AgentRunItemStatus::Running,
            "search_memory",
            json!({"arguments": {"query": "queued work"}}),
        );
        second.updated_at = "2026-08-15T12:00:02Z".to_string();

        let update = live_transcript_update(&[first, second]).expect("active tool");

        assert_eq!(update.label, "Searching the web for “current work”");
        assert_eq!(update.updated_at, parse_epoch("2026-08-15T12:00:01Z"));
    }
    #[test]
    fn live_activity_partial_output_hides_an_older_completed_line() {
        let items = vec![
            run_item(
                1,
                1,
                AgentRunItemKind::AssistantOutput,
                AgentRunItemStatus::Completed,
                "Finished the earlier action.",
                json!({}),
            ),
            run_item(
                2,
                2,
                AgentRunItemKind::AssistantOutput,
                AgentRunItemStatus::Running,
                "Starting the next action",
                json!({}),
            ),
        ];

        assert_eq!(live_label(&items), None);
    }
    #[test]
    fn live_activity_uses_the_latest_meaningful_task_line() {
        let items = vec![
            run_item(
                1,
                0,
                AgentRunItemKind::ProgressNotice,
                AgentRunItemStatus::Completed,
                "Still finding relevant records.",
                json!({}),
            ),
            run_item(
                2,
                3,
                AgentRunItemKind::AssistantOutput,
                AgentRunItemStatus::Completed,
                "Comparing the matching records.",
                json!({}),
            ),
            run_item(
                3,
                3,
                AgentRunItemKind::ProgressNotice,
                AgentRunItemStatus::Completed,
                "Provider response received.",
                json!({"phase": "provider_response"}),
            ),
        ];

        assert_eq!(
            live_label(&items).as_deref(),
            Some("Comparing the matching records.")
        );
    }
    #[test]
    fn terminal_live_activity_keeps_completed_output_metadata() {
        let projection = LiveProjection::terminal(
            "task:done",
            "Finished task",
            Some("Personal"),
            "Atlas",
            "completed",
            Some(3),
            42.0,
        );

        assert_eq!(projection.content["phase"], "completed");
        assert_eq!(projection.content["agentName"], "Atlas");
        assert_eq!(projection.content["completedOutputCount"], 3);
        assert_eq!(projection.content["activeTaskCount"], 0);
    }
    #[test]
    fn delivery_statuses_have_bounded_retry_and_expiry_classes() {
        assert_eq!(
            delivery_outcome(Ok(StatusCode::CREATED)),
            NotificationDeliveryOutcome::Delivered
        );
        assert_eq!(
            delivery_outcome(Ok(StatusCode::GONE)),
            NotificationDeliveryOutcome::Expired
        );
        assert_eq!(
            delivery_outcome(Ok(StatusCode::TOO_MANY_REQUESTS)),
            NotificationDeliveryOutcome::Retry("remote_retry")
        );
        assert_eq!(
            delivery_outcome(Ok(StatusCode::BAD_REQUEST)),
            NotificationDeliveryOutcome::Failed(Some("remote_rejected"))
        );
        assert_eq!(
            delivery_outcome(Err("offline".to_string())),
            NotificationDeliveryOutcome::Retry("transport_unavailable")
        );
        assert_eq!(
            delivery_outcome_apns(Err(ApnsSendError::InvalidToken)),
            NotificationDeliveryOutcome::InvalidToken
        );
        assert_eq!(
            delivery_outcome_apns(Ok(StatusCode::BAD_REQUEST)),
            NotificationDeliveryOutcome::Failed(Some("remote_rejected"))
        );
        assert_eq!(
            delivery_outcome_apns(Ok(StatusCode::TOO_MANY_REQUESTS)),
            NotificationDeliveryOutcome::Retry("remote_retry")
        );
    }
}
