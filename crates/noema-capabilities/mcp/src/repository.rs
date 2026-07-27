use std::{fmt, future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    McpControlPlaneServer, McpDataSharingPolicy, McpDefinitionRecord, McpDiscoveredTool,
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolPolicyOverride,
    McpToolPolicyRecord, McpToolRecord, McpTransportKind, McpUnsafeActionPolicy, NewMcpServer,
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
    /// Whether setup creates a definition or explicitly reuses one exact revision.
    pub definition: McpDefinitionTarget,
    /// Validated server configuration without a durable id.
    pub server: NewMcpServer,
    /// Optional account or installation label for the concrete connection.
    pub connection_label: Option<String>,
    /// Complete discovered tool set.
    pub tools: Vec<McpDiscoveredTool>,
    /// Authentication state proven by discovery.
    pub auth_status: McpServerAuthStatus,
}

/// Definition selection for a newly discovered MCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpDefinitionTarget {
    /// Create a new definition and its first connection.
    New,
    /// Reuse only this explicitly selected immutable definition revision.
    Existing {
        /// Stable definition identity.
        mcp_definition_id: String,
        /// Exact immutable revision observed by the caller.
        expected_definition_revision: String,
    },
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
    /// Current effective policy, when present.
    pub policy: Option<McpToolPolicyRecord>,
}

/// Exact provider policy update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpProviderPolicyUpdate {
    /// Durable MCP server identifier.
    pub mcp_server_id: String,
    /// Automatic data-sharing policy.
    pub data_sharing_policy: McpDataSharingPolicy,
    /// Approval policy for unsafe calls.
    pub unsafe_action_policy: McpUnsafeActionPolicy,
}

/// Task-oriented durable MCP operations.
///
/// Implementations preserve atomicity across the server, tool, and
/// behavior-policy projections rather than exposing backend-shaped CRUD.
pub trait McpRepository: Send + Sync + fmt::Debug {
    /// Return one reusable non-secret definition by exact structured identity.
    fn definition(
        &self,
        mcp_definition_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpDefinitionRecord>>>;

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

    /// Re-read one exact server/tool/policy snapshot for invocation.
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

    /// Save the two provider policies atomically and advance their revision.
    fn save_provider_policy(
        &self,
        update: McpProviderPolicyUpdate,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpServerRecord>>;

    /// Save one complete human behavior override for the current tool snapshot.
    fn save_tool_override(
        &self,
        update: McpToolPolicyOverride,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>>;

    /// Reset one tool from its current annotations and return the pending/ready policy.
    fn reset_tool_policy(
        &self,
        mcp_tool_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>>;

    /// Disable one tool or re-enable its existing effective policy.
    fn set_tool_enabled(
        &self,
        mcp_tool_id: String,
        enabled: bool,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>>;

    /// Commit model or defaulted missing hints only for the captured pending revision.
    fn complete_tool_policy(
        &self,
        policy: McpToolPolicyRecord,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpToolPolicyRecord>>>;

    /// Make a server uncallable and rotate its authority before secret cleanup.
    fn begin_delete(
        &self,
        mcp_server_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpDeleteTicket>>>;

    /// Delete active server, tool, and behavior-policy projections if the deletion
    /// fence is still current. Historical audit rows remain outside this port.
    fn finish_delete(
        &self,
        ticket: McpDeleteTicket,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<bool>>;
}

/// Shared MCP repository handle.
pub type McpRepositoryHandle = Arc<dyn McpRepository>;
