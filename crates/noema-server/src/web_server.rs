use noema_host::NoemaHost;
use thiserror::Error;

use super::web::{self, WebState};

/// Run the daemon web server until the process receives Ctrl-C.
///
/// # Errors
///
/// Returns [`WebServerError`] when the web listener cannot be bound, the runtime
/// cannot start, Ctrl-C cannot be observed, or accepting a client connection
/// fails.
pub async fn run_daemon_web(host: NoemaHost) -> Result<(), WebServerError> {
    let result = serve_daemon_web(&host).await;
    host.shutdown().await;
    result
}

async fn serve_daemon_web(host: &NoemaHost) -> Result<(), WebServerError> {
    let web_listener = web::bind_listener(host.web_config()).await?;
    let listener_address = web_listener.local_addr()?;
    let authority = web::authority::CanonicalAuthority::from_socket_addr(listener_address);
    let sessions = web::session::SessionSecurity::generate().map_err(|_| {
        WebServerError::Protocol("failed to generate the browser bootstrap capability".to_string())
    })?;
    let auth_mode = web::WebAuthMode::from_build();
    let graphql_state = noema_api::graphql::GraphqlState::from_host_services(host.services());
    let web_state = WebState::new(
        graphql_state,
        authority.clone(),
        sessions.clone(),
        auth_mode,
    );
    if auth_mode.requires_session() {
        let bootstrap_url = sessions.bootstrap_url(authority.as_str()).ok_or_else(|| {
            WebServerError::Protocol("failed to read the browser bootstrap capability".to_string())
        })?;
        println!("Noema browser bootstrap: {bootstrap_url}");
    } else {
        println!("Noema browser authentication disabled (development only)");
    }
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
    let signal_result = shutdown_error
        .lock()
        .map_err(|_| WebServerError::Protocol("Ctrl-C error state was poisoned".to_string()))?
        .take();
    if let Some(error) = signal_result {
        return Err(error.into());
    }
    server_result.map_err(WebServerError::from)
}

/// Failure to start or serve the local web application.
#[derive(Debug, Error)]
pub enum WebServerError {
    /// A server configuration or lifecycle invariant failed.
    #[error("{0}")]
    Protocol(String),
    /// Listener, signal, or connection I/O failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
