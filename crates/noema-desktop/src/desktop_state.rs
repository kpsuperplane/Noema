//! Managed desktop runtime state.

use noema_core::{
    NoemaRuntimeHost, RuntimeHostError,
    graphql::{self, GraphqlSchema},
};
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
        codex: noema_core::CodexProviderConfig,
    ) -> Result<(), RuntimeHostError> {
        let host = NoemaRuntimeHost::start(codex).await?;
        let schema = graphql::build_schema(graphql::GraphqlState::from_runtime_host(&host));
        let mut inner = self.inner.lock().await;
        *inner = Some(DesktopRuntime { host, schema });
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

    /// Shut down runtime-owned background work.
    pub async fn shutdown(&self) {
        let runtime = {
            let mut inner = self.inner.lock().await;
            inner.take()
        };
        if let Some(runtime) = runtime {
            runtime.host.shutdown().await;
        }
    }
}

struct DesktopRuntime {
    host: NoemaRuntimeHost,
    schema: GraphqlSchema,
}
