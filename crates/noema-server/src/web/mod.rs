//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
mod router;
pub(super) mod session;

use noema_core::{DaemonError, WebConfig};
use tokio::net::TcpListener;

#[derive(Clone)]
pub(crate) struct WebState {
    graphql_state: noema_core::graphql::GraphqlState,
    graphql_schema: noema_core::graphql::GraphqlSchema,
    authority: authority::CanonicalAuthority,
    sessions: session::SessionSecurity,
}

impl WebState {
    #[must_use]
    pub(super) fn new(
        graphql_state: noema_core::graphql::GraphqlState,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
    ) -> Self {
        let graphql_schema = noema_core::graphql::build_schema(graphql_state.clone());
        Self {
            graphql_state,
            graphql_schema,
            authority,
            sessions,
        }
    }

    pub(crate) fn graphql_state(&self) -> &noema_core::graphql::GraphqlState {
        &self.graphql_state
    }
    pub(crate) fn graphql_schema(&self) -> &noema_core::graphql::GraphqlSchema {
        &self.graphql_schema
    }
    fn authority(&self) -> &authority::CanonicalAuthority {
        &self.authority
    }
    fn sessions(&self) -> &session::SessionSecurity {
        &self.sessions
    }
}

pub(super) use router::build_router;

pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, DaemonError> {
    let host = authority::parse_loopback_ip(&config.host).map_err(DaemonError::Protocol)?;
    TcpListener::bind((host, config.port))
        .await
        .map_err(|source| {
            DaemonError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}
