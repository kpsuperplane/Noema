//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
mod clients;
mod passkey;
mod router;
pub(super) mod session;

use noema_host::WebConfig;
use tokio::net::TcpListener;

use crate::WebServerError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebAuthMode {
    Required,
    DisabledForDevelopment,
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
    client_auth: clients::ClientAuth,
    graphiql_enabled: bool,
    recovery: Option<noema_host::RecoveryCodeStore>,
}

impl WebState {
    pub(super) fn new(
        graphql_state: noema_api::graphql::GraphqlState,
        store: noema_store::NoemaStore,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
        auth_mode: WebAuthMode,
        graphiql_enabled: bool,
        recovery: Option<noema_host::RecoveryCodeStore>,
    ) -> Result<Self, String> {
        let graphql_schema = noema_api::graphql::build_schema(graphql_state.clone());
        let passkeys = passkey::PasskeySecurity::new(&authority)?;
        let client_auth = clients::ClientAuth::new();
        Ok(Self {
            graphql_state,
            graphql_schema,
            authority,
            sessions,
            auth_mode,
            store,
            passkeys,
            client_auth,
            graphiql_enabled,
            recovery,
        })
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
