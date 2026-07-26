//! Model Context Protocol capability contracts and local implementation.
//!
//! The always-compiled surface owns MCP domain vocabulary, repository and
//! control-plane ports, catalog bindings, eligibility, and pure classification
//! validation. The `transport` feature adds the root-bound local service,
//! filesystem secrets, OAuth, protocol clients, setup, and invocation.

#[cfg(any(feature = "transport", test))]
mod catalog;
#[cfg(any(feature = "transport", test))]
mod classification;
mod completion;
mod diagnostics;
#[cfg(any(feature = "transport", test))]
mod eligibility;
mod identity;
mod limits;
mod model;
mod oauth_model;
mod operations;
mod repository;
mod secret_model;
mod setup_model;
#[cfg(test)]
mod test_fixture;

#[cfg(feature = "transport")]
mod client;
#[cfg(feature = "transport")]
mod connection_url;
#[cfg(feature = "transport")]
mod control;
#[cfg(feature = "transport")]
mod http;
#[cfg(feature = "transport")]
mod http_body;
#[cfg(feature = "transport")]
mod invocation;
#[cfg(feature = "transport")]
mod lifecycle;
#[cfg(feature = "transport")]
mod oauth;
#[cfg(feature = "transport")]
mod secrets;
#[cfg(feature = "transport")]
mod service;
#[cfg(feature = "transport")]
mod setup;
#[cfg(feature = "transport")]
mod stdio;

#[cfg(any(feature = "transport", test))]
pub use classification::McpToolHintCompletion;
#[cfg(feature = "transport")]
pub use client::{
    McpClientError, McpClientFuture, McpClientResult, McpPreparedSession, McpRequestContext,
    McpSessionFactory, McpSessionFactoryHandle, McpSessionFactoryRouter, McpSessionPreparation,
    McpToolCallOutput,
};
pub use completion::{
    McpCompletionFuture, McpToolClassificationCompletion, McpToolClassificationError,
    McpToolClassificationHandle, McpToolClassificationRequest, McpToolClassificationResponse,
};
pub use diagnostics::{
    McpDiagnosticEvent, McpDiagnosticHandle, McpDiagnosticKind, McpDiagnosticSink,
    SystemErrorMcpDiagnostics,
};
#[cfg(feature = "transport")]
pub use http::{
    McpHttpAuthorization, McpHttpAuthorizationHandle, McpHttpAuthorizationProvider,
    StreamableHttpMcpSessionFactory,
};
pub use identity::discovered_tool_fingerprint;
pub use model::{
    McpControlPlaneServer, McpControlPlaneTool, McpDataSharingPolicy, McpDiscoveredTool,
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolHint, McpToolHintSource,
    McpToolPolicyOverride, McpToolPolicyRecord, McpToolPolicyStatus, McpToolRecord,
    McpTransportKind, McpUnsafeActionPolicy, NewMcpServer, validate_provider_policy,
};
#[cfg(feature = "transport")]
pub(crate) use oauth::{McpOAuthAttemptContext, McpOAuthStartRequest};
#[cfg(feature = "transport")]
pub use oauth::{McpOAuthError, McpOAuthErrorKind, McpOAuthRegistry, McpOAuthRegistryConfig};
pub use oauth_model::{
    CompleteMcpOAuthSetupCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, StartMcpOAuthReauthenticationCommand,
    StartMcpOAuthSetupCommand,
};
pub use operations::{
    McpControlPlaneHandle, McpDeleteServerCommand, McpDeleteServerResult, McpListToolsCommand,
    McpOperationError, McpOperationFuture, McpOperationResult, McpOperations,
    McpResetToolPolicyCommand, McpSaveProviderPolicyCommand, McpSaveToolOverrideCommand,
    McpServerList, McpSetToolEnabledCommand, McpToolList,
};
pub use repository::{
    McpConnectionReplacement, McpDeleteTicket, McpDiscoveryCommit, McpFailureStatus,
    McpInitialDiscoveryCommit, McpInvocationSnapshot, McpProviderPolicyUpdate, McpRepository,
    McpRepositoryError, McpRepositoryErrorKind, McpRepositoryFuture, McpRepositoryHandle,
    McpRepositoryResult,
};
pub use secret_model::{McpOAuthClientCredentials, McpOAuthStoredCredentials, McpSecretMaterial};
#[cfg(feature = "transport")]
pub use secrets::{
    FilesystemMcpSecretStore, McpSecretCommit, McpSecretStage, McpSecretStore, McpSecretStoreError,
    McpSecretStoreHandle,
};
#[cfg(feature = "transport")]
pub use service::{LocalMcpService, LocalMcpServiceConfig, LocalMcpServiceConstructionError};
pub use setup_model::{
    ContinueMcpServerSetupCommand, CreateMcpServerCommand, McpDiscoveryStatus,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupAuthPreference, McpSetupIssue,
    McpSetupStatus, McpSetupTransportConfig, McpStdioSetupConfig, McpStreamableHttpSetupConfig,
};
#[cfg(feature = "transport")]
pub use stdio::StdioMcpSessionFactory;

/// MCP metadata or tool-call payload was malformed.
pub const SYSTEM_ERROR_MCP_MALFORMED_RESPONSE: &str = "mcp_malformed_response";
/// MCP tool execution failed at the transport or remote tool boundary.
pub const SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE: &str = "mcp_tool_call_failure";
