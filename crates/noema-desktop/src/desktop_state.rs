//! Managed desktop runtime state.

use std::{collections::HashMap, path::PathBuf};

use noema_api::graphql::{self, GraphqlSchema};
use noema_host::{
    NoemaHost, RuntimeHostError, start_from_process_env_with_local_model_runtime_root,
};
use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;

/// Shared Tauri application state for desktop IPC commands.
pub struct DesktopState {
    inner: Mutex<Option<DesktopRuntime>>,
}

impl DesktopState {
    /// Create empty desktop state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
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
        let mut inner = self.inner.lock().await;
        *inner = Some(DesktopRuntime {
            host,
            schema,
            mcp_oauth_callback_url,
            mcp_oauth_callback_server,
            subscriptions: SubscriptionTasks::default(),
        });
        Ok(())
    }

    /// Return the initialized GraphQL schema.
    ///
    /// # Errors
    ///
    /// Returns a user-facing error while the desktop runtime is unavailable.
    pub async fn schema(&self) -> Result<GraphqlSchema, String> {
        let inner = self.inner.lock().await;
        inner
            .as_ref()
            .map(|runtime| runtime.schema.clone())
            .ok_or_else(|| "Noema lost connection to its local app service.".to_string())
    }

    /// Return the desktop-local MCP OAuth callback URL.
    ///
    /// # Errors
    ///
    /// Returns a user-facing error while the desktop runtime is unavailable.
    pub async fn mcp_oauth_callback_url(&self) -> Result<String, String> {
        let inner = self.inner.lock().await;
        inner
            .as_ref()
            .map(|runtime| runtime.mcp_oauth_callback_url.clone())
            .ok_or_else(|| "Noema lost connection to its local app service.".to_string())
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
        let Some(runtime) = inner.as_mut() else {
            handle.abort();
            return Err("Noema lost connection to its local app service.".to_string());
        };
        Ok(runtime.subscriptions.insert(id, handle))
    }

    /// Remove a finished GraphQL subscription task if it still owns the id.
    pub async fn remove_finished_subscription(&self, id: &str, generation: u64) {
        let mut inner = self.inner.lock().await;
        if let Some(runtime) = inner.as_mut() {
            runtime.subscriptions.remove_finished(id, generation);
        }
    }

    /// Remove and abort an active GraphQL subscription task.
    pub async fn remove_subscription(&self, id: &str) {
        let mut inner = self.inner.lock().await;
        if let Some(runtime) = inner.as_mut() {
            runtime.subscriptions.remove(id);
        }
    }

    /// Shut down runtime-owned background work.
    pub async fn shutdown(&self) {
        let runtime = {
            let mut inner = self.inner.lock().await;
            inner.take()
        };
        if let Some(runtime) = runtime {
            runtime.mcp_oauth_callback_server.abort();
            runtime.subscriptions.abort_all();
            runtime.host.shutdown().await;
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

    fn pending_handle() -> JoinHandle<()> {
        tauri::async_runtime::spawn(async {
            future::pending::<()>().await;
        })
    }
}
