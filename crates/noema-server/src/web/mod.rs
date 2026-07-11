//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
mod router;
pub(super) mod session;

use noema_core::{DaemonError, WebConfig};
use tokio::net::TcpListener;

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
    graphql_state: noema_core::graphql::GraphqlState,
    graphql_schema: noema_core::graphql::GraphqlSchema,
    authority: authority::CanonicalAuthority,
    sessions: session::SessionSecurity,
    auth_mode: WebAuthMode,
}

impl WebState {
    #[must_use]
    pub(super) fn new(
        graphql_state: noema_core::graphql::GraphqlState,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
        auth_mode: WebAuthMode,
    ) -> Self {
        let graphql_schema = noema_core::graphql::build_schema(graphql_state.clone());
        Self {
            graphql_state,
            graphql_schema,
            authority,
            sessions,
            auth_mode,
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
    fn auth_mode(&self) -> WebAuthMode {
        self.auth_mode
    }
}

pub(super) use router::build_router;

pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, DaemonError> {
    let host = authority::parse_bind_ip(&config.host).map_err(DaemonError::Protocol)?;
    TcpListener::bind((host, config.port))
        .await
        .map_err(|source| {
            DaemonError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_auth_mode_is_only_disabled_for_debug_dev_feature() {
        #[cfg(all(feature = "dev-no-auth", debug_assertions))]
        assert_eq!(
            WebAuthMode::from_build(),
            WebAuthMode::DisabledForDevelopment
        );

        #[cfg(not(all(feature = "dev-no-auth", debug_assertions)))]
        assert_eq!(WebAuthMode::from_build(), WebAuthMode::Required);
    }
}
