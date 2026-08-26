use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_capabilities_mcp::{
    AddMcpConnectionCommand, CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand,
    McpDeleteServerCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, McpSecretMaterial, McpServerRecord,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupAuthPreference, McpTransportKind,
    StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
};
use serde_json::Value;

use super::{errors::graphql_error, schema::GraphqlState};

mod input;

use input::{json_string_map, parse_create_mcp_server_input, parse_oauth_client_credentials};

/// MCP server metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpServer")]
pub struct GraphqlMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Exact connection authority generation used for policy fencing.
    pub connection_revision: String,
    /// Monotonic provider-policy revision used for policy fencing.
    pub policy_revision: u64,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: String,
    /// Last known server health.
    pub health_status: String,
    /// Last known server authentication state.
    pub auth_status: String,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
    /// Number of tools waiting for background classification.
    pub pending_tool_count: usize,
    /// Whether this persisted server can restart browser OAuth authorization.
    pub browser_oauth_reauthentication_supported: bool,
}

/// Add and verify an MCP server.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateMcpServerInput")]
pub struct GraphqlCreateMcpServerInput {
    /// Human-visible server name.
    pub display_name: String,
    /// MCP transport kind: `stdio` or `streamable_http`.
    pub transport_kind: String,
    /// Stdio transport config, when `transport_kind` is `stdio`.
    pub stdio: Option<GraphqlMcpStdioConfigInput>,
    /// HTTP transport config, when `transport_kind` is `streamable_http`.
    pub http: Option<GraphqlMcpHttpConfigInput>,
    /// How to handle optional browser authentication advertised after discovery.
    pub auth_preference: Option<GraphqlMcpSetupAuthPreference>,
}

/// Add a fresh authenticated connection to one exact MCP definition revision.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "AddMcpConnectionInput")]
pub struct GraphqlAddMcpConnectionInput {
    pub mcp_definition_id: String,
    pub expected_definition_revision: String,
    pub connection_label: Option<String>,
    pub secret_env: Option<Json<Value>>,
    pub secret_headers: Option<Json<Value>>,
    pub oauth_client_credentials: Option<GraphqlMcpOAuthClientCredentialsInput>,
    pub auth_preference: Option<GraphqlMcpSetupAuthPreference>,
}

/// Authentication behavior for an otherwise successful anonymous setup.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "McpSetupAuthPreference")]
pub enum GraphqlMcpSetupAuthPreference {
    /// Offer browser authentication before persisting the server.
    PromptIfAvailable,
    /// Persist only the anonymously visible tools.
    UseAnonymous,
}

impl From<GraphqlMcpSetupAuthPreference> for McpSetupAuthPreference {
    fn from(value: GraphqlMcpSetupAuthPreference) -> Self {
        match value {
            GraphqlMcpSetupAuthPreference::PromptIfAvailable => Self::PromptIfAvailable,
            GraphqlMcpSetupAuthPreference::UseAnonymous => Self::UseAnonymous,
        }
    }
}

/// Stdio MCP setup config.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "McpStdioConfigInput")]
pub struct GraphqlMcpStdioConfigInput {
    /// Command to launch.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<String>,
    /// Non-secret environment variables.
    pub env: Option<Json<Value>>,
    /// Secret environment variables stored on disk.
    pub secret_env: Option<Json<Value>>,
}

/// HTTP MCP setup config.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "McpHttpConfigInput")]
pub struct GraphqlMcpHttpConfigInput {
    /// MCP endpoint URL.
    pub url: String,
    /// Non-secret request headers.
    pub headers: Option<Json<Value>>,
    /// Secret request headers stored on disk.
    pub secret_headers: Option<Json<Value>>,
    /// OAuth client-secret credentials, when supported by the server.
    pub oauth_client_credentials: Option<GraphqlMcpOAuthClientCredentialsInput>,
}

/// OAuth client-secret credentials for MCP setup.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "McpOAuthClientCredentialsInput")]
pub struct GraphqlMcpOAuthClientCredentialsInput {
    /// OAuth client id.
    pub client_id: String,
    /// OAuth client secret stored on disk.
    pub client_secret: String,
    /// OAuth scopes to request.
    pub scopes: Vec<String>,
}

/// Continue setup after adding authentication material.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ContinueMcpServerSetupInput")]
pub struct GraphqlContinueMcpServerSetupInput {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Secret environment variables stored on disk.
    pub secret_env: Option<Json<Value>>,
    /// Secret headers stored on disk.
    pub secret_headers: Option<Json<Value>>,
    /// OAuth client-secret credentials, when supported by the server.
    pub oauth_client_credentials: Option<GraphqlMcpOAuthClientCredentialsInput>,
}

/// Guided MCP setup result.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpServerSetupResult")]
pub struct GraphqlMcpServerSetupResult {
    /// Server metadata safe to show in Settings.
    pub server: Option<GraphqlMcpServer>,
    /// Setup status.
    pub setup_status: String,
    /// Number of discovered tools.
    pub discovered_tool_count: usize,
    /// Non-secret setup error, when present.
    pub setup_error: Option<String>,
    /// Authentication options safe to show to the user.
    pub auth: Option<GraphqlMcpSetupAuthDetails>,
}

/// Authentication options safe to show during MCP setup.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpSetupAuthDetails")]
pub struct GraphqlMcpSetupAuthDetails {
    /// Whether OAuth client-secret credentials can be attempted.
    pub oauth_client_credentials_supported: bool,
    /// Whether browser OAuth authorization can be attempted from the server URL.
    pub oauth_authorization_supported: bool,
    /// Suggested OAuth scopes, when known.
    pub scopes: Vec<String>,
}

/// Start a browser OAuth setup attempt for a hosted MCP server.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "StartMcpServerOAuthSetupInput")]
pub struct GraphqlStartMcpServerOAuthSetupInput {
    /// Pending MCP server setup input.
    pub server: GraphqlCreateMcpServerInput,
    /// Absolute local callback URI owned by Noema web.
    pub redirect_uri: String,
}

/// Start a browser OAuth reauthentication attempt for an existing MCP server.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "StartMcpServerReauthenticationOAuthSetupInput")]
pub struct GraphqlStartMcpServerReauthenticationOAuthSetupInput {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Absolute local callback URI owned by Noema web.
    pub redirect_uri: String,
}

/// Safe browser OAuth setup attempt state.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpOAuthSetupAttempt")]
pub struct GraphqlMcpOAuthSetupAttempt {
    /// Short-lived attempt id.
    pub attempt_id: String,
    /// Current attempt status.
    pub status: String,
    /// Authorization URL to open in the user's browser.
    pub authorization_url: Option<String>,
    /// Final setup result after OAuth callback and tool discovery.
    pub setup_result: Option<GraphqlMcpServerSetupResult>,
    /// Non-secret UI-safe failure message.
    pub error_message: Option<String>,
}

impl From<McpServerSetupResult> for GraphqlMcpServerSetupResult {
    fn from(result: McpServerSetupResult) -> Self {
        Self {
            server: result.server.map(Into::into),
            setup_status: result.setup_status.as_str().to_string(),
            discovered_tool_count: result.discovered_tool_count,
            setup_error: result.setup_status.issue().map(|issue| issue.to_string()),
            auth: result.auth.map(Into::into),
        }
    }
}

impl From<McpSetupAuthDetails> for GraphqlMcpSetupAuthDetails {
    fn from(auth: McpSetupAuthDetails) -> Self {
        Self {
            oauth_client_credentials_supported: auth.oauth_client_credentials_supported,
            oauth_authorization_supported: auth.oauth_authorization_supported,
            scopes: auth.scopes,
        }
    }
}

impl From<McpOAuthSetupAttemptView> for GraphqlMcpOAuthSetupAttempt {
    fn from(attempt: McpOAuthSetupAttemptView) -> Self {
        Self {
            attempt_id: attempt.attempt_id,
            status: oauth_attempt_status_label(attempt.status).to_string(),
            authorization_url: attempt.authorization_url,
            setup_result: attempt.setup_result.map(Into::into),
            error_message: attempt.failure.map(|failure| failure.to_string()),
        }
    }
}

impl From<McpServerRecord> for GraphqlMcpServer {
    fn from(server: McpServerRecord) -> Self {
        let browser_oauth_reauthentication_supported = browser_oauth_reauth_supported(&server);
        Self {
            mcp_server_id: server.mcp_server_id,
            connection_revision: server.authority_generation,
            policy_revision: server.policy_revision,
            display_name: server.display_name,
            transport_kind: server.transport_kind.as_str().to_string(),
            health_status: server.health_status.as_str().to_string(),
            auth_status: server.auth_status.as_str().to_string(),
            tool_count: server.tool_count,
            pending_tool_count: server.pending_tool_count,
            browser_oauth_reauthentication_supported,
        }
    }
}

fn browser_oauth_reauth_supported(server: &McpServerRecord) -> bool {
    matches!(server.transport_kind, McpTransportKind::StreamableHttp)
        && server
            .safe_config
            .get("secret_refs")
            .and_then(Value::as_object)
            .and_then(|refs| refs.get("oauth_credentials"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

const fn oauth_attempt_status_label(status: McpOAuthSetupAttemptStatus) -> &'static str {
    match status {
        McpOAuthSetupAttemptStatus::WaitingForUser => "waiting_for_user",
        McpOAuthSetupAttemptStatus::Completed => "completed",
        McpOAuthSetupAttemptStatus::Failed => "failed",
    }
}

pub(super) async fn mcp_servers(state: &GraphqlState) -> Result<Vec<GraphqlMcpServer>> {
    let result = state
        .mcp_operations()?
        .list_servers()
        .await
        .map_err(graphql_error)?;
    Ok(result.servers.into_iter().map(Into::into).collect())
}

pub(super) async fn mcp_oauth_setup_attempt(
    state: &GraphqlState,
    principal: &str,
    attempt_id: String,
) -> Result<Option<GraphqlMcpOAuthSetupAttempt>> {
    let attempt = state
        .mcp_operations()?
        .oauth_setup_attempt(McpOAuthSetupAttemptQuery {
            attempt_id,
            owner_human_id: Some(principal.to_string()),
        })
        .await
        .map_err(graphql_error)?;
    if let Some(attempt) = attempt.as_ref()
        && attempt.status == McpOAuthSetupAttemptStatus::Failed
        && attempt.failure != Some(McpOAuthSetupFailure::Superseded)
        && let Some(store) = state.optional_store()
    {
        store
            .reset_capability_authentication_attempt(&attempt.attempt_id, "oauth_failed")
            .await?;
    }
    Ok(attempt.map(Into::into))
}

pub(super) async fn add_mcp_connection(
    state: &GraphqlState,
    input: GraphqlAddMcpConnectionInput,
) -> Result<GraphqlMcpServerSetupResult> {
    state
        .mcp_operations()?
        .add_connection(AddMcpConnectionCommand {
            mcp_definition_id: input.mcp_definition_id,
            expected_definition_revision: input.expected_definition_revision,
            connection_label: input.connection_label,
            secrets: McpSecretMaterial {
                secret_identity_revision: None,
                env: json_string_map(input.secret_env, "secretEnv")?,
                headers: json_string_map(input.secret_headers, "secretHeaders")?,
                oauth_client_credentials: parse_oauth_client_credentials(
                    input.oauth_client_credentials,
                )?,
                oauth_credentials: None,
            },
            auth_preference: input.auth_preference.map(Into::into).unwrap_or_default(),
        })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn create_mcp_server(
    state: &GraphqlState,
    input: GraphqlCreateMcpServerInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let command = parse_create_mcp_server_input(input)?;
    let result = state
        .mcp_operations()?
        .create_server(command)
        .await
        .map_err(graphql_error)?;
    Ok(result.into())
}

pub(super) async fn start_mcp_server_oauth_setup(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlStartMcpServerOAuthSetupInput,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    require_exact_oauth_callback(state, &input.redirect_uri)?;
    let _start = state.mcp_oauth_start_lock().lock().await;
    let setup = parse_create_mcp_server_input(input.server)?;
    let attempt = state
        .mcp_operations()?
        .start_oauth_setup(StartMcpOAuthSetupCommand {
            owner_human_id: principal.to_string(),
            setup,
            redirect_uri: input.redirect_uri,
        })
        .await
        .map_err(graphql_error)?;
    if attempt.status == McpOAuthSetupAttemptStatus::Completed {
        resume_mcp_authentication_attempt(state, &attempt.attempt_id).await?;
    }
    Ok(attempt.into())
}

pub(super) async fn start_mcp_server_reauthentication_oauth_setup(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlStartMcpServerReauthenticationOAuthSetupInput,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    require_exact_oauth_callback(state, &input.redirect_uri)?;
    let _start = state.mcp_oauth_start_lock().lock().await;
    let operations = state.mcp_operations()?;
    if let Some(store) = state.optional_store()
        && let Some(attempt_id) = store
            .active_mcp_authentication_attempt(principal, &input.mcp_server_id)
            .await?
    {
        if let Some(attempt) = operations
            .oauth_setup_attempt(McpOAuthSetupAttemptQuery {
                attempt_id: attempt_id.clone(),
                owner_human_id: Some(principal.to_string()),
            })
            .await
            .map_err(graphql_error)?
            && attempt.status != McpOAuthSetupAttemptStatus::Failed
        {
            return Ok(attempt.into());
        }
        store
            .reset_capability_authentication_attempt(&attempt_id, "oauth_attempt_missing")
            .await?;
    }
    let attempt = operations
        .start_oauth_reauthentication(StartMcpOAuthReauthenticationCommand {
            owner_human_id: principal.to_string(),
            mcp_server_id: input.mcp_server_id,
            redirect_uri: input.redirect_uri,
        })
        .await
        .map_err(graphql_error)?;
    Ok(attempt.into())
}

pub(super) fn require_exact_oauth_callback(state: &GraphqlState, callback: &str) -> Result<()> {
    if callback != state.mcp_oauth_callback_url()? {
        return Err(async_graphql::Error::new(
            "MCP OAuth callback does not match this Noema process",
        ));
    }
    Ok(())
}

/// Complete a pending MCP OAuth setup attempt from an authorization callback URL.
///
/// This is used by both the daemon web callback route and the desktop
/// loopback callback listener.
///
/// # Errors
///
/// Returns a GraphQL error if the attempt no longer exists, if OAuth callback
/// handling fails, if credentials cannot be stored, or if MCP tool discovery
/// does not reach provider-policy setup.
pub async fn complete_mcp_server_oauth_setup(
    state: &GraphqlState,
    attempt_id: &str,
    callback_url: &str,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    let operations = state.mcp_operations()?;
    let attempt = match operations
        .complete_oauth_setup(CompleteMcpOAuthSetupCommand {
            attempt_id: attempt_id.to_string(),
            callback_url: callback_url.to_string(),
        })
        .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            let reset_retryable = operations
                .oauth_setup_attempt(McpOAuthSetupAttemptQuery {
                    attempt_id: attempt_id.to_string(),
                    owner_human_id: None,
                })
                .await
                .ok()
                .flatten()
                .is_some_and(|attempt| {
                    attempt.status == McpOAuthSetupAttemptStatus::Failed
                        && attempt.failure != Some(McpOAuthSetupFailure::Superseded)
                });
            if reset_retryable && let Some(store) = state.optional_store() {
                store
                    .reset_capability_authentication_attempt(attempt_id, "oauth_failed")
                    .await?;
            }
            return Err(graphql_error(error));
        }
    };
    if attempt.status == McpOAuthSetupAttemptStatus::Completed {
        resume_mcp_authentication_attempt(state, &attempt.attempt_id).await?;
    }
    Ok(attempt.into())
}

async fn resume_mcp_authentication_attempt(state: &GraphqlState, attempt_id: &str) -> Result<()> {
    if let Some(runtime) = state.optional_runtime() {
        runtime
            .resume_mcp_authentication_attempt(attempt_id.to_string())
            .await?;
    }
    Ok(())
}

pub(super) async fn continue_mcp_server_setup(
    state: &GraphqlState,
    input: GraphqlContinueMcpServerSetupInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let result = state
        .mcp_operations()?
        .continue_setup(ContinueMcpServerSetupCommand {
            mcp_server_id: input.mcp_server_id,
            secrets: McpSecretMaterial {
                secret_identity_revision: None,
                env: json_string_map(input.secret_env, "secretEnv")?,
                headers: json_string_map(input.secret_headers, "secretHeaders")?,
                oauth_client_credentials: parse_oauth_client_credentials(
                    input.oauth_client_credentials,
                )?,
                oauth_credentials: None,
            },
        })
        .await
        .map_err(graphql_error)?;
    Ok(result.into())
}

pub(super) async fn delete_mcp_server(state: &GraphqlState, mcp_server_id: String) -> Result<bool> {
    let result = state
        .mcp_operations()?
        .delete_server(McpDeleteServerCommand { mcp_server_id })
        .await
        .map_err(graphql_error)?;
    if result.deleted {
        state
            .runtime()?
            .publish_capability_authentication_origins()
            .await?;
    }
    Ok(result.deleted)
}
