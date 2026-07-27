//! Object-safe MCP control-plane operations and safe consumer views.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    McpControlPlaneTool, McpDataSharingPolicy, McpServerRecord, McpToolPolicyOverride,
    McpToolPolicyRecord, McpUnsafeActionPolicy,
    oauth_model::{
        CompleteMcpOAuthSetupCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptView,
        StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
    },
    setup_model::{
        AddMcpConnectionCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
        McpServerSetupResult,
    },
};

/// Boxed future returned by object-safe MCP control-plane operations.
pub type McpOperationFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Fixed safe control-plane failure without repository, transport, or secret text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpOperationError {
    /// Command input failed domain validation.
    #[error("invalid MCP operation input")]
    InvalidInput,
    /// Requested server, tool, or setup attempt does not exist.
    #[error("the requested MCP resource was not found")]
    NotFound,
    /// Current durable state conflicts with the requested operation.
    #[error("the MCP operation conflicts with current state")]
    Conflict,
    /// Authentication must be completed before this operation can proceed.
    #[error("MCP authentication is required")]
    AuthenticationRequired,
    /// The MCP service or its durable state is temporarily unavailable.
    #[error("the MCP service is unavailable")]
    Unavailable,
    /// The remote server returned malformed or unsupported protocol data.
    #[error("the MCP server returned an unsupported response")]
    MalformedResponse,
    /// The operation was cancelled.
    #[error("the MCP operation was cancelled")]
    Cancelled,
    /// The operation exceeded its configured deadline.
    #[error("the MCP operation timed out")]
    TimedOut,
    /// The service has begun shutdown and rejects new work.
    #[error("the MCP service is shutting down")]
    ShuttingDown,
    /// The operation failed without safe implementation-specific details.
    #[error("the MCP operation failed")]
    Failed,
}

impl McpOperationError {
    /// Return the stable safe error code for API adapters.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::AuthenticationRequired => "authentication_required",
            Self::Unavailable => "unavailable",
            Self::MalformedResponse => "malformed_response",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::ShuttingDown => "shutting_down",
            Self::Failed => "failed",
        }
    }
}

/// Safe MCP control-plane operation result.
pub type McpOperationResult<T> = Result<T, McpOperationError>;

/// Server list returned to settings and other control-plane consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerList {
    /// Servers in deterministic display order.
    pub servers: Vec<McpServerRecord>,
}

/// Request the discovered tools for one server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpListToolsCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
}

/// Joined tool and behavior-policy list for one server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolList {
    /// Current server view.
    pub server: McpServerRecord,
    /// Discovered tools with their current behavior policy, in deterministic order.
    pub tools: Vec<McpControlPlaneTool>,
}

/// Save the two provider-scoped MCP policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSaveProviderPolicyCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
    /// Automatic data-sharing policy.
    pub data_sharing_policy: McpDataSharingPolicy,
    /// Approval policy for unsafe calls.
    pub unsafe_action_policy: McpUnsafeActionPolicy,
}

/// Save a complete human override for one tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSaveToolOverrideCommand {
    /// Complete override for one exact tool snapshot.
    pub policy: McpToolPolicyOverride,
}

/// Reset or retry one tool's derived policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResetToolPolicyCommand {
    /// Durable MCP tool identifier.
    pub mcp_tool_id: String,
}

/// Enable or disable one MCP tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSetToolEnabledCommand {
    /// Durable MCP tool identifier.
    pub mcp_tool_id: String,
    /// Whether the tool should be enabled without rerunning classification.
    pub enabled: bool,
}

/// Delete one server after first fencing it from new invocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDeleteServerCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
}

/// Result of a fail-closed server delete operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpDeleteServerResult {
    /// Whether an existing server was made uncallable and deleted.
    pub deleted: bool,
}

/// Object-safe MCP settings, setup, OAuth, policy, and delete operations.
pub trait McpOperations: Send + Sync {
    /// List all configured servers.
    fn list_servers(&self) -> McpOperationFuture<'_, McpOperationResult<McpServerList>>;

    /// List discovered tools and current behavior policies for one server.
    fn list_tools(
        &self,
        command: McpListToolsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolList>>;

    /// Validate, authenticate, discover, and atomically create one server.
    fn create_server(
        &self,
        command: CreateMcpServerCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>>;

    /// Authenticate, discover, and add a fresh connection to an exact definition revision.
    fn add_connection(
        &self,
        command: AddMcpConnectionCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>>;

    /// Merge replacement secrets and retry discovery for an existing server.
    fn continue_setup(
        &self,
        command: ContinueMcpServerSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>>;

    /// Start browser OAuth for a pending server setup.
    fn start_oauth_setup(
        &self,
        command: StartMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>>;

    /// Start browser OAuth reauthentication for an existing server.
    fn start_oauth_reauthentication(
        &self,
        command: StartMcpOAuthReauthenticationCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>>;

    /// Query one short-lived OAuth setup attempt.
    fn oauth_setup_attempt(
        &self,
        query: McpOAuthSetupAttemptQuery,
    ) -> McpOperationFuture<'_, McpOperationResult<Option<McpOAuthSetupAttemptView>>>;

    /// Complete OAuth from a callback listener-owned URL and continue discovery.
    fn complete_oauth_setup(
        &self,
        command: CompleteMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>>;

    /// Persist one complete provider policy pair.
    fn save_provider_policy(
        &self,
        command: McpSaveProviderPolicyCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerRecord>>;

    /// Persist one complete human tool-hint override.
    fn save_tool_override(
        &self,
        command: McpSaveToolOverrideCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>>;

    /// Reset one tool from annotations and asynchronously classify missing hints.
    fn reset_tool_policy(
        &self,
        command: McpResetToolPolicyCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>>;

    /// Enable a tool from annotations or disable it immediately.
    fn set_tool_enabled(
        &self,
        command: McpSetToolEnabledCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>>;

    /// Make a server uncallable before deleting durable and secret state.
    fn delete_server(
        &self,
        command: McpDeleteServerCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpDeleteServerResult>>;
}

/// Shared MCP control-plane operations handle.
pub type McpControlPlaneHandle = Arc<dyn McpOperations>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_errors_expose_only_fixed_safe_messages_and_codes() {
        let cases = [
            (
                McpOperationError::InvalidInput,
                "invalid_input",
                "invalid MCP operation input",
            ),
            (
                McpOperationError::NotFound,
                "not_found",
                "the requested MCP resource was not found",
            ),
            (
                McpOperationError::Conflict,
                "conflict",
                "the MCP operation conflicts with current state",
            ),
            (
                McpOperationError::AuthenticationRequired,
                "authentication_required",
                "MCP authentication is required",
            ),
            (
                McpOperationError::Unavailable,
                "unavailable",
                "the MCP service is unavailable",
            ),
            (
                McpOperationError::MalformedResponse,
                "malformed_response",
                "the MCP server returned an unsupported response",
            ),
            (
                McpOperationError::Cancelled,
                "cancelled",
                "the MCP operation was cancelled",
            ),
            (
                McpOperationError::TimedOut,
                "timed_out",
                "the MCP operation timed out",
            ),
            (
                McpOperationError::ShuttingDown,
                "shutting_down",
                "the MCP service is shutting down",
            ),
            (
                McpOperationError::Failed,
                "failed",
                "the MCP operation failed",
            ),
        ];

        for (error, code, message) in cases {
            assert_eq!(error.code(), code);
            let display = error.to_string();
            assert_eq!(display, message);
            assert!(!display.contains("secret"));
            assert!(!display.contains("backend"));
        }
    }
}
