//! Managed desktop runtime state.

use std::{collections::HashMap, path::PathBuf};

use noema_api::graphql::{self, GraphqlSchema};
use noema_host::{
    NoemaHost, RuntimeHostError, start_from_process_env_with_local_model_runtime_root,
};
use tauri::async_runtime::JoinHandle;
use tokio::sync::{Mutex, watch};

/// Shared Tauri application state for desktop IPC commands.
pub struct DesktopState {
    inner: Mutex<DesktopLifecycle<DesktopRuntime>>,
}

impl DesktopState {
    /// Create empty desktop state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DesktopLifecycle::Uninitialized),
        }
    }

    /// Initialize the local runtime host and GraphQL schema.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeHostError`] if Noema cannot open its data folder, store,
    /// or runtime provider.
    pub async fn initialize(
        &self,
        local_model_runtime_root: Option<PathBuf>,
    ) -> Result<(), RuntimeHostError> {
        let host =
            start_from_process_env_with_local_model_runtime_root(local_model_runtime_root).await?;
        let graphql_state = graphql::GraphqlState::from_host_services(host.services());
        let schema = graphql::build_schema(graphql_state.clone());
        let (mcp_oauth_callback_url, mcp_oauth_callback_server) =
            match crate::mcp_oauth_callback::start(graphql_state).await {
                Ok(callback) => callback,
                Err(error) => {
                    host.shutdown().await;
                    return Err(RuntimeHostError::Composition(error));
                }
            };
        let runtime = DesktopRuntime {
            host,
            schema,
            mcp_oauth_callback_url,
            mcp_oauth_callback_server,
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

    /// Return the initialized GraphQL schema.
    ///
    /// # Errors
    ///
    /// Returns a user-facing error while the desktop runtime is unavailable.
    pub async fn schema(&self) -> Result<GraphqlSchema, String> {
        let inner = self.inner.lock().await;
        match &*inner {
            DesktopLifecycle::Running(runtime) => Ok(runtime.schema.clone()),
            DesktopLifecycle::Uninitialized | DesktopLifecycle::Shutdown(_) => {
                Err("Noema lost connection to its local app service.".to_string())
            }
        }
    }

    /// Return the desktop-local MCP OAuth callback URL.
    ///
    /// # Errors
    ///
    /// Returns a user-facing error while the desktop runtime is unavailable.
    pub async fn mcp_oauth_callback_url(&self) -> Result<String, String> {
        let inner = self.inner.lock().await;
        match &*inner {
            DesktopLifecycle::Running(runtime) => Ok(runtime.mcp_oauth_callback_url.clone()),
            DesktopLifecycle::Uninitialized | DesktopLifecycle::Shutdown(_) => {
                Err("Noema lost connection to its local app service.".to_string())
            }
        }
    }

    /// Store an active GraphQL subscription task.
    ///
    /// Replacing an existing subscription id aborts the old task.
    ///
    /// # Errors
    ///
    /// Returns a user-facing error while the desktop runtime is unavailable.
    pub async fn insert_subscription(
        &self,
        id: String,
        handle: JoinHandle<()>,
    ) -> Result<u64, String> {
        let mut inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &mut *inner else {
            handle.abort();
            return Err("Noema lost connection to its local app service.".to_string());
        };
        Ok(runtime.subscriptions.insert(id, handle))
    }

    /// Remove a finished GraphQL subscription task if it still owns the id.
    pub async fn remove_finished_subscription(&self, id: &str, generation: u64) {
        let mut inner = self.inner.lock().await;
        if let DesktopLifecycle::Running(runtime) = &mut *inner {
            runtime.subscriptions.remove_finished(id, generation);
        }
    }

    /// Remove and abort an active GraphQL subscription task.
    pub async fn remove_subscription(&self, id: &str) {
        let mut inner = self.inner.lock().await;
        if let DesktopLifecycle::Running(runtime) = &mut *inner {
            runtime.subscriptions.remove(id);
        }
    }

    /// Shut down runtime-owned background work and wait for the shared teardown.
    pub async fn shutdown(&self) {
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

struct DesktopRuntime {
    host: NoemaHost,
    schema: GraphqlSchema,
    mcp_oauth_callback_url: String,
    mcp_oauth_callback_server: JoinHandle<()>,
    subscriptions: SubscriptionTasks,
}

impl DesktopRuntime {
    async fn shutdown(self) {
        self.mcp_oauth_callback_server.abort();
        self.subscriptions.abort_all();
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

    fn remove_finished(&mut self, id: &str, generation: u64) -> bool {
        let should_remove = self
            .entries
            .get(id)
            .is_some_and(|entry| entry.generation == generation);
        if should_remove {
            self.entries.remove(id);
        }
        should_remove
    }

    fn abort_all(self) {
        for (_, entry) in self.entries {
            entry.handle.abort();
        }
    }

    #[cfg(test)]
    fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    #[cfg(test)]
    fn generation(&self, id: &str) -> Option<u64> {
        self.entries.get(id).map(|entry| entry.generation)
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
    async fn finished_subscription_cleanup_removes_matching_generation() {
        let mut subscriptions = SubscriptionTasks::default();
        let generation = subscriptions.insert("sub_1".to_string(), pending_handle());

        assert!(subscriptions.remove_finished("sub_1", generation));
        assert!(!subscriptions.contains("sub_1"));
    }

    #[tokio::test]
    async fn finished_subscription_cleanup_preserves_newer_replacement() {
        let mut subscriptions = SubscriptionTasks::default();
        let old_generation = subscriptions.insert("sub_1".to_string(), pending_handle());
        let new_generation = subscriptions.insert("sub_1".to_string(), pending_handle());

        assert_ne!(old_generation, new_generation);
        assert!(!subscriptions.remove_finished("sub_1", old_generation));
        assert_eq!(subscriptions.generation("sub_1"), Some(new_generation));

        subscriptions.remove("sub_1");
    }

    #[tokio::test]
    async fn concurrent_shutdown_callers_wait_for_the_same_completion() {
        let mut lifecycle = DesktopLifecycle::Running(());
        let (owner_completion, signal) = match lifecycle.begin_shutdown() {
            ShutdownAction::Start {
                runtime: (),
                completion,
                signal,
            } => (completion, signal),
            ShutdownAction::Wait(_) | ShutdownAction::Complete => {
                panic!("first shutdown caller must own teardown")
            }
        };
        let waiting_completion = match lifecycle.begin_shutdown() {
            ShutdownAction::Wait(completion) => completion,
            ShutdownAction::Start { .. } | ShutdownAction::Complete => {
                panic!("concurrent shutdown caller must wait")
            }
        };
        let waiting_caller = tokio::spawn(waiting_completion.wait());

        tokio::task::yield_now().await;
        assert!(!waiting_caller.is_finished());

        signal.send_replace(true);
        owner_completion.wait().await;
        waiting_caller.await.expect("waiting shutdown caller");

        match lifecycle.begin_shutdown() {
            ShutdownAction::Wait(completion) => completion.wait().await,
            ShutdownAction::Start { .. } | ShutdownAction::Complete => {
                panic!("completed shutdown must remain a shared one-shot")
            }
        }
    }

    #[tokio::test]
    async fn shutdown_before_initialization_remains_complete() {
        let mut lifecycle = DesktopLifecycle::<()>::Uninitialized;
        assert!(matches!(
            lifecycle.begin_shutdown(),
            ShutdownAction::Complete
        ));

        match lifecycle.begin_shutdown() {
            ShutdownAction::Wait(completion) => completion.wait().await,
            ShutdownAction::Start { .. } | ShutdownAction::Complete => {
                panic!("pre-initialization shutdown must remain complete")
            }
        }
    }

    fn pending_handle() -> JoinHandle<()> {
        tauri::async_runtime::spawn(async {
            future::pending::<()>().await;
        })
    }
}
