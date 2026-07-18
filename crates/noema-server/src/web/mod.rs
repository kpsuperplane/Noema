//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
mod router;
pub(super) mod session;

use noema_host::WebConfig;
use tokio::net::TcpListener;

use crate::WebServerError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebAuthMode {
    #[cfg_attr(all(feature = "dev-no-auth", debug_assertions), allow(dead_code))]
    Required,
    #[cfg(any(test, all(feature = "dev-no-auth", debug_assertions)))]
    DisabledForDevelopment,
}

impl WebAuthMode {
    #[cfg(all(feature = "dev-no-auth", debug_assertions))]
    pub(crate) const fn from_build() -> Self {
        Self::DisabledForDevelopment
    }

    #[cfg(not(all(feature = "dev-no-auth", debug_assertions)))]
    pub(crate) const fn from_build() -> Self {
        Self::Required
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
}

impl WebState {
    #[must_use]
    pub(super) fn new(
        graphql_state: noema_api::graphql::GraphqlState,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
        auth_mode: WebAuthMode,
    ) -> Self {
        let graphql_schema = noema_api::graphql::build_schema(graphql_state.clone());
        Self {
            graphql_state,
            graphql_schema,
            authority,
            sessions,
            auth_mode,
        }
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
