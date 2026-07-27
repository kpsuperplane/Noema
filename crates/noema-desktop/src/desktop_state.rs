//! Managed desktop runtime state.

use std::{collections::HashMap, path::PathBuf};

use noema_api::graphql::{self, GraphqlSchema};
use noema_host::{
    NoemaHost, RuntimeHostError, start_from_process_env_with_local_model_runtime_root,
};
use tauri::async_runtime::JoinHandle;
use tokio::sync::{Mutex, watch};

pub(crate) struct DesktopState {
    inner: Mutex<DesktopLifecycle<DesktopRuntime>>,
}

impl DesktopState {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(DesktopLifecycle::Uninitialized),
        }
    }

    pub(crate) async fn initialize(
        &self,
        local_model_runtime_root: Option<PathBuf>,
    ) -> Result<(), RuntimeHostError> {
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
        let schema = graphql::build_schema(graphql_state);
        let runtime = DesktopRuntime {
            host,
            schema,
            mcp_oauth_callback_url: oauth_callback_urls.mcp,
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

    pub(crate) async fn schema(&self) -> Result<GraphqlSchema, String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its local app service.".to_string());
        };
        Ok(runtime.schema.clone())
    }

    pub(crate) async fn mcp_oauth_callback_url(&self) -> Result<String, String> {
        let inner = self.inner.lock().await;
        let DesktopLifecycle::Running(runtime) = &*inner else {
            return Err("Noema lost connection to its local app service.".to_string());
        };
        Ok(runtime.mcp_oauth_callback_url.clone())
    }

    pub(crate) async fn insert_subscription(
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

        assert_ne!(old_generation, new_generation);
        subscriptions.remove_finished("sub_1", old_generation);
        assert_eq!(subscriptions.entries["sub_1"].generation, new_generation);
        subscriptions.remove_finished("sub_1", new_generation);
        assert!(!subscriptions.entries.contains_key("sub_1"));
    }

    #[tokio::test]
    async fn shutdown_state_machine_is_idempotent_and_shares_concurrent_completion() {
        let mut uninitialized = DesktopLifecycle::<()>::Uninitialized;
        assert!(matches!(
            uninitialized.begin_shutdown(),
            ShutdownAction::Complete
        ));
        match uninitialized.begin_shutdown() {
            ShutdownAction::Wait(completion) => completion.wait().await,
            ShutdownAction::Start { .. } | ShutdownAction::Complete => {
                panic!("pre-initialization shutdown must remain complete")
            }
        }

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

    fn pending_handle() -> JoinHandle<()> {
        tauri::async_runtime::spawn(async {
            future::pending::<()>().await;
        })
    }
}
