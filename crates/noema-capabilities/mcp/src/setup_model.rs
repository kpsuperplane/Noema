//! Transport-neutral MCP setup commands and result models.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{McpServerRecord, McpTransportKind};

use crate::secret_model::McpSecretMaterial;

/// Safe stdio connection configuration supplied during setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpStdioSetupConfig {
    /// Executable name or path.
    pub command: String,
    /// Executable arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional child working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Non-secret environment entries safe to persist in structured storage.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// Safe Streamable HTTP connection configuration supplied during setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpStreamableHttpSetupConfig {
    /// MCP endpoint URL.
    pub url: String,
    /// Non-secret headers safe to persist in structured storage.
    #[serde(default)]
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
            Self::Stdio(config) => serde_json::to_value(config),
            Self::StreamableHttp(config) => serde_json::to_value(config),
        }
        .expect("MCP setup configuration is serializable")
    }
}

/// Add and verify one MCP server without persisting it before discovery succeeds.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateMcpServerCommand {
    /// Human-visible server name.
    pub display_name: String,
    /// Typed safe connection configuration.
    pub transport: McpSetupTransportConfig,
    /// Secret connection material kept outside structured storage.
    pub secrets: McpSecretMaterial,
    /// Whether advertised browser authentication should pause before persistence.
    pub auth_preference: McpSetupAuthPreference,
}

/// Add a fresh connection to one explicitly selected MCP definition revision.
#[derive(Debug, Clone, PartialEq)]
pub struct AddMcpConnectionCommand {
    /// Stable definition selected by the human.
    pub mcp_definition_id: String,
    /// Exact immutable definition revision shown during setup.
    pub expected_definition_revision: String,
    /// Optional account or installation label.
    pub connection_label: Option<String>,
    /// Fresh connection credentials; sibling credentials are never copied.
    pub secrets: McpSecretMaterial,
    /// Whether advertised browser authentication should pause before persistence.
    pub auth_preference: McpSetupAuthPreference,
}

/// Authentication behavior for an otherwise successful anonymous setup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpSetupAuthPreference {
    /// Offer browser authentication when protected-resource metadata is advertised.
    #[default]
    PromptIfAvailable,
    /// Persist the tools visible without browser authentication.
    UseAnonymous,
}

/// Update secret material for an existing server and retry discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueMcpServerSetupCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
    /// Secret entries to merge into the server's current secret material.
    pub secrets: McpSecretMaterial,
}

/// High-level guided setup status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStatus {
    /// Setup needs authentication before discovery can finish.
    NeedsAuth,
    /// Anonymous discovery succeeded and browser authentication is available.
    AuthenticationAvailable,
    /// Discovery succeeded and provider policy can be configured.
    ReadyForPolicy,
    /// The configured server or transport is unavailable.
    Unavailable,
    /// The server returned malformed or unsupported metadata.
    Malformed,
}

impl McpSetupStatus {
    /// Return the stable client-facing status value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAuth => "needs_auth",
            Self::AuthenticationAvailable => "authentication_available",
            Self::ReadyForPolicy => "ready_for_policy",
            Self::Unavailable => "unavailable",
            Self::Malformed => "malformed",
        }
    }

    /// Return the fixed safe issue for a failed setup state.
    #[must_use]
    pub const fn issue(self) -> Option<McpSetupIssue> {
        match self {
            Self::NeedsAuth => Some(McpSetupIssue::AuthenticationRequired),
            Self::Unavailable => Some(McpSetupIssue::Unavailable),
            Self::Malformed => Some(McpSetupIssue::Malformed),
            Self::AuthenticationAvailable | Self::ReadyForPolicy => None,
        }
    }
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
    /// Number of tools discovered, whether or not setup has been committed yet.
    pub discovered_tool_count: usize,
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
            auth_preference: McpSetupAuthPreference::PromptIfAvailable,
        };

        let debug = format!("{command:?}");

        assert!(!debug.contains("setup-secret"));
        assert!(debug.contains("Docs"));
    }
}
