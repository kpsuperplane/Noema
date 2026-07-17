//! Object-safe MCP control-plane operations and safe consumer views.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    McpControlPlaneTool, McpServerRecord, McpToolCalibrationSuggestion, NewToolCalibration,
    ToolCalibrationRecord,
    oauth_model::{
        CompleteMcpOAuthSetupCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptView,
        StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
    },
    setup_model::{ContinueMcpServerSetupCommand, CreateMcpServerCommand, McpServerSetupResult},
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

/// Joined tool and calibration list for one server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolList {
    /// Current server view.
    pub server: McpServerRecord,
    /// Discovered tools with their current calibration, in deterministic order.
    pub tools: Vec<McpControlPlaneTool>,
}

/// Request metadata-only calibration suggestions for one server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAutofillCalibrationsCommand {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
}

/// Validated draft suggestions returned without persisting calibration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpAutofillCalibrationsResult {
    /// Suggestions keyed by durable MCP tool identifier.
    pub suggestions: Vec<McpToolCalibrationSuggestion>,
}

/// Save one exact calibration batch transactionally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSaveCalibrationsCommand {
    /// Complete batch to validate before any calibration is changed.
    pub calibrations: Vec<NewToolCalibration>,
}

/// Calibrations committed by one transactional save operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSaveCalibrationsResult {
    /// Saved calibrations in command order.
    pub calibrations: Vec<ToolCalibrationRecord>,
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

/// Object-safe MCP settings, setup, OAuth, calibration, and delete operations.
pub trait McpOperations: Send + Sync {
    /// List all configured servers.
    fn list_servers(&self) -> McpOperationFuture<'_, McpOperationResult<McpServerList>>;

    /// List discovered tools and current calibrations for one server.
    fn list_tools(
        &self,
        command: McpListToolsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolList>>;

    /// Validate, authenticate, discover, and atomically create one server.
    fn create_server(
        &self,
        command: CreateMcpServerCommand,
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

    /// Generate validated, unpersisted calibration suggestions.
    fn autofill_calibrations(
        &self,
        command: McpAutofillCalibrationsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpAutofillCalibrationsResult>>;

    /// Validate and save a complete calibration batch transactionally.
    fn save_calibrations(
        &self,
        command: McpSaveCalibrationsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpSaveCalibrationsResult>>;

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
    fn operations_trait_remains_dyn_compatible() {
        fn accepts_object_safe_operations(_operations: Option<&dyn McpOperations>) {}

        accepts_object_safe_operations(None);
    }

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
