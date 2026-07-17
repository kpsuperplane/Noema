#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use noema_capabilities_mcp::McpTransportKind;

pub use noema_capabilities_mcp::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord, NewToolCalibration,
    ToolCalibrationRecord,
};

/// Input for creating an MCP server metadata row.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NewMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
}

/// Input captured from MCP tool discovery.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NewMcpTool {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Value,
    /// Optional MCP output schema.
    pub output_schema: Option<Value>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Value,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
}
