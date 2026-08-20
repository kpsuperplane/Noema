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
    let auth_mode = web::WebAuthMode::from_config(host.web_config());
    let recovery = if auth_mode.requires_session() {
        Some(
            noema_host::RecoveryCodeStore::open(host.services().noema_paths.config_path())
                .map_err(|error| {
                    WebServerError::Protocol(format!("recovery setup failed: {error}"))
                })?,
        )
    } else {
        None
    };
    let web_listener = web::bind_listener(host.web_config()).await?;
    let listener_address = web_listener.local_addr()?;
    let authority = web::authority::CanonicalAuthority::from_web_config(
        host.web_config(),
        listener_address.port(),
    )
    .map_err(WebServerError::Protocol)?;
    let sessions = web::session::SessionSecurity::generate().map_err(|_| {
        WebServerError::Protocol("failed to generate browser session security".to_string())
    })?;
    let mut graphql_state = noema_api::graphql::GraphqlState::from_host_services(host.services())
        .with_mcp_oauth_callback_url(format!("{}/mcp/oauth/callback", authority.origin()))
        .with_provider_oauth_callback_url(format!("{}/provider/oauth/callback", authority.origin()))
        .with_adapter_oauth_callback_url(format!("{}/adapter/oauth/callback", authority.origin()));
    let notifications = if authority.secure() {
        Some(
            noema_api::graphql::NotificationCoordinator::new_with_paths(
                host.services().store.clone(),
                authority.origin().to_string(),
                host.services().noema_paths.clone(),
            )
            .await
            .map_err(WebServerError::Protocol)?,
        )
    } else {
        None
    };
    if let Some(notifications) = &notifications {
        graphql_state = graphql_state.with_notifications(notifications.clone());
    }
    let notification_graphql_state = graphql_state.clone();
    let web_state = WebState::new(
        graphql_state,
        host.services().store.clone(),
        authority.clone(),
        sessions.clone(),
        auth_mode,
        host.web_config().graphiql,
        web::WebFiles::new(recovery, host.services().noema_paths.clone()),
    )
    .map_err(WebServerError::Protocol)?;
    let local_graphql = if host.web_config().local_graphql_socket {
        Some(
            web::LocalGraphqlServer::bind(
                host.services().noema_paths.graphql_socket_path(),
                web_state.local_graphql_schema(),
            )
            .await?,
        )
    } else {
        None
    };
    let web_push_task = notifications.map(|notifications| {
        let receiver = host.services().runtime_events.subscribe_all_conversations();
        let task_receiver = host.services().runtime_events.subscribe_all_tasks();
        let work_receiver = host
            .services()
            .runtime_events
            .subscribe_work("workspace:personal");
        tokio::spawn(notifications.run(
            notification_graphql_state,
            receiver,
            task_receiver,
            work_receiver,
        ))
    });
    if !auth_mode.requires_session() {
        println!("Noema browser authentication disabled (development only)");
    } else {
        println!(
            "Noema browser passkey authentication enabled at {}",
            authority.origin()
        );
    }
    let shutdown_error = std::sync::Arc::new(std::sync::Mutex::new(None));
    let signal_error = shutdown_error.clone();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
    let local_graphql_task = local_graphql
        .map(|server| tokio::spawn(async move { server.serve(shutdown_receiver).await }));
    let browser_session_revocations = sessions.subscribe_revocations();
    let session_cleanup_task = tokio::spawn(sessions.run_expiry_cleanup());
    let browser_push_cleanup_task = tokio::spawn(run_browser_push_cleanup(
        host.services().store.clone(),
        browser_session_revocations,
    ));
    let oauth_cleanup_task = tokio::spawn(run_native_oauth_expiry_cleanup(
        host.services().store.clone(),
    ));
    let signal_shutdown = shutdown_sender.clone();
    let server_result = axum::serve(web_listener, web::build_router(web_state))
        .with_graceful_shutdown(async move {
            if let Err(error) = tokio::signal::ctrl_c().await
                && let Ok(mut shutdown_error) = signal_error.lock()
            {
                *shutdown_error = Some(error);
            }
            let _ = signal_shutdown.send(true);
        })
        .await;
    let _ = shutdown_sender.send(true);
    let local_graphql_result = match local_graphql_task {
        Some(task) => match task.await {
            Ok(result) => result.map_err(WebServerError::Io),
            Err(error) => Err(WebServerError::Protocol(format!(
                "local GraphQL server stopped unexpectedly: {error}"
            ))),
        },
        None => Ok(()),
    };
    if let Some(task) = web_push_task {
        task.abort();
    }
    session_cleanup_task.abort();
    browser_push_cleanup_task.abort();
    oauth_cleanup_task.abort();
    let signal_result = shutdown_error
        .lock()
        .map_err(|_| WebServerError::Protocol("Ctrl-C error state was poisoned".to_string()))?
        .take();
    if let Some(error) = signal_result {
        return Err(error.into());
    }
    server_result.map_err(WebServerError::from)?;
    local_graphql_result
}

async fn run_browser_push_cleanup(
    store: noema_store::NoemaStore,
    mut revocations: tokio::sync::broadcast::Receiver<tower_sessions::session::Id>,
) {
    loop {
        match revocations.recv().await {
            Ok(session_id) => {
                let _ = store
                    .remove_web_push_for_session(web::session::session_id_hash(session_id))
                    .await;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let _ = store.remove_all_web_push_subscriptions().await;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn run_native_oauth_expiry_cleanup(store: noema_store::NoemaStore) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = store
            .expire_native_oauth_families(
                std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .unwrap_or_default()
                    .as_secs()
                    .try_into()
                    .unwrap_or(i64::MAX),
            )
            .await
        {
            eprintln!("Native OAuth expiry cleanup failed: {error}");
        }
    }
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
