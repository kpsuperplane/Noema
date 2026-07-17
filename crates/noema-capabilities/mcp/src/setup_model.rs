//! Transport-neutral MCP setup commands and result models.

use std::{collections::BTreeMap, fmt};

use serde_json::Value;
use thiserror::Error;

use crate::{McpServerRecord, McpTransportKind};

use crate::secret_model::McpSecretMaterial;

/// Safe stdio connection configuration supplied during setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpStdioSetupConfig {
    /// Executable name or path.
    pub command: String,
    /// Executable arguments.
    pub args: Vec<String>,
    /// Optional child working directory.
    pub cwd: Option<String>,
    /// Non-secret environment entries safe to persist in structured storage.
    pub env: BTreeMap<String, String>,
}

/// Safe Streamable HTTP connection configuration supplied during setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpStreamableHttpSetupConfig {
    /// MCP endpoint URL.
    pub url: String,
    /// Non-secret headers safe to persist in structured storage.
    pub headers: BTreeMap<String, String>,
}

/// Exactly one supported MCP transport configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpSetupTransportConfig {
    /// Local stdio process configuration.
    Stdio(McpStdioSetupConfig),
    /// Remote Streamable HTTP configuration.
    StreamableHttp(McpStreamableHttpSetupConfig),
}

impl McpSetupTransportConfig {
    /// Return the transport kind represented by this configuration.
    #[must_use]
    pub const fn transport_kind(&self) -> McpTransportKind {
        match self {
            Self::Stdio(_) => McpTransportKind::Stdio,
            Self::StreamableHttp(_) => McpTransportKind::StreamableHttp,
        }
    }

    /// Convert the typed setup configuration to its non-secret repository view.
    #[must_use]
    pub fn safe_config(&self) -> Value {
        match self {
            Self::Stdio(config) => serde_json::json!({
                "command": config.command,
                "args": config.args,
                "cwd": config.cwd,
                "env": config.env,
            }),
            Self::StreamableHttp(config) => serde_json::json!({
                "url": config.url,
                "headers": config.headers,
            }),
        }
    }
}

/// Add and verify one MCP server without persisting it before discovery succeeds.
#[derive(Clone, PartialEq)]
pub struct CreateMcpServerCommand {
    /// Human-visible server name.
    pub display_name: String,
    /// Typed safe connection configuration.
    pub transport: McpSetupTransportConfig,
    /// Secret connection material kept outside structured storage.
    pub secrets: McpSecretMaterial,
}

impl fmt::Debug for CreateMcpServerCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateMcpServerCommand")
            .field("display_name", &self.display_name)
            .field("transport", &self.transport)
            .field("secrets", &self.secrets)
            .finish()
    }
}

/// Update secret material for an existing server and retry discovery.
#[derive(Clone, PartialEq)]
pub struct ContinueMcpServerSetupCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
    /// Secret entries to merge into the server's current secret material.
    pub secrets: McpSecretMaterial,
}

impl fmt::Debug for ContinueMcpServerSetupCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContinueMcpServerSetupCommand")
            .field("mcp_server_id", &self.mcp_server_id)
            .field("secrets", &self.secrets)
            .finish()
    }
}

/// High-level guided setup status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStatus {
    /// Setup needs authentication before discovery can finish.
    NeedsAuth,
    /// Discovery succeeded and calibration can begin.
    ReadyForCalibration,
    /// The configured server or transport is unavailable.
    Unavailable,
    /// The server returned malformed or unsupported metadata.
    Malformed,
}

/// Metadata discovery status exposed to control-plane consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpDiscoveryStatus {
    /// Discovery could not begin until authentication is supplied.
    NeedsAuth,
    /// Discovery completed successfully.
    Discovered,
    /// Discovery could not reach the server.
    Unavailable,
    /// Discovery returned malformed or unsupported metadata.
    Malformed,
}

/// Authentication options safe to present during guided setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSetupAuthDetails {
    /// Whether OAuth client-secret credentials can be attempted.
    pub oauth_client_credentials_supported: bool,
    /// Whether browser OAuth authorization can be attempted.
    pub oauth_authorization_supported: bool,
    /// Suggested OAuth scopes, when known.
    pub scopes: Vec<String>,
}

/// Fixed setup issue safe for UI and model-visible surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpSetupIssue {
    /// The server requires authentication before discovery.
    #[error("this MCP server requires authentication before Noema can list tools")]
    AuthenticationRequired,
    /// The server could not be reached with the supplied configuration.
    #[error("Noema could not connect to this MCP server")]
    Unavailable,
    /// The server returned unsupported metadata.
    #[error("the MCP server returned unsupported tool metadata")]
    Malformed,
}

/// Result of guided MCP setup or reauthentication.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerSetupResult {
    /// Persisted server view, when setup reached a durable state.
    pub server: Option<McpServerRecord>,
    /// High-level setup status.
    pub setup_status: McpSetupStatus,
    /// Discovery status, when discovery was attempted.
    pub discovery_status: Option<McpDiscoveryStatus>,
    /// Number of tools discovered and committed atomically.
    pub discovered_tool_count: usize,
    /// Fixed safe issue text, when setup did not complete.
    pub issue: Option<McpSetupIssue>,
    /// Authentication options detected for this setup.
    pub auth: Option<McpSetupAuthDetails>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_command_debug_uses_nested_secret_redaction() {
        let command = CreateMcpServerCommand {
            display_name: "Docs".to_string(),
            transport: McpSetupTransportConfig::Stdio(McpStdioSetupConfig {
                command: "docs-server".to_string(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
            }),
            secrets: McpSecretMaterial {
                env: BTreeMap::from([("TOKEN".to_string(), "setup-secret".to_string())]),
                ..McpSecretMaterial::default()
            },
        };

        let debug = format!("{command:?}");

        assert!(!debug.contains("setup-secret"));
        assert!(debug.contains("Docs"));
    }
}
