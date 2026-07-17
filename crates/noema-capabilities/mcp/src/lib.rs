//! Model Context Protocol capability contracts and local implementation.
//!
//! The always-compiled surface owns MCP domain vocabulary, repository and
//! control-plane ports, catalog bindings, eligibility, and pure autofill
//! validation. The `transport` feature adds the root-bound local service,
//! filesystem secrets, OAuth, protocol clients, setup, and invocation.

mod autofill;
mod catalog;
mod completion;
mod diagnostics;
mod eligibility;
mod identity;
mod limits;
mod model;
mod oauth_model;
mod operations;
mod repository;
mod secret_model;
mod setup_model;

#[cfg(feature = "transport")]
mod client;
#[cfg(feature = "transport")]
mod control;
#[cfg(feature = "transport")]
mod http;
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

pub use autofill::{
    McpAutofillError, McpToolCalibrationSuggestion, build_autofill_prompt, parse_autofill_response,
};
pub use catalog::{MCP_INVOKER_KEY, McpBindingSource, McpOperationAuthority};
#[cfg(feature = "transport")]
pub use client::{
    McpClientError, McpClientFuture, McpClientResult, McpPreparedSession, McpRequestContext,
    McpSessionFactory, McpSessionFactoryHandle, McpSessionFactoryRouter, McpSessionPreparation,
    McpToolCallOutput,
};
pub use completion::{
    McpAutofillCompletion, McpAutofillCompletionError, McpAutofillCompletionHandle,
    McpAutofillCompletionRequest, McpAutofillCompletionResponse, McpCompletionFuture,
};
pub use diagnostics::{
    McpDiagnosticEvent, McpDiagnosticHandle, McpDiagnosticKind, McpDiagnosticSink,
    NoopMcpDiagnostics, SystemErrorMcpDiagnostics,
};
pub use eligibility::{
    McpToolIneligibility, mcp_tool_catalog_ineligibility, mcp_tool_ineligibility,
    prompt_safe_mcp_tool_description,
};
#[cfg(feature = "transport")]
pub use http::{
    McpHttpAuthorization, McpHttpAuthorizationHandle, McpHttpAuthorizationProvider,
    StreamableHttpMcpSessionFactory,
};
pub use identity::discovered_tool_fingerprint;
pub use model::{
    McpCalibrationStatus, McpControlPlaneServer, McpControlPlaneTool, McpDiscoveredTool,
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord, McpTransportKind,
    McpTrustClassification, NewMcpServer, NewToolCalibration, ToolCalibrationRecord,
};
#[cfg(feature = "transport")]
pub use oauth::{
    McpOAuthAttemptContext, McpOAuthCompletion, McpOAuthError, McpOAuthErrorKind, McpOAuthRegistry,
    McpOAuthRegistryConfig, McpOAuthStartRequest,
};
pub use oauth_model::{
    CompleteMcpOAuthSetupCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, StartMcpOAuthReauthenticationCommand,
    StartMcpOAuthSetupCommand,
};
pub use operations::{
    McpAutofillCalibrationsCommand, McpAutofillCalibrationsResult, McpControlPlaneHandle,
    McpDeleteServerCommand, McpDeleteServerResult, McpListToolsCommand, McpOperationError,
    McpOperationFuture, McpOperationResult, McpOperations, McpSaveCalibrationsCommand,
    McpSaveCalibrationsResult, McpServerList, McpToolList,
};
pub use repository::{
    McpConnectionReplacement, McpDeleteTicket, McpDiscoveryCommit, McpFailureStatus,
    McpInitialDiscoveryCommit, McpInvocationSnapshot, McpRepository, McpRepositoryError,
    McpRepositoryErrorKind, McpRepositoryFuture, McpRepositoryHandle, McpRepositoryResult,
};
pub use secret_model::{McpOAuthClientCredentials, McpOAuthStoredCredentials, McpSecretMaterial};
#[cfg(feature = "transport")]
pub use secrets::{
    FilesystemMcpSecretStore, McpSecretCommit, McpSecretStage, McpSecretStore, McpSecretStoreError,
    McpSecretStoreHandle, McpSecretStoreOperation,
};
#[cfg(feature = "transport")]
pub use service::{LocalMcpService, LocalMcpServiceConfig, LocalMcpServiceConstructionError};
pub use setup_model::{
    ContinueMcpServerSetupCommand, CreateMcpServerCommand, McpDiscoveryStatus,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupIssue, McpSetupStatus,
    McpSetupTransportConfig, McpStdioSetupConfig, McpStreamableHttpSetupConfig,
};
#[cfg(feature = "transport")]
pub use stdio::StdioMcpSessionFactory;

/// MCP metadata or tool-call payload was malformed.
pub const SYSTEM_ERROR_MCP_MALFORMED_RESPONSE: &str = "mcp_malformed_response";
/// MCP tool execution failed at the transport or remote tool boundary.
pub const SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE: &str = "mcp_tool_call_failure";
