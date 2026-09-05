//! Model Context Protocol records and shared validation.
//!
//! The crate retains the persisted vocabulary, repository contract, catalog,
//! limits, identity, and policy validation needed by evaluation support.

#[cfg(test)]
mod catalog;
#[cfg(test)]
mod eligibility;
mod identity;
#[cfg(test)]
mod limits;
mod model;
mod repository;
#[cfg(test)]
mod test_fixture;
pub use identity::discovered_tool_fingerprint;
pub use model::{
    McpControlPlaneServer, McpControlPlaneTool, McpDataSharingPolicy, McpDefinitionRecord,
    McpDiscoveredTool, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolHint,
    McpToolHintSource, McpToolPolicyOverride, McpToolPolicyRecord, McpToolPolicyStatus,
    McpToolRecord, McpTransportKind, McpUnsafeActionPolicy, NewMcpServer, validate_provider_policy,
};
pub use repository::{
    McpConnectionLabelUpdate, McpConnectionReplacement, McpDefinitionTarget, McpDeleteTicket,
    McpDiscoveryCommit, McpFailureStatus, McpInitialDiscoveryCommit, McpInvocationSnapshot,
    McpProviderPolicyUpdate, McpRepository, McpRepositoryError, McpRepositoryErrorKind,
    McpRepositoryFuture, McpRepositoryHandle, McpRepositoryResult, McpResetToolPolicyUpdate,
    McpSetToolEnabledUpdate, McpToolPolicyOverrideUpdate,
};
