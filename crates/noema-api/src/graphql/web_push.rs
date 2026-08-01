//! Installed-web notification registration, presence, projection, and delivery.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_graphql::{InputObject, Result, SimpleObject};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::Stream;
use noema_capabilities::web::url_policy::{is_public_ip, validate_public_url};
use noema_conversations::{ConversationItemKind, ReplayMode};
use noema_runtime::ConversationRuntimeEvent;
use noema_store::{ClaimedWebPushDelivery, NewWebPushSubscription, NoemaStore};
use reqwest::{Client, redirect::Policy};
use tokio::sync::Notify;
use url::{Host, Url};
use web_push_native::{
    Auth, WebPushBuilder, jwt_simple::algorithms::ES256KeyPair, p256::PublicKey,
};

use super::{
    errors::graphql_error,
    human_interventions::{self, GraphqlHumanIntervention},
    runtime_state::GraphqlState,
};

const LOCAL_HUMAN_ID: &str = "human:local";
const MAX_PREVIEW_BYTES: usize = 600;

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

/// Installation-scoped Web Push projection, presence, and delivery service.
#[derive(Clone)]
pub struct WebPushCoordinator {
    inner: Arc<WebPushInner>,
}

struct WebPushInner {
    store: NoemaStore,
    public_origin: String,
    identity: noema_store::WebPushIdentity,
    visible: Mutex<HashMap<String, usize>>,
    wake: Notify,
}

impl std::fmt::Debug for WebPushCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WebPushCoordinator")
    }
}

impl WebPushCoordinator {
    /// Create or reopen the installation's stable VAPID identity.
    ///
    /// # Errors
    /// Returns a configuration or storage error when HTTPS Web Push cannot be initialized.
    pub async fn new(
        store: NoemaStore,
        public_origin: String,
    ) -> std::result::Result<Self, String> {
        let origin = Url::parse(&public_origin).map_err(|_| "invalid Web Push origin")?;
        if origin.scheme() != "https" || origin.host_str().is_none() {
            return Err("Web Push requires an HTTPS public origin".to_string());
        }
        let generated = ES256KeyPair::generate();
        let identity = store
            .get_or_insert_web_push_identity(
                &generated.to_bytes(),
                &generated.public_key().to_bytes(),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            inner: Arc::new(WebPushInner {
                store,
                public_origin,
                identity,
                visible: Mutex::new(HashMap::new()),
                wake: Notify::new(),
            }),
        })
    }

    pub(super) async fn status(
        &self,
        owner_human_id: &str,
        endpoint: Option<&str>,
    ) -> Result<GraphqlWebPushStatus> {
        let subscription = match endpoint {
            Some(endpoint) => self
                .inner
                .store
                .web_push_subscription_for_endpoint(owner_human_id, endpoint)
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
        input: GraphqlRegisterWebPushSubscriptionInput,
    ) -> Result<GraphqlWebPushStatus> {
        validate_subscription_material(&input)?;
        let endpoint = validate_push_endpoint(&input.endpoint)?;
        self.inner
            .store
            .register_web_push_subscription(NewWebPushSubscription {
                owner_human_id: owner_human_id.to_string(),
                endpoint: endpoint.to_string(),
                p256dh: input.p256dh,
                auth_secret: input.auth,
            })
            .await
            .map_err(graphql_error)?;
        self.status(owner_human_id, Some(endpoint.as_str())).await
    }

    pub(super) async fn remove(&self, owner_human_id: &str, subscription_id: &str) -> Result<bool> {
        self.inner
            .store
            .remove_web_push_subscription(owner_human_id, subscription_id)
            .await
            .map_err(graphql_error)
    }

    pub(super) async fn presence(
        &self,
        owner_human_id: &str,
        subscription_id: String,
    ) -> Result<impl Stream<Item = GraphqlWebPushPresenceEvent> + use<>> {
        let ids = self
            .inner
            .store
            .web_push_subscription_ids(owner_human_id)
            .await
            .map_err(graphql_error)?;
        if !ids.iter().any(|candidate| candidate == &subscription_id) {
            return Err(async_graphql::Error::new(
                "Web Push subscription is unavailable",
            ));
        }
        let lease = VisibilityLease::new(self.inner.clone(), subscription_id.clone());
        Ok(async_stream::stream! {
            let _lease = lease;
            yield GraphqlWebPushPresenceEvent { subscription_id, ready: true };
            std::future::pending::<()>().await;
        })
    }

    /// Run the event-driven projection and bounded delivery loop.
    pub async fn run(
        self,
        state: GraphqlState,
        mut events: tokio::sync::broadcast::Receiver<ConversationRuntimeEvent>,
    ) {
        let _ = self.reconcile_primary_chat(&state).await;
        let _ = self.reconcile_interventions(&state).await;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => self.handle_event(&state, event).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let _ = self.reconcile_primary_chat(&state).await;
                        let _ = self.reconcile_interventions(&state).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                () = self.inner.wake.notified() => {},
                () = tokio::time::sleep(Duration::from_secs(30)) => {},
            }
            let _ = self.reconcile_primary_chat(&state).await;
            let _ = self.reconcile_interventions(&state).await;
            self.drain_due().await;
        }
    }

    async fn handle_event(&self, state: &GraphqlState, event: ConversationRuntimeEvent) {
        match event {
            ConversationRuntimeEvent::HumanInterventionsChanged { .. } => {
                let _ = self.reconcile_interventions(state).await;
            }
            ConversationRuntimeEvent::Turn { .. } | ConversationRuntimeEvent::Completed { .. } => {
                let _ = self.reconcile_primary_chat(state).await;
            }
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
            .web_push_primary_checkpoint()
            .await
            .map_err(graphql_error)?;
        if checkpoint.conversation_id.as_deref() != Some(&conversation.conversation_id) {
            self.inner
                .store
                .advance_web_push_primary_checkpoint(&conversation.conversation_id, latest_sequence)
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
                self.inner
                    .store
                    .queue_web_push_notification(
                        LOCAL_HUMAN_ID,
                        &event_key,
                        &title,
                        &body,
                        "normal",
                        3600,
                        &visible,
                    )
                    .await
                    .map_err(graphql_error)?;
            }
        }
        self.inner
            .store
            .advance_web_push_primary_checkpoint(&conversation.conversation_id, latest_sequence)
            .await
            .map_err(graphql_error)?;
        self.inner.wake.notify_one();
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
            .web_push_attention_seeded()
            .await
            .map_err(graphql_error)?;
        let visible = self.visible_subscriptions();
        for intervention in interventions {
            let (key, title, body) = intervention_notification(&intervention);
            let is_new = self
                .inner
                .store
                .observe_web_push_attention(&key)
                .await
                .map_err(graphql_error)?;
            if seeded && is_new {
                self.inner
                    .store
                    .queue_web_push_notification(
                        LOCAL_HUMAN_ID,
                        &format!("attention:{key}"),
                        &title,
                        &preview(&body),
                        "high",
                        86_400,
                        &visible,
                    )
                    .await
                    .map_err(graphql_error)?;
            }
        }
        if !seeded {
            self.inner
                .store
                .mark_web_push_attention_seeded()
                .await
                .map_err(graphql_error)?;
        }
        self.inner.wake.notify_one();
        Ok(())
    }

    async fn drain_due(&self) {
        while let Ok(Some(delivery)) = self.inner.store.claim_due_web_push_delivery().await {
            if self.is_visible(&delivery.subscription.subscription_id) {
                let _ = self
                    .inner
                    .store
                    .finish_web_push_delivery(
                        &delivery.subscription.subscription_id,
                        &delivery.event_key,
                        "suppressed",
                        None,
                    )
                    .await;
                continue;
            }
            let (disposition, code) = delivery_disposition(self.send(delivery.clone()).await);
            let _ = self
                .inner
                .store
                .finish_web_push_delivery(
                    &delivery.subscription.subscription_id,
                    &delivery.event_key,
                    disposition,
                    code,
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
            |visible| visible.keys().cloned().collect(),
        )
    }

    fn is_visible(&self, subscription_id: &str) -> bool {
        self.inner
            .visible
            .lock()
            .is_ok_and(|visible| visible.contains_key(subscription_id))
    }
}

struct VisibilityLease {
    inner: Arc<WebPushInner>,
    subscription_id: String,
}

impl VisibilityLease {
    fn new(inner: Arc<WebPushInner>, subscription_id: String) -> Self {
        if let Ok(mut visible) = inner.visible.lock() {
            *visible.entry(subscription_id.clone()).or_default() += 1;
        }
        Self {
            inner,
            subscription_id,
        }
    }
}

impl Drop for VisibilityLease {
    fn drop(&mut self) {
        if let Ok(mut visible) = self.inner.visible.lock()
            && let Some(count) = visible.get_mut(&self.subscription_id)
        {
            *count -= 1;
            if *count == 0 {
                visible.remove(&self.subscription_id);
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

fn preview(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= MAX_PREVIEW_BYTES {
        return normalized;
    }
    let mut end = MAX_PREVIEW_BYTES;
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
        preview(item.content_text.as_deref()?),
    ))
}

fn delivery_disposition(
    result: std::result::Result<reqwest::StatusCode, String>,
) -> (&'static str, Option<&'static str>) {
    match result {
        Ok(status) if status.is_success() => ("delivered", None),
        Ok(status) if matches!(status.as_u16(), 404 | 410) => ("expired", Some("expired")),
        Ok(status) if status.as_u16() == 429 || status.is_server_error() => {
            ("retry", Some("remote_retry"))
        }
        Ok(_) => ("failed", Some("remote_rejected")),
        Err(_) => ("retry", Some("transport_unavailable")),
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
    use noema_conversations::{
        ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    };
    use reqwest::StatusCode;
    use serde_json::json;

    use super::*;

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
            payload_json: json!({}),
            metadata: json!({"phase": phase}),
        }
    }

    #[test]
    fn only_final_primary_chat_text_becomes_a_notification() {
        assert_eq!(
            primary_chat_notification(&assistant_item("final_answer")),
            Some((
                "chat-turn:turn:one".to_string(),
                "Ready for you.".to_string()
            ))
        );
        assert_eq!(
            primary_chat_notification(&assistant_item("commentary")),
            None
        );
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
    fn delivery_statuses_have_bounded_retry_and_expiry_classes() {
        assert_eq!(
            delivery_disposition(Ok(StatusCode::CREATED)),
            ("delivered", None)
        );
        assert_eq!(
            delivery_disposition(Ok(StatusCode::GONE)),
            ("expired", Some("expired"))
        );
        assert_eq!(
            delivery_disposition(Ok(StatusCode::TOO_MANY_REQUESTS)),
            ("retry", Some("remote_retry"))
        );
        assert_eq!(
            delivery_disposition(Ok(StatusCode::BAD_REQUEST)),
            ("failed", Some("remote_rejected"))
        );
        assert_eq!(
            delivery_disposition(Err("offline".to_string())),
            ("retry", Some("transport_unavailable"))
        );
    }
}
