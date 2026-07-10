use serde::{Deserialize, Serialize};

/// Default host for the local web UI.
pub const DEFAULT_WEB_HOST: &str = "127.0.0.1";
/// Default port for the local web UI.
pub const DEFAULT_WEB_PORT: u16 = 3737;

/// Configuration for the local web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebConfig {
    /// Host/interface to bind.
    pub host: String,
    /// TCP port to bind.
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_WEB_HOST.to_string(),
            port: DEFAULT_WEB_PORT,
        }
    }
}
