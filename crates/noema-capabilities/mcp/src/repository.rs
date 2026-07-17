use std::{fmt, future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    McpControlPlaneServer, McpDiscoveredTool, McpServerAuthStatus, McpServerHealthStatus,
    McpServerRecord, McpToolRecord, McpTransportKind, NewMcpServer, NewToolCalibration,
    ToolCalibrationRecord,
};

/// Boxed future returned by object-safe MCP repository operations.
pub type McpRepositoryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Coarse error class safe for MCP orchestration decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpRepositoryErrorKind {
    /// Requested record does not exist.
    NotFound,
    /// Input conflicts with durable state or violates an invariant.
    Conflict,
    /// Durable state is corrupt or internally inconsistent.
    Invariant,
    /// Repository could not complete the operation.
    Unavailable,
}

/// Repository failure without backend-specific error exposure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct McpRepositoryError {
    kind: McpRepositoryErrorKind,
    message: String,
}

impl McpRepositoryError {
    /// Construct a repository error.
    #[must_use]
    pub fn new(kind: McpRepositoryErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Return the coarse failure class.
    #[must_use]
    pub const fn kind(&self) -> McpRepositoryErrorKind {
        self.kind
    }
}

/// Repository operation result.
pub type McpRepositoryResult<T> = Result<T, McpRepositoryError>;

/// Initial atomic server-and-discovery commit after remote metadata validation.
#[derive(Debug, Clone, PartialEq)]
pub struct McpInitialDiscoveryCommit {
    /// Validated server configuration without a durable id.
    pub server: NewMcpServer,
    /// Complete discovered tool set.
    pub tools: Vec<McpDiscoveredTool>,
    /// Authentication state proven by discovery.
    pub auth_status: McpServerAuthStatus,
}

/// Atomic discovery reconciliation request.
#[derive(Debug, Clone, PartialEq)]
pub struct McpDiscoveryCommit {
    /// Server whose exact current catalog is being committed.
    pub mcp_server_id: String,
    /// Authority generation whose connection produced this discovery.
    pub expected_authority_generation: String,
    /// Exact discovered tool set. Missing prior tools are removed.
    pub tools: Vec<McpDiscoveredTool>,
    /// Health state after discovery.
    pub health_status: McpServerHealthStatus,
    /// Authentication state after discovery.
    pub auth_status: McpServerAuthStatus,
}

/// Generation-fenced server connection replacement.
#[derive(Debug, Clone, PartialEq)]
pub struct McpConnectionReplacement {
    /// Server being changed.
    pub mcp_server_id: String,
    /// Authority generation observed before acquiring the service lock.
    pub expected_authority_generation: String,
    /// Replacement transport.
    pub transport_kind: McpTransportKind,
    /// Replacement non-secret configuration.
    pub safe_config: serde_json::Value,
}

/// Generation-fenced failure projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpFailureStatus {
    /// Server whose current connection failed.
    pub mcp_server_id: String,
    /// Authority generation used by the failed work.
    pub expected_authority_generation: String,
    /// Resulting health status.
    pub health_status: McpServerHealthStatus,
    /// Resulting authentication status.
    pub auth_status: McpServerAuthStatus,
}

/// Ticket returned after atomically fencing a server from new calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDeleteTicket {
    /// Server being deleted.
    pub mcp_server_id: String,
    /// Fresh deletion generation guarding final removal.
    pub deletion_generation: String,
}

/// Joined invocation snapshot read atomically from durable state.
#[derive(Debug, Clone, PartialEq)]
pub struct McpInvocationSnapshot {
    /// Current server record.
    pub server: McpServerRecord,
    /// Current tool record.
    pub tool: McpToolRecord,
    /// Current calibration, when present.
    pub calibration: Option<ToolCalibrationRecord>,
}

/// Task-oriented durable MCP operations.
///
/// Implementations preserve atomicity across the server, tool, and
/// calibration projections rather than exposing backend-shaped CRUD.
pub trait McpRepository: Send + Sync + fmt::Debug {
    /// Allocate collision-resistant server/tool identities and commit the
    /// verified initial server plus its exact discovered catalog atomically.
    fn commit_initial_discovery(
        &self,
        input: McpInitialDiscoveryCommit,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>>;

    /// Return one joined control-plane view.
    fn control_plane_server(
        &self,
        mcp_server_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpControlPlaneServer>>>;

    /// Return all joined control-plane views in deterministic display order.
    fn control_plane_catalog(
        &self,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Vec<McpControlPlaneServer>>>;

    /// Re-read one exact server/tool/calibration snapshot for invocation.
    fn invocation_snapshot(
        &self,
        mcp_server_id: String,
        mcp_tool_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpInvocationSnapshot>>>;

    /// Replace connection identity, rotating authority only when the
    /// transport-defining metadata changes.
    fn replace_connection(
        &self,
        input: McpConnectionReplacement,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpServerRecord>>;

    /// Reconcile an exact discovery result, invalidate stale reviews, remove
    /// missing tools, and update health/auth/enabled projections atomically.
    fn commit_discovery(
        &self,
        input: McpDiscoveryCommit,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>>;

    /// Persist a failure status before returning a safe setup/invocation error.
    fn record_failure_status(
        &self,
        input: McpFailureStatus,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<bool>>;

    /// Validate and save a complete calibration batch transactionally.
    fn save_calibrations(
        &self,
        calibrations: Vec<NewToolCalibration>,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Vec<ToolCalibrationRecord>>>;

    /// Make a server uncallable and rotate its authority before secret cleanup.
    fn begin_delete(
        &self,
        mcp_server_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpDeleteTicket>>>;

    /// Delete active server, tool, and calibration projections if the deletion
    /// fence is still current. Historical audit rows remain outside this port.
    fn finish_delete(
        &self,
        ticket: McpDeleteTicket,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<bool>>;
}

/// Shared MCP repository handle.
pub type McpRepositoryHandle = Arc<dyn McpRepository>;
