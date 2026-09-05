//! Managed local and remote desktop runtime state.

use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom as _, SystemRandom};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::{
    io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, watch},
};

use crate::{
    desktop_profile::{DesktopProfileStore, DesktopSelection},
    remote_graphql::{RemoteError, RemoteGraphql},
    remote_oauth::{ConnectionStage, PendingConnection},
};

pub(crate) struct DesktopState {
    inner: Mutex<DesktopLifecycle<DesktopRuntime>>,
    profiles: Arc<DesktopProfileStore>,
    pending_connection: Mutex<Option<PendingConnection>>,
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
        server_path: PathBuf,
        local_model_runtime_root: PathBuf,
    ) -> Result<(), String> {
        let backend = match self.profiles.load() {
            DesktopSelection::Local => DesktopBackend::Local(Box::new(
                start_local(server_path, local_model_runtime_root).await?,
            )),
            DesktopSelection::Remote(profile) => {
                match RemoteGraphql::new(profile, self.profiles.clone()) {
                    Ok(remote) => DesktopBackend::Remote(remote),
                    Err(message) => DesktopBackend::Recovery { message },
                }
            }
            DesktopSelection::Recovery { message } => DesktopBackend::Recovery { message },
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
        Err("Desktop runtime initialization raced with shutdown.".to_string())
    }

    pub(crate) async fn graphql_target(&self) -> Result<(RemoteGraphql, bool), String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its app service.".to_string());
        };
        match &runtime.backend {
            DesktopBackend::Local(local) => Ok((local.transport.clone(), true)),
            DesktopBackend::Remote(remote) => Ok((remote.clone(), false)),
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
            StatusTarget::Local(local) => {
                let (state, message) = match local.health().await {
                    Ok(()) => ("ready", None),
                    Err(_) => (
                        "unavailable",
                        Some("Noema lost connection to its local app service.".to_string()),
                    ),
                };
                DesktopConnectionStatus {
                    mode: "local",
                    state,
                    origin: None,
                    message,
                    pending_connection_origin,
                }
            }
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
            StatusTarget::Recovery { message } => DesktopConnectionStatus {
                mode: "remote",
                state: "credential_unavailable",
                origin: None,
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
            DesktopBackend::Local(local) => {
                Ok(format!("{}/mcp/oauth/callback", local.transport.origin()))
            }
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
    server_path: PathBuf,
    local_model_runtime_root: PathBuf,
) -> Result<LocalRuntime, String> {
    let token = desktop_token()?;
    let mut child = Command::new(server_path)
        .args(["--migration-spike", "--desktop-sidecar"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "Noema could not start its local app service.".to_string())?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Noema could not open its local app service.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Noema could not open its local app service.".to_string())?;
    let startup = serde_json::to_vec(&SidecarStartup {
        token: &token,
        runtime_root: &local_model_runtime_root,
    })
    .map_err(|_| "Noema could not prepare its local app service.".to_string())?;
    if stdin.write_all(&startup).await.is_err()
        || stdin.write_all(b"\n").await.is_err()
        || stdin.flush().await.is_err()
    {
        let _ = child.kill().await;
        return Err("Noema could not start its local app service.".to_string());
    }
    let mut lines = BufReader::new(stdout).lines();
    let ready = match tokio::time::timeout(Duration::from_secs(30), lines.next_line()).await {
        Ok(Ok(Some(line))) if line.len() <= 4096 => {
            serde_json::from_str::<SidecarReady>(&line).ok()
        }
        _ => None,
    };
    let Some(ready) = ready.filter(|ready| ready.kind == "ready") else {
        let _ = child.kill().await;
        return Err("Noema could not start its local app service.".to_string());
    };
    let transport = match RemoteGraphql::new_local(ready.origin, token) {
        Ok(transport) => transport,
        _ => {
            let _ = child.kill().await;
            return Err("Noema could not start its local app service.".to_string());
        }
    };
    Ok(LocalRuntime {
        transport,
        child,
        stdin,
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
    Recovery { message: String },
}

impl DesktopBackend {
    fn status_target(&self) -> StatusTarget {
        match self {
            Self::Local(local) => StatusTarget::Local(local.transport.clone()),
            Self::Remote(remote) => StatusTarget::Remote(remote.clone()),
            Self::Recovery { message } => StatusTarget::Recovery {
                message: message.clone(),
            },
        }
    }
}

enum StatusTarget {
    Local(RemoteGraphql),
    Remote(RemoteGraphql),
    Recovery { message: String },
}

struct LocalRuntime {
    transport: RemoteGraphql,
    child: Child,
    stdin: ChildStdin,
}

impl LocalRuntime {
    async fn shutdown(mut self) {
        drop(self.stdin);
        if tokio::time::timeout(Duration::from_secs(12), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
            let _ = self.child.wait().await;
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarStartup<'a> {
    token: &'a str,
    runtime_root: &'a PathBuf,
}

#[derive(Deserialize)]
struct SidecarReady {
    #[serde(rename = "type")]
    kind: String,
    origin: String,
}

fn desktop_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Noema could not create its local app credential.".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
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
