mod approvals;
mod calibrations;
mod model;
mod rows;
mod servers;
mod tools;
mod trusted_identities;

pub use model::{
    McpApprovalRequestRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    NewTrustedIdentitySelector, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};

pub(super) fn mcp_record_fragment(id: &str) -> String {
    super::ids::hex_record_fragment("mcp_", id)
}
