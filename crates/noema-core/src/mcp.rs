//! Third-party MCP control-plane domain types.

/// Advisory MCP tool calibration autofill prompt and parser support.
pub mod autofill;
pub mod client;
mod eligibility;
/// HTTP MCP metadata transports.
pub mod http;
/// Hosted MCP OAuth setup attempts.
pub mod oauth;
/// Disk-backed MCP secret storage.
pub mod secrets;
/// Guided MCP server setup orchestration.
pub mod setup;
/// Stdio MCP metadata transport.
pub mod stdio;

pub use client::{DiscoveredMcpTool, McpClientError, McpClientRuntime, McpTransport};
pub use eligibility::{
    McpToolIneligibility, mcp_tool_ineligibility, prompt_safe_mcp_tool_description,
};
pub(crate) use eligibility::{
    mcp_tool_catalog_ineligibility, sanitize_prompt_line, truncate_chars,
};
pub use http::StreamableHttpMcpTransport;
pub use oauth::{
    McpOAuthSetupAttemptStatus, McpOAuthSetupAttemptView, McpOAuthSetupManager,
    StartMcpOAuthSetupRequest,
};
pub use stdio::StdioMcpTransport;

use serde::{Deserialize, Serialize};

/// Effective trust classification for an MCP tool policy axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTrustClassification {
    /// The tool does not exercise this policy axis.
    None,
    /// The tool operates only on trusted-owner data for this axis.
    Trusted,
    /// The tool operates on untrusted-owner data for this axis.
    Untrusted,
    /// Trust depends on resolved ownership for this axis.
    Mixed,
}

impl McpTrustClassification {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::Mixed => "mixed",
        }
    }
}

/// Transport used to connect to an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    /// Local stdio MCP transport.
    Stdio,
    /// Remote Streamable HTTP MCP transport.
    StreamableHttp,
}

impl McpTransportKind {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable_http",
        }
    }
}

/// Review status for a calibrated MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpCalibrationStatus {
    /// The tool metadata exists but needs human or admin review.
    NeedsReview,
    /// The tool cannot be enabled because owner resolution is incomplete.
    BlockedUnresolvedOwnership,
    /// The tool is reviewed and ready for gateway use.
    Ready,
    /// The tool is intentionally disabled.
    Disabled,
}

impl McpCalibrationStatus {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsReview => "needs_review",
            Self::BlockedUnresolvedOwnership => "blocked_unresolved_ownership",
            Self::Ready => "ready",
            Self::Disabled => "disabled",
        }
    }
}
