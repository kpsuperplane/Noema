use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderError, ProviderHandle,
    ProviderInstanceKey, ProviderOperations, ProviderRegistry, ProviderRouteFuture,
    ProviderSelectionLoader, ProviderSelectionSnapshot, RegistryProviderRouteResolver,
};
use serde_json::Value;

use super::{MemoryModelProxy, MemoryModelProxyConfig};

#[derive(Debug)]
pub(crate) struct CapturingProvider {
    response: GenerateResponse,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl CapturingProvider {
    pub(crate) fn new(response: GenerateResponse) -> Self {
        Self {
            response,
            requests: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn requests(&self) -> Vec<GenerateRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl ProviderOperations for CapturingProvider {
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests.lock().expect("requests lock").push(request);
            Ok(self.response.clone())
        })
    }
}

#[derive(Debug)]
pub(crate) struct BlockingProvider {
    pub(crate) started: Arc<tokio::sync::Notify>,
    pub(crate) release: Arc<tokio::sync::Notify>,
}

impl ProviderOperations for BlockingProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.started.notify_one();
            self.release.notified().await;
            Ok(GenerateResponse::final_text("old", "codex", "memory-model"))
        })
    }
}

#[derive(Debug)]
pub(crate) struct FakeSelectionRepository(Mutex<ProviderSelectionSnapshot>);

impl FakeSelectionRepository {
    pub(crate) fn new(selection: ProviderSelectionSnapshot) -> Self {
        Self(Mutex::new(selection))
    }

    pub(crate) fn save(&self, selection: ProviderSelectionSnapshot) {
        *self.0.lock().expect("selection lock") = selection;
    }
}

impl ProviderSelectionLoader for FakeSelectionRepository {
    fn load_selection(&self) -> ProviderRouteFuture<'_, ProviderSelectionSnapshot> {
        let selection = self.0.lock().expect("selection lock").clone();
        Box::pin(async move { Ok(selection) })
    }
}

pub(crate) fn instance_key(value: &str) -> ProviderInstanceKey {
    ProviderInstanceKey::new(value).expect("provider instance key")
}

pub(crate) fn selection(
    provider_kind: &str,
    provider_account_id: &str,
    key: ProviderInstanceKey,
    model_profile: &str,
) -> ProviderSelectionSnapshot {
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        None,
        Some("memory_proxy_test".to_string()),
    );
    selection.provider_instance_key = Some(key);
    selection
}

pub(crate) fn proxy_config(
    provider: ProviderHandle,
    model_profile: &str,
) -> MemoryModelProxyConfig {
    let key = instance_key("codex:memory-proxy:test");
    let registry = Arc::new(ProviderRegistry::new());
    registry
        .register(key.clone(), provider)
        .expect("register provider");
    let repository = Arc::new(FakeSelectionRepository::new(selection(
        "codex",
        "provider_account:codex:default",
        key,
        model_profile,
    )));
    MemoryModelProxyConfig {
        route_resolver: Arc::new(RegistryProviderRouteResolver::new(repository, registry)),
        api_key: "secret".to_string(),
        model_profile: model_profile.to_string(),
        system_errors: None,
    }
}

pub(crate) async fn start_test_proxy() -> (MemoryModelProxy, String) {
    static NEXT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let api_key = format!(
        "proxy-secret-{}",
        NEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
        "ok",
        "test",
        "memory-model",
    )));
    let mut config = proxy_config(provider, "memory-model");
    config.api_key.clone_from(&api_key);
    let proxy = MemoryModelProxy::start(config).await.expect("start proxy");
    (proxy, api_key)
}

pub(crate) async fn post_chat(proxy: &MemoryModelProxy, payload: Value) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{}/chat/completions", proxy.openai_base_url()))
        .bearer_auth(proxy.api_key())
        .json(&payload)
        .send()
        .await
        .expect("proxy request")
}
