//! Runtime-only Mnemosyne endpoint configuration.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

/// Runtime-only connection details for a Mnemosyne sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MnemosyneConnection {
    /// Service base URL used only inside the runtime.
    pub base_url: String,
    /// Optional API key for the service.
    pub api_key: Option<String>,
}

impl MnemosyneConnection {
    /// Build a connection and normalize the base URL.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }
}

pub(super) fn allocate_loopback_port() -> Result<u16, std::io::Error> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}
