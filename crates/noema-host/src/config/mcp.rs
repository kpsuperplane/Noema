use serde::{Deserialize, Serialize};

/// Startup configuration for MCP transports with host authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    /// Permit MCP definitions to start local stdio processes.
    pub stdio_enabled: bool,
}
