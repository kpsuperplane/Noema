//! Private OpenAI-compatible model proxy for managed memory services.

pub(crate) mod protocol;
mod server;
mod translation;

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

use noema_home::{SystemErrorEvent, SystemErrorLogger};
use noema_providers::ProviderRouteResolverHandle;
use thiserror::Error;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

/// Configuration for Noema's private memory model proxy.
#[derive(Clone)]
pub struct MemoryModelProxyConfig {
    /// Fresh provider route bound to Settings > Memory.
    pub route_resolver: ProviderRouteResolverHandle,
    /// Bearer token accepted from the memory child process.
    pub api_key: String,
    /// Model/profile selected by Settings > Memory.
    pub model_profile: String,
    /// Developer diagnostics sink.
    pub system_errors: Option<SystemErrorLogger>,
}

/// Private loopback OpenAI-compatible proxy owned by managed memory services.
pub struct MemoryModelProxy {
    openai_base_url: String,
    api_key: String,
    model_profile: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl MemoryModelProxy {
    /// Start the loopback model proxy.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryModelProxyError`] when the listener cannot bind.
    pub async fn start(config: MemoryModelProxyConfig) -> Result<Self, MemoryModelProxyError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let openai_base_url = format!("http://{address}/v1");
        let api_key = config.api_key.clone();
        let model_profile = config.model_profile.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(server::run_proxy(listener, config, shutdown_rx));

        Ok(Self {
            openai_base_url,
            api_key,
            model_profile,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    /// OpenAI-compatible base URL, including `/v1`.
    #[must_use]
    pub fn openai_base_url(&self) -> &str {
        &self.openai_base_url
    }

    /// Bearer token accepted by this proxy.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Model/profile to advertise to Memory.
    #[must_use]
    pub fn model_profile(&self) -> &str {
        &self.model_profile
    }

    /// Stop the proxy accept loop after admitted connections drain.
    pub async fn shutdown(mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let _ = self.task.await;
    }
}

impl std::fmt::Debug for MemoryModelProxy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemoryModelProxy")
            .field("openai_base_url", &self.openai_base_url)
            .field("model_profile", &self.model_profile)
            .finish_non_exhaustive()
    }
}

pub(super) fn log_proxy_error(
    system_errors: Option<&SystemErrorLogger>,
    code: &'static str,
    message: &str,
) {
    if let Some(system_errors) = system_errors {
        system_errors.try_append(
            SystemErrorEvent::new(code, "Memory model proxy request failed")
                .with_error_chain([message.to_string()]),
        );
    }
}

/// Errors returned by the private model proxy.
#[derive(Debug, Error)]
pub enum MemoryModelProxyError {
    /// Network I/O failed.
    #[error("memory model proxy I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// Request or protocol validation failed.
    #[error("memory model proxy protocol error: {0}")]
    Protocol(String),
}
