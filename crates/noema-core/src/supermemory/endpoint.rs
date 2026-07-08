//! Runtime-only Supermemory endpoint configuration.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

/// Runtime-only connection details for a Supermemory service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemoryConnection {
    /// Service base URL used only inside the runtime.
    pub base_url: String,
    /// Optional API key for the service.
    pub api_key: Option<String>,
}

impl SupermemoryConnection {
    /// Build a connection and normalize the base URL.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }
}

/// Allocate an ephemeral loopback TCP port for a managed Supermemory sidecar.
///
/// The listener is released before the child process starts, so callers should
/// treat the result as a candidate and retry startup if the port is stolen.
///
/// # Errors
///
/// Returns the OS bind error if a loopback port cannot be allocated.
pub fn allocate_loopback_port() -> Result<u16, std::io::Error> {
    for _ in 0..16 {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        if port != 6767 {
            return Ok(port);
        }
    }
    Ok(6767)
}

#[cfg(test)]
mod tests {
    #[test]
    fn allocated_loopback_port_is_nonzero_and_not_default() {
        let port = super::allocate_loopback_port().expect("port");

        assert_ne!(port, 0);
        assert_ne!(port, 6767);
    }

    #[test]
    fn connection_trims_base_url() {
        let connection = super::SupermemoryConnection::new(
            "http://127.0.0.1:12345/".to_string(),
            Some("secret".to_string()),
        );

        assert_eq!(connection.base_url, "http://127.0.0.1:12345");
        assert_eq!(connection.api_key.as_deref(), Some("secret"));
    }
}
