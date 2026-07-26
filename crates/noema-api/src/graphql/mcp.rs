use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_capabilities_mcp::{
    CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, McpDeleteServerCommand,
    McpDiscoveryStatus, McpListToolsCommand, McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus,
    McpOAuthSetupAttemptView, McpResetToolPolicyCommand, McpSaveProviderPolicyCommand,
    McpSaveToolOverrideCommand, McpSecretMaterial, McpServerRecord, McpServerSetupResult,
    McpSetToolEnabledCommand, McpSetupAuthDetails, McpSetupAuthPreference, McpSetupStatus,
    McpToolHint, McpToolPolicyRecord, McpToolRecord, McpTransportKind,
    StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
};
use serde_json::Value;

use super::{errors::graphql_error, schema::GraphqlState};

mod input;

use input::{
    json_string_map, parse_create_mcp_server_input, parse_oauth_client_credentials,
    parse_provider_policy_input,
};

/// MCP server metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpServer")]
pub struct GraphqlMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: String,
    /// Whether this server is enabled.
    pub enabled: bool,
    /// Automatic provider data-sharing policy, when configured.
    pub data_sharing_policy: Option<String>,
    /// Approval policy for unsafe calls, when configured.
    pub unsafe_action_policy: Option<String>,
    /// Current provider-policy revision.
    pub policy_revision: u64,
    /// Last known server health.
    pub health_status: String,
    /// Last known server authentication state.
    pub auth_status: String,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
    /// Number of tools currently available to call.
    pub available_tool_count: usize,
    /// Number of tools waiting for background classification.
    pub pending_tool_count: usize,
    /// Number of tools using pessimistic fallback hints.
    pub defaulted_tool_count: usize,
    /// Number of tools disabled by the user.
    pub disabled_tool_count: usize,
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
    /// Metadata discovery status.
    pub discovery_status: Option<String>,
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
            setup_status: setup_status_label(result.setup_status).to_string(),
            discovery_status: result
                .discovery_status
                .map(discovery_status_label)
                .map(str::to_string),
            discovered_tool_count: result.discovered_tool_count,
            setup_error: result.issue.map(|issue| issue.to_string()),
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

/// MCP tool metadata and effective behavior policy safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpTool")]
pub struct GraphqlMcpTool {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional human-readable MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Json<Value>,
    /// Optional MCP output schema.
    pub output_schema: Option<Json<Value>>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Json<Value>,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
    /// Current effective behavior policy.
    pub policy: Option<GraphqlMcpToolPolicy>,
}

impl GraphqlMcpTool {
    fn from_records(tool: McpToolRecord, policy: Option<McpToolPolicyRecord>) -> Self {
        Self {
            mcp_tool_id: tool.mcp_tool_id,
            mcp_server_id: tool.mcp_server_id,
            name: tool.name,
            description: tool.description,
            input_schema: Json(tool.input_schema),
            output_schema: tool.output_schema.map(Json),
            annotations: Json(tool.annotations),
            metadata_fingerprint: tool.metadata_fingerprint,
            policy: policy.map(Into::into),
        }
    }
}

impl From<McpServerRecord> for GraphqlMcpServer {
    fn from(server: McpServerRecord) -> Self {
        let browser_oauth_reauthentication_supported = browser_oauth_reauth_supported(&server);
        Self {
            mcp_server_id: server.mcp_server_id,
            display_name: server.display_name,
            transport_kind: server.transport_kind.as_str().to_string(),
            enabled: server.enabled,
            data_sharing_policy: server
                .data_sharing_policy
                .map(|policy| policy.as_str().to_string()),
            unsafe_action_policy: server
                .unsafe_action_policy
                .map(|policy| policy.as_str().to_string()),
            policy_revision: server.policy_revision,
            health_status: server.health_status.as_str().to_string(),
            auth_status: server.auth_status.as_str().to_string(),
            tool_count: server.tool_count,
            available_tool_count: server.available_tool_count,
            pending_tool_count: server.pending_tool_count,
            defaulted_tool_count: server.defaulted_tool_count,
            disabled_tool_count: server.disabled_tool_count,
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

/// One effective behavior hint and its provenance.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpToolHint")]
pub struct GraphqlMcpToolHint {
    /// Effective value, or null while classification is pending.
    pub value: Option<bool>,
    /// Annotation, model, safe-default, or human source.
    pub source: Option<String>,
}

impl From<McpToolHint> for GraphqlMcpToolHint {
    fn from(hint: McpToolHint) -> Self {
        Self {
            value: hint.value,
            source: hint.source.map(|source| source.as_str().to_string()),
        }
    }
}

/// Effective policy for one exact MCP tool metadata snapshot.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpToolPolicy")]
pub struct GraphqlMcpToolPolicy {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read-only behavior.
    pub read_only: GraphqlMcpToolHint,
    /// Effective idempotency behavior.
    pub idempotent: GraphqlMcpToolHint,
    /// Effective destructive behavior.
    pub destructive: GraphqlMcpToolHint,
    /// Effective open-world behavior.
    pub open_world: GraphqlMcpToolHint,
    /// Pending, ready, defaulted, or disabled state.
    pub status: String,
    /// Current tool-policy revision.
    pub policy_revision: u64,
    /// Exact metadata fingerprint covered by this policy.
    pub metadata_fingerprint: String,
}

impl From<McpToolPolicyRecord> for GraphqlMcpToolPolicy {
    fn from(policy: McpToolPolicyRecord) -> Self {
        Self {
            mcp_tool_id: policy.mcp_tool_id,
            read_only: policy.read_only.into(),
            idempotent: policy.idempotent.into(),
            destructive: policy.destructive.into(),
            open_world: policy.open_world.into(),
            status: policy.status.as_str().to_string(),
            policy_revision: policy.policy_revision,
            metadata_fingerprint: policy.metadata_fingerprint,
        }
    }
}

/// Save both provider-scoped MCP policy choices atomically.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveMcpProviderPolicyInput")]
pub struct GraphqlSaveMcpProviderPolicyInput {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// `allow_automatically` or `review_every_call`.
    pub data_sharing_policy: String,
    /// `always_ask`, `reviewer_may_approve`, or `never_ask`.
    pub unsafe_action_policy: String,
}

/// Save a complete human override for one exact tool snapshot.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveMcpToolOverrideInput")]
pub struct GraphqlSaveMcpToolOverrideInput {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read-only behavior.
    pub read_only: bool,
    /// Effective idempotency behavior.
    pub idempotent: bool,
    /// Effective destructive behavior.
    pub destructive: bool,
    /// Effective open-world behavior.
    pub open_world: bool,
    /// Exact metadata fingerprint being overridden.
    pub metadata_fingerprint: String,
}

const fn setup_status_label(status: McpSetupStatus) -> &'static str {
    match status {
        McpSetupStatus::NeedsAuth => "needs_auth",
        McpSetupStatus::AuthenticationAvailable => "authentication_available",
        McpSetupStatus::ReadyForPolicy => "ready_for_policy",
        McpSetupStatus::Unavailable => "unavailable",
        McpSetupStatus::Malformed => "malformed",
    }
}

const fn discovery_status_label(status: McpDiscoveryStatus) -> &'static str {
    match status {
        McpDiscoveryStatus::NeedsAuth => "needs_auth",
        McpDiscoveryStatus::Discovered => "discovered",
        McpDiscoveryStatus::Unavailable => "unavailable",
        McpDiscoveryStatus::Malformed => "malformed",
    }
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

pub(super) async fn mcp_tools(
    state: &GraphqlState,
    mcp_server_id: String,
) -> Result<Vec<GraphqlMcpTool>> {
    let result = state
        .mcp_operations()?
        .list_tools(McpListToolsCommand { mcp_server_id })
        .await
        .map_err(graphql_error)?;
    Ok(result
        .tools
        .into_iter()
        .map(|entry| GraphqlMcpTool::from_records(entry.tool, entry.policy))
        .collect())
}

pub(super) async fn mcp_oauth_setup_attempt(
    state: &GraphqlState,
    attempt_id: String,
) -> Result<Option<GraphqlMcpOAuthSetupAttempt>> {
    let attempt = state
        .mcp_operations()?
        .oauth_setup_attempt(McpOAuthSetupAttemptQuery { attempt_id })
        .await
        .map_err(graphql_error)?
        .map(Into::into);
    Ok(attempt)
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
    input: GraphqlStartMcpServerOAuthSetupInput,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    let setup = parse_create_mcp_server_input(input.server)?;
    let attempt = state
        .mcp_operations()?
        .start_oauth_setup(StartMcpOAuthSetupCommand {
            setup,
            redirect_uri: input.redirect_uri,
        })
        .await
        .map_err(graphql_error)?;
    Ok(attempt.into())
}

pub(super) async fn start_mcp_server_reauthentication_oauth_setup(
    state: &GraphqlState,
    input: GraphqlStartMcpServerReauthenticationOAuthSetupInput,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    let attempt = state
        .mcp_operations()?
        .start_oauth_reauthentication(StartMcpOAuthReauthenticationCommand {
            mcp_server_id: input.mcp_server_id,
            redirect_uri: input.redirect_uri,
        })
        .await
        .map_err(graphql_error)?;
    Ok(attempt.into())
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
    let attempt = state
        .mcp_operations()?
        .complete_oauth_setup(CompleteMcpOAuthSetupCommand {
            attempt_id: attempt_id.to_string(),
            callback_url: callback_url.to_string(),
        })
        .await
        .map_err(graphql_error)?;
    Ok(attempt.into())
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
    Ok(result.deleted)
}

pub(super) async fn save_mcp_provider_policy(
    state: &GraphqlState,
    input: GraphqlSaveMcpProviderPolicyInput,
) -> Result<GraphqlMcpServer> {
    let (data_sharing_policy, unsafe_action_policy) =
        parse_provider_policy_input(&input.data_sharing_policy, &input.unsafe_action_policy)?;
    state
        .mcp_operations()?
        .save_provider_policy(McpSaveProviderPolicyCommand {
            mcp_server_id: input.mcp_server_id,
            data_sharing_policy,
            unsafe_action_policy,
        })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn save_mcp_tool_override(
    state: &GraphqlState,
    input: GraphqlSaveMcpToolOverrideInput,
) -> Result<GraphqlMcpToolPolicy> {
    state
        .mcp_operations()?
        .save_tool_override(McpSaveToolOverrideCommand {
            policy: noema_capabilities_mcp::McpToolPolicyOverride {
                mcp_tool_id: input.mcp_tool_id,
                read_only: input.read_only,
                idempotent: input.idempotent,
                destructive: input.destructive,
                open_world: input.open_world,
                metadata_fingerprint: input.metadata_fingerprint,
            },
        })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn reset_mcp_tool_policy(
    state: &GraphqlState,
    mcp_tool_id: String,
) -> Result<GraphqlMcpToolPolicy> {
    state
        .mcp_operations()?
        .reset_tool_policy(McpResetToolPolicyCommand { mcp_tool_id })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn set_mcp_tool_enabled(
    state: &GraphqlState,
    mcp_tool_id: String,
    enabled: bool,
) -> Result<GraphqlMcpToolPolicy> {
    state
        .mcp_operations()?
        .set_tool_enabled(McpSetToolEnabledCommand {
            mcp_tool_id,
            enabled,
        })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}
