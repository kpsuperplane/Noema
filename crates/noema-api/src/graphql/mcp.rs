use async_graphql::{InputObject, Json, Result, SimpleObject};
use noema_capabilities_mcp::{
    CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, McpAutofillCalibrationsCommand,
    McpDeleteServerCommand, McpDiscoveryStatus, McpListToolsCommand, McpOAuthSetupAttemptQuery,
    McpOAuthSetupAttemptStatus, McpOAuthSetupAttemptView, McpSaveCalibrationsCommand,
    McpSecretMaterial, McpServerRecord, McpServerSetupResult, McpSetupAuthDetails, McpSetupStatus,
    McpToolCalibrationSuggestion, McpToolRecord, McpTransportKind,
    StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand, ToolCalibrationRecord,
};
use serde_json::Value;

use super::{errors::graphql_error, schema::GraphqlState};

mod input;

use input::{
    json_string_map, parse_create_mcp_server_input, parse_oauth_client_credentials,
    parse_save_tool_calibration_input,
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
    /// Last known server health.
    pub health_status: String,
    /// Last known server authentication state.
    pub auth_status: String,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
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

/// MCP tool metadata and current calibration safe to show in Settings.
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
    /// Current calibration, when configured.
    pub calibration: Option<GraphqlToolCalibration>,
}

impl GraphqlMcpTool {
    fn from_records(tool: McpToolRecord, calibration: Option<ToolCalibrationRecord>) -> Self {
        Self {
            mcp_tool_id: tool.mcp_tool_id,
            mcp_server_id: tool.mcp_server_id,
            name: tool.name,
            description: tool.description,
            input_schema: Json(tool.input_schema),
            output_schema: tool.output_schema.map(Json),
            annotations: Json(tool.annotations),
            metadata_fingerprint: tool.metadata_fingerprint,
            calibration: calibration.map(Into::into),
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
            health_status: server.health_status.as_str().to_string(),
            auth_status: server.auth_status.as_str().to_string(),
            tool_count: server.tool_count,
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

/// MCP tool calibration safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ToolCalibration")]
pub struct GraphqlToolCalibration {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: String,
    /// Effective write classification.
    pub write_classification: String,
    /// Effective export classification.
    pub export_classification: String,
    /// Review/gateway readiness status.
    pub status: String,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

impl From<ToolCalibrationRecord> for GraphqlToolCalibration {
    fn from(calibration: ToolCalibrationRecord) -> Self {
        Self {
            calibration_id: calibration.calibration_id,
            mcp_tool_id: calibration.mcp_tool_id,
            read_classification: calibration.read_classification.as_str().to_string(),
            write_classification: calibration.write_classification.as_str().to_string(),
            export_classification: calibration.export_classification.as_str().to_string(),
            status: calibration.status.as_str().to_string(),
            reviewed_by: calibration.reviewed_by,
            reviewed_metadata_fingerprint: calibration.reviewed_metadata_fingerprint,
        }
    }
}

/// Advisory MCP tool calibration Autofill result.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AutofillToolCalibrationsResult")]
pub struct GraphqlAutofillToolCalibrationsResult {
    /// Validated calibration suggestions keyed by MCP tool id.
    pub suggestions: Vec<GraphqlToolCalibrationSuggestion>,
}

/// Advisory calibration suggestion for one MCP tool.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ToolCalibrationSuggestion")]
pub struct GraphqlToolCalibrationSuggestion {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Suggested read classification.
    pub read_classification: String,
    /// Suggested write classification.
    pub write_classification: String,
    /// Suggested export classification.
    pub export_classification: String,
    /// Optional disabled-state suggestion; null preserves the current frontend draft.
    pub disabled: Option<bool>,
}

impl From<McpToolCalibrationSuggestion> for GraphqlToolCalibrationSuggestion {
    fn from(suggestion: McpToolCalibrationSuggestion) -> Self {
        Self {
            mcp_tool_id: suggestion.mcp_tool_id,
            read_classification: suggestion.read_classification.as_str().to_string(),
            write_classification: suggestion.write_classification.as_str().to_string(),
            export_classification: suggestion.export_classification.as_str().to_string(),
            disabled: suggestion.disabled,
        }
    }
}

/// Save reviewed MCP tool calibration.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveToolCalibrationInput")]
pub struct GraphqlSaveToolCalibrationInput {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: String,
    /// Effective write classification.
    pub write_classification: String,
    /// Effective export classification.
    pub export_classification: String,
    /// Review/gateway readiness status.
    pub status: String,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

const fn setup_status_label(status: McpSetupStatus) -> &'static str {
    match status {
        McpSetupStatus::NeedsAuth => "needs_auth",
        McpSetupStatus::ReadyForCalibration => "ready_for_calibration",
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
        .map(|entry| GraphqlMcpTool::from_records(entry.tool, entry.calibration))
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

pub(super) async fn autofill_tool_calibrations(
    state: &GraphqlState,
    mcp_server_id: String,
) -> Result<GraphqlAutofillToolCalibrationsResult> {
    let result = state
        .mcp_operations()?
        .autofill_calibrations(McpAutofillCalibrationsCommand { mcp_server_id })
        .await
        .map_err(graphql_error)?;

    Ok(GraphqlAutofillToolCalibrationsResult {
        suggestions: result.suggestions.into_iter().map(Into::into).collect(),
    })
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
/// does not reach the calibration step.
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

pub(super) async fn save_tool_calibration(
    state: &GraphqlState,
    input: GraphqlSaveToolCalibrationInput,
) -> Result<GraphqlToolCalibration> {
    let calibration = parse_save_tool_calibration_input(input)?;
    let result = state
        .mcp_operations()?
        .save_calibrations(McpSaveCalibrationsCommand {
            calibrations: vec![calibration],
        })
        .await
        .map_err(graphql_error)?;
    result
        .calibrations
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| graphql_error("MCP calibration save returned no result"))
}

pub(super) async fn save_tool_calibrations(
    state: &GraphqlState,
    inputs: Vec<GraphqlSaveToolCalibrationInput>,
) -> Result<Vec<GraphqlToolCalibration>> {
    let calibrations = inputs
        .into_iter()
        .map(parse_save_tool_calibration_input)
        .collect::<Result<Vec<_>>>()?;

    let result = state
        .mcp_operations()?
        .save_calibrations(McpSaveCalibrationsCommand { calibrations })
        .await
        .map_err(graphql_error)?;
    Ok(result.calibrations.into_iter().map(Into::into).collect())
}
