//! Managed local and remote desktop runtime state.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use noema_api::graphql::{self, GraphqlSchema};
use noema_host::{
    NoemaHost, RuntimeHostError, start_from_process_env_with_local_model_runtime_root,
};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::{Mutex, watch};

use crate::{
    desktop_profile::{DesktopProfileStore, DesktopSelection, RemoteMetadata},
    remote_graphql::{RemoteError, RemoteGraphql},
    remote_oauth::{ConnectionStage, PendingConnection},
};

pub(crate) struct DesktopState {
    inner: Mutex<DesktopLifecycle<DesktopRuntime>>,
    profiles: Arc<DesktopProfileStore>,
    pending_connection: Mutex<Option<PendingConnection>>,
}

#[derive(Clone)]
pub(crate) enum GraphqlTarget {
    Local(GraphqlSchema),
    Remote(RemoteGraphql),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopConnectionStatus {
    mode: &'static str,
    state: &'static str,
    origin: Option<String>,
    message: Option<String>,
    pending_connection_origin: Option<String>,
}

impl DesktopState {
    #[must_use]
    pub(crate) fn new(config_path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(DesktopLifecycle::Uninitialized),
            profiles: Arc::new(DesktopProfileStore::new(config_path)),
            pending_connection: Mutex::new(None),
        }
    }

    pub(crate) async fn initialize(
        &self,
        local_model_runtime_root: Option<PathBuf>,
    ) -> Result<(), RuntimeHostError> {
        let backend = match self.profiles.load() {
            DesktopSelection::Local => {
                DesktopBackend::Local(Box::new(start_local(local_model_runtime_root).await?))
            }
            DesktopSelection::Remote(profile) => {
                match RemoteGraphql::new(profile, self.profiles.clone()) {
                    Ok(remote) => DesktopBackend::Remote(remote),
                    Err(message) => DesktopBackend::Recovery {
                        metadata: None,
                        message,
                    },
                }
            }
            DesktopSelection::Recovery { metadata, message } => {
                DesktopBackend::Recovery { metadata, message }
            }
        };
        let runtime = DesktopRuntime {
            backend,
            subscriptions: SubscriptionTasks::default(),
        };
        let mut inner = self.inner.lock().await;
        if matches!(&*inner, DesktopLifecycle::Uninitialized) {
            *inner = DesktopLifecycle::Running(runtime);
            return Ok(());
        }
        drop(inner);
        runtime.shutdown().await;
        Err(RuntimeHostError::Composition(
            "desktop runtime initialization raced with shutdown".to_string(),
        ))
    }

    pub(crate) async fn graphql_target(&self) -> Result<GraphqlTarget, String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its app service.".to_string());
        };
        match &runtime.backend {
            DesktopBackend::Local(local) => Ok(GraphqlTarget::Local(local.schema.clone())),
            DesktopBackend::Remote(remote) => Ok(GraphqlTarget::Remote(remote.clone())),
            DesktopBackend::Recovery { message, .. } => Err(message.clone()),
        }
    }

    pub(crate) async fn connection_status(&self) -> DesktopConnectionStatus {
        let pending_connection_origin = self
            .pending_connection
            .lock()
            .await
            .as_ref()
            .map(|connection| connection.stage().origin);
        let backend = {
            let inner = self.inner.lock().await;
            let DesktopLifecycle::Running(runtime) = &*inner else {
                return DesktopConnectionStatus {
                    mode: "local",
                    state: "unavailable",
                    origin: None,
                    message: Some("Noema lost connection to its app service.".to_string()),
                    pending_connection_origin,
                };
            };
            runtime.backend.status_target()
        };
        match backend {
            StatusTarget::Local => DesktopConnectionStatus {
                mode: "local",
                state: "ready",
                origin: None,
                message: None,
                pending_connection_origin,
            },
            StatusTarget::Remote(remote) => {
                let (state, message) = match remote.health().await {
                    Ok(()) => ("ready", None),
                    Err(RemoteError::Unauthorized) => (
                        "unauthorized",
                        Some("This desktop client no longer has access to the server.".to_string()),
                    ),
                    Err(RemoteError::Offline) => (
                        "offline",
                        Some("Noema could not reach the remote server.".to_string()),
                    ),
                    Err(RemoteError::InvalidResponse) => (
                        "unavailable",
                        Some("The remote server returned an invalid response.".to_string()),
                    ),
                };
                DesktopConnectionStatus {
                    mode: "remote",
                    state,
                    origin: Some(remote.origin().to_string()),
                    message,
                    pending_connection_origin,
                }
            }
            StatusTarget::Recovery { metadata, message } => DesktopConnectionStatus {
                mode: "remote",
                state: "credential_unavailable",
                origin: metadata.map(|metadata| metadata.origin),
                message: Some(message),
                pending_connection_origin,
            },
        }
    }

    pub(crate) async fn stage_connection(&self, server: &str) -> Result<ConnectionStage, String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its app service.".to_string());
        };
        if !matches!(runtime.backend, DesktopBackend::Local(_)) {
            return Err("Use local Noema before you connect another server.".to_string());
        }
        drop(inner);
        let connection = PendingConnection::parse(server)?;
        let stage = connection.stage();
        *self.pending_connection.lock().await = Some(connection);
        Ok(stage)
    }

    pub(crate) async fn cancel_connection(&self) {
        *self.pending_connection.lock().await = None;
    }

    pub(crate) async fn complete_connection(&self) -> Result<(), String> {
        let connection = self
            .pending_connection
            .lock()
            .await
            .take()
            .ok_or_else(|| "The server connection is no longer available.".to_string())?;
        let profile = connection.authorize().await?;
        if let Err(error) = self.profiles.save_remote(&profile) {
            if let Ok(remote) = RemoteGraphql::new(profile, self.profiles.clone()) {
                let _ = remote.revoke_self().await;
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) async fn disconnect_remote(&self) -> Result<(), String> {
        let remote = {
            let inner = self.inner.lock().await;
            let DesktopLifecycle::Running(runtime) = &*inner else {
                return Err("Noema lost connection to its app service.".to_string());
            };
            match &runtime.backend {
                DesktopBackend::Remote(remote) => remote.clone(),
                DesktopBackend::Recovery { .. } => {
                    return Err(
                        "Noema cannot reach the saved server. Forget it to use local Noema."
                            .to_string(),
                    );
                }
                DesktopBackend::Local(_) => return Ok(()),
            }
        };
        remote_allows_local(remote.revoke_self().await)?;
        self.profiles.clear_remote()
    }

    pub(crate) fn forget_remote(&self) -> Result<(), String> {
        self.profiles.clear_remote()
    }

    pub(crate) async fn mcp_oauth_callback_url(&self) -> Result<String, String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its app service.".to_string());
        };
        match &runtime.backend {
            DesktopBackend::Local(local) => Ok(local.mcp_oauth_callback_url.clone()),
            DesktopBackend::Remote(remote) => Ok(format!("{}/mcp/oauth/callback", remote.origin())),
            DesktopBackend::Recovery { message, .. } => Err(message.clone()),
        }
    }

    pub(crate) async fn insert_subscription(
        &self,
        id: String,
        handle: JoinHandle<()>,
    ) -> Result<u64, String> {
        let mut inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &mut *inner else {
            handle.abort();
            return Err("Noema lost connection to its app service.".to_string());
        };
        Ok(runtime.subscriptions.insert(id, handle))
    }

    pub(crate) async fn remove_finished_subscription(&self, id: &str, generation: u64) {
        let mut inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &mut *inner else {
            return;
        };
        runtime.subscriptions.remove_finished(id, generation);
    }

    pub(crate) async fn remove_subscription(&self, id: &str) {
        let mut inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &mut *inner else {
            return;
        };
        runtime.subscriptions.remove(id);
    }

    pub(crate) async fn shutdown(&self) {
        let action = {
            let mut inner = self.inner.lock().await;
            inner.begin_shutdown()
        };
        match action {
            ShutdownAction::Start {
                runtime,
                completion,
                signal,
            } => {
                tauri::async_runtime::spawn(async move {
                    runtime.shutdown().await;
                    signal.send_replace(true);
                });
                completion.wait().await;
            }
            ShutdownAction::Wait(completion) => completion.wait().await,
            ShutdownAction::Complete => {}
        }
    }
}

fn remote_allows_local(result: Result<(), RemoteError>) -> Result<(), String> {
    match result {
        Ok(()) | Err(RemoteError::Unauthorized) => Ok(()),
        Err(RemoteError::Offline | RemoteError::InvalidResponse) => Err(
            "Noema could not revoke this client. Forget the server only if you cannot reconnect."
                .to_string(),
        ),
    }
}

async fn start_local(
    local_model_runtime_root: Option<PathBuf>,
) -> Result<LocalRuntime, RuntimeHostError> {
    let host =
        start_from_process_env_with_local_model_runtime_root(local_model_runtime_root).await?;
    let graphql_state = graphql::GraphqlState::from_host_services(host.services());
    let (graphql_state, oauth_callback_urls, mcp_oauth_callback_server) =
        match crate::mcp_oauth_callback::start(graphql_state.clone()).await {
            Ok(callback) => callback,
            Err(error) => {
                host.shutdown().await;
                return Err(RuntimeHostError::Composition(error));
            }
        };
    Ok(LocalRuntime {
        host,
        schema: graphql::build_schema(graphql_state),
        mcp_oauth_callback_url: oauth_callback_urls.mcp,
        mcp_oauth_callback_server,
    })
}

struct DesktopRuntime {
    backend: DesktopBackend,
    subscriptions: SubscriptionTasks,
}

impl DesktopRuntime {
    async fn shutdown(self) {
        self.subscriptions.abort_all();
        if let DesktopBackend::Local(local) = self.backend {
            local.shutdown().await;
        }
    }
}

enum DesktopBackend {
    Local(Box<LocalRuntime>),
    Remote(RemoteGraphql),
    Recovery {
        metadata: Option<RemoteMetadata>,
        message: String,
    },
}

impl DesktopBackend {
    fn status_target(&self) -> StatusTarget {
        match self {
            Self::Local(_) => StatusTarget::Local,
            Self::Remote(remote) => StatusTarget::Remote(remote.clone()),
            Self::Recovery { metadata, message } => StatusTarget::Recovery {
                metadata: metadata.clone(),
                message: message.clone(),
            },
        }
    }
}

enum StatusTarget {
    Local,
    Remote(RemoteGraphql),
    Recovery {
        metadata: Option<RemoteMetadata>,
        message: String,
    },
}

struct LocalRuntime {
    host: NoemaHost,
    schema: GraphqlSchema,
    mcp_oauth_callback_url: String,
    mcp_oauth_callback_server: JoinHandle<()>,
}

impl LocalRuntime {
    async fn shutdown(self) {
        self.mcp_oauth_callback_server.abort();
        self.host.shutdown().await;
    }
}

enum DesktopLifecycle<T> {
    Uninitialized,
    Running(T),
    Shutdown(ShutdownCompletion),
}

impl<T> DesktopLifecycle<T> {
    fn begin_shutdown(&mut self) -> ShutdownAction<T> {
        match std::mem::replace(self, Self::Uninitialized) {
            Self::Uninitialized => {
                let (_, receiver) = watch::channel(true);
                *self = Self::Shutdown(ShutdownCompletion { receiver });
                ShutdownAction::Complete
            }
            Self::Running(runtime) => {
                let (signal, receiver) = watch::channel(false);
                let completion = ShutdownCompletion { receiver };
                *self = Self::Shutdown(completion.clone());
                ShutdownAction::Start {
                    runtime,
                    completion,
                    signal,
                }
            }
            Self::Shutdown(completion) => {
                *self = Self::Shutdown(completion.clone());
                ShutdownAction::Wait(completion)
            }
        }
    }
}

enum ShutdownAction<T> {
    Start {
        runtime: T,
        completion: ShutdownCompletion,
        signal: watch::Sender<bool>,
    },
    Wait(ShutdownCompletion),
    Complete,
}

#[derive(Clone)]
struct ShutdownCompletion {
    receiver: watch::Receiver<bool>,
}

impl ShutdownCompletion {
    async fn wait(mut self) {
        while !*self.receiver.borrow() {
            if self.receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

#[derive(Default)]
struct SubscriptionTasks {
    next_generation: u64,
    entries: HashMap<String, SubscriptionTask>,
}

impl SubscriptionTasks {
    fn insert(&mut self, id: String, handle: JoinHandle<()>) -> u64 {
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("subscription generation counter exhausted");
        let generation = self.next_generation;
        if let Some(previous) = self
            .entries
            .insert(id, SubscriptionTask { generation, handle })
        {
            previous.handle.abort();
        }
        generation
    }

    fn remove(&mut self, id: &str) {
        if let Some(entry) = self.entries.remove(id) {
            entry.handle.abort();
        }
    }

    fn remove_finished(&mut self, id: &str, generation: u64) {
        if self
            .entries
            .get(id)
            .is_some_and(|entry| entry.generation == generation)
        {
            self.entries.remove(id);
        }
    }

    fn abort_all(self) {
        for entry in self.entries.into_values() {
            entry.handle.abort();
        }
    }
}

struct SubscriptionTask {
    generation: u64,
    handle: JoinHandle<()>,
}

#[cfg(test)]
mod tests {
    use std::future;

    use super::*;

    #[tokio::test]
    async fn finished_subscription_cleanup_preserves_newer_replacement() {
        let mut subscriptions = SubscriptionTasks::default();
        let old_generation = subscriptions.insert("sub_1".to_string(), pending_handle());
        let new_generation = subscriptions.insert("sub_1".to_string(), pending_handle());
        subscriptions.remove_finished("sub_1", old_generation);
        assert_eq!(subscriptions.entries["sub_1"].generation, new_generation);
        subscriptions.remove_finished("sub_1", new_generation);
        assert!(subscriptions.entries.is_empty());
    }

    #[test]
    fn local_mode_requires_revocation_or_explicit_forget() {
        assert_eq!(remote_allows_local(Ok(())), Ok(()));
        assert_eq!(remote_allows_local(Err(RemoteError::Unauthorized)), Ok(()));
        assert!(remote_allows_local(Err(RemoteError::Offline)).is_err());
        assert!(remote_allows_local(Err(RemoteError::InvalidResponse)).is_err());
    }

    fn pending_handle() -> JoinHandle<()> {
        tauri::async_runtime::spawn(future::pending())
    }
}
