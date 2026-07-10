use crate::{DaemonError, NoemaRuntimeHost, ProviderConfig, WebConfig};

use super::web::{self, WebState};

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
    let listener_address = web_listener.local_addr()?;
    let authority = web::authority::CanonicalAuthority::from_socket_addr(listener_address);
    let sessions = web::session::SessionSecurity::generate().map_err(|_| {
        DaemonError::Protocol("failed to generate the browser bootstrap capability".to_string())
    })?;
    let host = NoemaRuntimeHost::start(config.provider)
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    let graphql_state = crate::graphql::GraphqlState::from_runtime_host(&host);
    let web_state = WebState::new(graphql_state, authority.clone(), sessions.clone());
    let bootstrap_url = sessions.bootstrap_url(authority.as_str()).ok_or_else(|| {
        DaemonError::Protocol("failed to read the browser bootstrap capability".to_string())
    })?;
    println!("Noema browser bootstrap: {bootstrap_url}");
    let shutdown_error = std::sync::Arc::new(std::sync::Mutex::new(None));
    let signal_error = shutdown_error.clone();
    let server_result = axum::serve(web_listener, web::build_router(web_state))
        .with_graceful_shutdown(async move {
            if let Err(error) = tokio::signal::ctrl_c().await
                && let Ok(mut shutdown_error) = signal_error.lock()
            {
                *shutdown_error = Some(error);
            }
        })
        .await;
    host.shutdown().await;
    let signal_result = shutdown_error
        .lock()
        .map_err(|_| DaemonError::Protocol("Ctrl-C error state was poisoned".to_string()))?
        .take();
    if let Some(error) = signal_result {
        return Err(error.into());
    }
    server_result.map_err(DaemonError::from)
}
