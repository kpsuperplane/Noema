use serde::{Deserialize, Serialize};

/// Default host for the local web UI.
pub(super) const DEFAULT_WEB_HOST: &str = "127.0.0.1";
/// Default port for the local web UI.
pub(super) const DEFAULT_WEB_PORT: u16 = 3737;

/// Configuration for the local web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    /// Host/interface to bind.
    pub host: String,
    /// TCP port to bind.
    pub port: u16,
    /// Stable WebAuthn relying-party identifier used to scope passkeys.
    pub rp_id: String,
    /// Browser-visible origin when Noema is served through an HTTPS reverse proxy.
    pub public_origin: Option<String>,
    /// Authenticate accepted requests as the built-in local human.
    pub dev_no_auth: bool,
    /// Expose the local-human GraphQL endpoint on a private Unix socket.
    pub local_graphql_socket: bool,
    /// Expose the authenticated GraphiQL development interface.
    pub graphiql: bool,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_WEB_HOST.to_string(),
            port: DEFAULT_WEB_PORT,
            rp_id: "localhost".to_string(),
            public_origin: None,
            dev_no_auth: false,
            local_graphql_socket: false,
            graphiql: false,
        }
    }
}
