use noema_core::{DaemonError, NoemaRuntimeHost, WebConfig};
use noema_providers::ProviderConfig;
use std::path::PathBuf;

use super::web::{self, WebState};

/// Run the daemon web server until the process receives Ctrl-C.
///
/// # Errors
///
/// Returns [`DaemonError`] when the web listener cannot be bound, the runtime
/// cannot start, Ctrl-C cannot be observed, or accepting a client connection
/// fails.
pub async fn run_daemon_web(
    provider: ProviderConfig,
    web_config: WebConfig,
    local_model_runtime_root: Option<PathBuf>,
) -> Result<(), DaemonError> {
    let web_listener = web::bind_listener(&web_config).await?;
    let listener_address = web_listener.local_addr()?;
    let authority = web::authority::CanonicalAuthority::from_socket_addr(listener_address);
    let sessions = web::session::SessionSecurity::generate().map_err(|_| {
        DaemonError::Protocol("failed to generate the browser bootstrap capability".to_string())
    })?;
    let auth_mode = web::WebAuthMode::from_build();
    let host =
        NoemaRuntimeHost::start_with_local_model_runtime_root(provider, local_model_runtime_root)
            .await
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    let graphql_state = noema_core::graphql::GraphqlState::from_runtime_host(&host);
    let web_state = WebState::new(
        graphql_state,
        authority.clone(),
        sessions.clone(),
        auth_mode,
    );
    if auth_mode.requires_session() {
        let bootstrap_url = sessions.bootstrap_url(authority.as_str()).ok_or_else(|| {
            DaemonError::Protocol("failed to read the browser bootstrap capability".to_string())
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
