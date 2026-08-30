//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
mod clients;
mod favicons;
mod local_graphql;
mod native_oauth;
mod passkey;
mod router;
pub(super) mod session;
mod session_store;

use noema_host::WebConfig;
use tokio::{net::TcpListener, sync::Semaphore};

use crate::WebServerError;

pub(super) const MAX_GRAPHQL_BODY_BYTES: usize = 64 * 1024;
const MAX_WEBSOCKET_CONNECTIONS: usize = 64;
pub(super) use local_graphql::LocalGraphqlServer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebAuthMode {
    Required,
    DisabledForDevelopment,
}

pub(super) struct WebFiles {
    recovery: Option<noema_host::RecoveryCodeStore>,
    noema_paths: noema_home::NoemaPaths,
}

impl WebFiles {
    pub(super) const fn new(
        recovery: Option<noema_host::RecoveryCodeStore>,
        noema_paths: noema_home::NoemaPaths,
    ) -> Self {
        Self {
            recovery,
            noema_paths,
        }
    }
}

impl WebAuthMode {
    pub(crate) const fn from_config(config: &WebConfig) -> Self {
        if config.dev_no_auth {
            Self::DisabledForDevelopment
        } else {
            Self::Required
        }
    }

    pub(crate) const fn requires_session(self) -> bool {
        matches!(self, Self::Required)
    }
}

#[derive(Clone)]
pub(crate) struct WebState {
    graphql_state: noema_api::graphql::GraphqlState,
    graphql_schema: noema_api::graphql::GraphqlSchema,
    authority: authority::CanonicalAuthority,
    sessions: session::SessionSecurity,
    auth_mode: WebAuthMode,
    store: noema_store::NoemaStore,
    passkeys: passkey::PasskeySecurity,
    native_oauth_retries: native_oauth::NativeOAuthRetryStore,
    graphiql_enabled: bool,
    recovery: Option<noema_host::RecoveryCodeStore>,
    favicons: favicons::FaviconService,
    websocket_slots: std::sync::Arc<Semaphore>,
}

impl WebState {
    pub(super) fn new(
        graphql_state: noema_api::graphql::GraphqlState,
        store: noema_store::NoemaStore,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
        auth_mode: WebAuthMode,
        graphiql_enabled: bool,
        files: WebFiles,
    ) -> Result<Self, String> {
        let graphql_schema = noema_api::graphql::build_schema(graphql_state.clone());
        let passkeys = passkey::PasskeySecurity::new(&authority)?;
        let native_oauth_retries =
            native_oauth::NativeOAuthRetryStore::new(files.noema_paths.native_oauth_retry_path());
        Ok(Self {
            graphql_state,
            graphql_schema,
            authority,
            sessions,
            auth_mode,
            store,
            passkeys,
            native_oauth_retries,
            graphiql_enabled,
            recovery: files.recovery,
            favicons: favicons::FaviconService::new(files.noema_paths.favicon_cache_dir()),
            websocket_slots: std::sync::Arc::new(Semaphore::new(MAX_WEBSOCKET_CONNECTIONS)),
        })
    }

    fn websocket_slot(&self) -> Result<tokio::sync::OwnedSemaphorePermit, axum::http::StatusCode> {
        self.websocket_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }

    pub(super) fn local_graphql_schema(&self) -> noema_api::graphql::GraphqlSchema {
        self.graphql_schema.clone()
    }
}

pub(super) use router::build_router;

pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, WebServerError> {
    let host = authority::parse_bind_ip(&config.host).map_err(WebServerError::Protocol)?;
    TcpListener::bind((host, config.port))
        .await
        .map_err(|source| {
            WebServerError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn websocket_capacity_rejects_excess_work() {
        let root = tempfile::tempdir().expect("store root");
        let store = noema_store::NoemaStore::open(&noema_store::StoreConfig::new(
            root.path().join("noema.sqlite3"),
        ))
        .await
        .expect("store");
        let state = WebState::new(
            noema_api::graphql::GraphqlState::for_tests(),
            store.clone(),
            authority::CanonicalAuthority::from_public_origin("http://localhost:3737", "localhost")
                .expect("authority"),
            session::SessionSecurity::for_tests(store),
            WebAuthMode::DisabledForDevelopment,
            false,
            WebFiles::new(
                None,
                noema_home::NoemaPaths::from_noema_home(root.path()).expect("paths"),
            ),
        )
        .expect("web state");
        let permits = (0..MAX_WEBSOCKET_CONNECTIONS)
            .map(|_| state.websocket_slot().expect("WebSocket slot"))
            .collect::<Vec<_>>();

        assert!(state.websocket_slot().is_err());
        drop(permits);
        assert!(state.websocket_slot().is_ok());
    }
}
