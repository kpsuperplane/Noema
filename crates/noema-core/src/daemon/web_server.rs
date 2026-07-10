use std::future::Future;

use crate::{DaemonError, NoemaRuntimeHost, ProviderConfig, WebConfig};
use tokio::task::JoinSet;

use super::web::{self, WebState};

#[cfg(test)]
mod test_server;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use test_server::TestDaemonWebServer;

/// Configuration required to start the daemon web server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonWebServerConfig {
    /// Provider configuration used by daemon conversations.
    pub provider: ProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

impl DaemonWebServerConfig {
    /// Create daemon web server configuration.
    #[must_use]
    pub fn new(provider: ProviderConfig, web: WebConfig) -> Self {
        Self { provider, web }
    }
}

/// Run the daemon web server until the process receives Ctrl-C.
///
/// # Errors
///
/// Returns [`DaemonError`] when the web listener cannot be bound, the runtime
/// cannot start, Ctrl-C cannot be observed, or accepting a client connection
/// fails.
pub async fn run_daemon_web(config: DaemonWebServerConfig) -> Result<(), DaemonError> {
    let web_listener = web::bind_listener(&config.web).await?;
    let host = NoemaRuntimeHost::start(config.provider)
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    let graphql_state = crate::graphql::GraphqlState::from_runtime_host(&host);
    let web_state = WebState::new(graphql_state);

    let result = serve_daemon_web(web_listener, web_state, async {
        tokio::signal::ctrl_c().await?;
        Ok(())
    })
    .await;
    host.shutdown().await;
    result
}

async fn serve_daemon_web(
    listener: tokio::net::TcpListener,
    web_state: WebState,
    shutdown: impl Future<Output = Result<(), DaemonError>> + Send,
) -> Result<(), DaemonError> {
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);

    let result = loop {
        tokio::select! {
            shutdown_result = &mut shutdown => break shutdown_result,
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(source) => break Err(source.into()),
                };
                let web_state = web_state.clone();
                connections.spawn(async move {
                    let _ = web::handle_connection(stream, web_state).await;
                });
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                let _ = completed;
            }
        }
    };

    connections.abort_all();
    while connections.join_next().await.is_some() {}
    result
}
