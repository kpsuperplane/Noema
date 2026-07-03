use std::collections::BTreeMap;

use async_graphql::{InputObject, Json, Result, SimpleObject};
use serde_json::Value;

use crate::{
    McpApprovalRequestRecord, McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus,
    McpServerRecord, McpToolRecord, McpTransportKind, McpTrustClassification, NewToolCalibration,
    OwnerExtractor, OwnerExtractorSource, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorKind, TrustedIdentitySelectorRecord,
    mcp::{
        McpOAuthSetupAttemptView, StartMcpOAuthSetupRequest,
        oauth::oauth_secret_material,
        secrets::{McpOAuthClientCredentials, McpSecretMaterial},
        setup::{
            ContinueMcpServerSetup, McpServerSetupResult, McpSetupAuthDetails, NewMcpServerSetup,
            continue_mcp_server_setup as continue_setup_service,
            create_mcp_server_setup as create_setup_service,
        },
    },
};

use super::{errors::graphql_error, schema::GraphqlState};

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
}

/// Add and verify an MCP server.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateMcpServerInput")]
pub struct GraphqlCreateMcpServerInput {
    /// Human-visible server name.
    pub display_name: String,
    /// MCP transport kind: `stdio`, `sse`, or `streamable_http`.
    pub transport_kind: String,
    /// Stdio transport config, when `transport_kind` is `stdio`.
    pub stdio: Option<GraphqlMcpStdioConfigInput>,
    /// HTTP transport config, when `transport_kind` is `sse` or `streamable_http`.
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
            discovery_status: result.discovery_status,
            discovered_tool_count: result.discovered_tool_count,
            setup_error: result.setup_error,
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
            status: attempt.status.as_str().to_string(),
            authorization_url: attempt.authorization_url,
            setup_result: attempt.setup_result.map(Into::into),
            error_message: attempt.error_message,
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
        Self {
            mcp_server_id: server.mcp_server_id,
            display_name: server.display_name,
            transport_kind: server.transport_kind.as_str().to_string(),
            enabled: server.enabled,
            health_status: health_status_label(server.health_status).to_string(),
            auth_status: auth_status_label(server.auth_status).to_string(),
            tool_count: server.tool_count,
        }
    }
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
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<GraphqlOwnerExtractor>,
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
            owner_extractors: calibration
                .owner_extractors
                .into_iter()
                .map(Into::into)
                .collect(),
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
    /// Suggested owner extractors.
    pub owner_extractors: Vec<GraphqlOwnerExtractor>,
    /// Optional disabled-state suggestion; null preserves the current frontend draft.
    pub disabled: Option<bool>,
}

impl From<crate::mcp::autofill::McpToolCalibrationSuggestion> for GraphqlToolCalibrationSuggestion {
    fn from(suggestion: crate::mcp::autofill::McpToolCalibrationSuggestion) -> Self {
        Self {
            mcp_tool_id: suggestion.mcp_tool_id,
            read_classification: suggestion.read_classification.as_str().to_string(),
            write_classification: suggestion.write_classification.as_str().to_string(),
            export_classification: suggestion.export_classification.as_str().to_string(),
            owner_extractors: suggestion
                .owner_extractors
                .into_iter()
                .map(Into::into)
                .collect(),
            disabled: suggestion.disabled,
        }
    }
}

/// Deterministic owner extractor safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "OwnerExtractor")]
pub struct GraphqlOwnerExtractor {
    /// Source document or field family to inspect.
    pub source: String,
    /// Type of trusted identity this extractor returns.
    pub selector_kind: String,
    /// JSON pointer, JSONPath-style path, URI pattern, or adapter key.
    pub path: String,
}

impl From<OwnerExtractor> for GraphqlOwnerExtractor {
    fn from(extractor: OwnerExtractor) -> Self {
        Self {
            source: owner_extractor_source_label(extractor.source).to_string(),
            selector_kind: extractor.selector_kind.as_str().to_string(),
            path: extractor.path,
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
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<GraphqlOwnerExtractorInput>,
    /// Review/gateway readiness status.
    pub status: String,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Deterministic owner extractor input for MCP ownership resolution.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "OwnerExtractorInput")]
pub struct GraphqlOwnerExtractorInput {
    /// Source document or field family to inspect.
    pub source: String,
    /// Type of trusted identity this extractor returns.
    pub selector_kind: String,
    /// JSON pointer, JSONPath-style path, URI pattern, or adapter key.
    pub path: String,
}

/// Trusted identity selector metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TrustedIdentitySelector")]
pub struct GraphqlTrustedIdentitySelector {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: String,
    /// Normalized selector value.
    pub normalized_value: String,
    /// Selector effect.
    pub effect: String,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
}

impl From<TrustedIdentitySelectorRecord> for GraphqlTrustedIdentitySelector {
    fn from(selector: TrustedIdentitySelectorRecord) -> Self {
        Self {
            selector_id: selector.selector_id,
            owner_scope_id: selector.owner_scope_id,
            selector_kind: selector.selector_kind.as_str().to_string(),
            normalized_value: selector.normalized_value,
            effect: selector_effect_label(selector.effect).to_string(),
            issuer_actor_id: selector.issuer_actor_id,
        }
    }
}

/// MCP approval request metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpApprovalRequest")]
pub struct GraphqlMcpApprovalRequest {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id.
    pub tool_invocation_id: String,
    /// Related MCP server id, when available.
    pub mcp_server_id: Option<String>,
    /// Related MCP tool id, when available.
    pub mcp_tool_id: Option<String>,
    /// Actor requesting approval.
    pub requester_actor_id: String,
    /// Governable owner scope for the approval.
    pub owner_scope_id: String,
    /// Active governable scope for the approval.
    pub active_scope_id: String,
    /// Destination or recipient summary.
    pub destination_summary: String,
    /// Data source summary.
    pub data_source_summary: String,
    /// Source owner identity label.
    pub source_owner_identity: String,
    /// Source owner trust label.
    pub source_owner_trust: String,
    /// Destination owner identity label.
    pub destination_owner_identity: String,
    /// Destination owner trust label.
    pub destination_owner_trust: String,
    /// What leaves the MCP destination trust boundary.
    pub export_summary: String,
    /// Safe payload preview for review surfaces.
    pub payload_preview: Json<Value>,
    /// Current approval status.
    pub status: String,
    /// Actor who decided the request, when decided.
    pub decision_actor_id: Option<String>,
    /// Safe decision comment, when available.
    pub decision_comment: Option<String>,
    /// Decision timestamp string, when decided.
    pub decided_at: Option<String>,
}

impl From<McpApprovalRequestRecord> for GraphqlMcpApprovalRequest {
    fn from(approval: McpApprovalRequestRecord) -> Self {
        Self {
            approval_id: approval.approval_id,
            action_summary: approval.action_summary,
            tool_invocation_id: approval.tool_invocation_id,
            mcp_server_id: approval.mcp_server_id,
            mcp_tool_id: approval.mcp_tool_id,
            requester_actor_id: approval.requester_actor_id,
            owner_scope_id: approval.owner_scope_id,
            active_scope_id: approval.active_scope_id,
            destination_summary: approval.destination_summary,
            data_source_summary: approval.data_source_summary,
            source_owner_identity: approval.source_owner_identity,
            source_owner_trust: approval.source_owner_trust,
            destination_owner_identity: approval.destination_owner_identity,
            destination_owner_trust: approval.destination_owner_trust,
            export_summary: approval.export_summary,
            payload_preview: Json(approval.payload_preview),
            status: approval.status,
            decision_actor_id: approval.decision_actor_id,
            decision_comment: approval.decision_comment,
            decided_at: approval.decided_at,
        }
    }
}

const fn health_status_label(status: McpServerHealthStatus) -> &'static str {
    match status {
        McpServerHealthStatus::Unknown => "unknown",
        McpServerHealthStatus::Healthy => "healthy",
        McpServerHealthStatus::Unavailable => "unavailable",
    }
}

const fn auth_status_label(status: McpServerAuthStatus) -> &'static str {
    match status {
        McpServerAuthStatus::None => "none",
        McpServerAuthStatus::NeedsAuth => "needs_auth",
        McpServerAuthStatus::Authenticated => "authenticated",
        McpServerAuthStatus::Unavailable => "unavailable",
    }
}

const fn selector_effect_label(effect: TrustedIdentitySelectorEffect) -> &'static str {
    match effect {
        TrustedIdentitySelectorEffect::Trust => "trust",
        TrustedIdentitySelectorEffect::Restrict => "restrict",
    }
}

const fn owner_extractor_source_label(source: OwnerExtractorSource) -> &'static str {
    match source {
        OwnerExtractorSource::Arguments => "arguments",
        OwnerExtractorSource::StructuredContent => "structured_content",
        OwnerExtractorSource::Metadata => "metadata",
        OwnerExtractorSource::ResourceUri => "resource_uri",
        OwnerExtractorSource::BuiltInAdapter => "built_in_adapter",
    }
}

pub(super) async fn mcp_servers(state: &GraphqlState) -> Result<Vec<GraphqlMcpServer>> {
    let store = state.store()?;
    let servers = store.list_mcp_servers().await.map_err(graphql_error)?;
    Ok(servers.into_iter().map(Into::into).collect())
}

pub(super) async fn mcp_tools(
    state: &GraphqlState,
    mcp_server_id: String,
) -> Result<Vec<GraphqlMcpTool>> {
    let store = state.store()?;
    let tools = store
        .list_mcp_tools_for_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?;
    let mut graphql_tools = Vec::with_capacity(tools.len());
    for tool in tools {
        let calibration = store
            .get_tool_calibration(&tool.mcp_tool_id)
            .await
            .map_err(graphql_error)?;
        graphql_tools.push(GraphqlMcpTool::from_records(tool, calibration));
    }
    Ok(graphql_tools)
}

pub(super) async fn mcp_oauth_setup_attempt(
    state: &GraphqlState,
    attempt_id: String,
) -> Result<Option<GraphqlMcpOAuthSetupAttempt>> {
    let attempt = state
        .mcp_oauth()?
        .attempt(&attempt_id)
        .await
        .map(Into::into);
    Ok(attempt)
}

pub(super) async fn autofill_tool_calibrations(
    state: &GraphqlState,
    mcp_server_id: String,
) -> Result<GraphqlAutofillToolCalibrationsResult> {
    let store = state.store()?;
    let server = store
        .get_mcp_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| graphql_error("MCP server was not found"))?;
    let tools = store
        .list_mcp_tools_for_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?;
    let prompt = crate::mcp::autofill::build_autofill_prompt(&server.display_name, &tools);
    let runtime = state.runtime()?;
    let mut request = crate::GenerateRequest::text(prompt);
    if let Some(model) = runtime.tool_classification_model() {
        request.model = Some(model.to_string());
    }
    request.instructions =
        Some("Return strict JSON only for MCP calibration suggestions.".to_string());

    let response = runtime
        .generate_once(request)
        .await
        .map_err(graphql_error)?;
    let suggestions =
        crate::mcp::autofill::parse_autofill_response(&response.assistant_text(), &tools)
            .map_err(graphql_error)?;

    Ok(GraphqlAutofillToolCalibrationsResult {
        suggestions: suggestions.into_iter().map(Into::into).collect(),
    })
}

pub(super) async fn create_mcp_server(
    state: &GraphqlState,
    input: GraphqlCreateMcpServerInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let store = state.store()?;
    let paths = state.paths()?;
    let mut setup_input = parse_create_mcp_server_input(input)?;
    setup_input.browser_oauth_supported = state.mcp_browser_oauth_supported(&setup_input).await;
    let result = create_setup_service(store, paths, setup_input, |server, secrets| {
        state.mcp_setup_transport(server, Some(secrets))
    })
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
        .mcp_oauth()?
        .start_attempt(StartMcpOAuthSetupRequest {
            setup,
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
    let manager = state.mcp_oauth()?;
    let Some(mut runtime) = manager.take_runtime(attempt_id).await else {
        return Err(graphql_error("MCP OAuth setup attempt was not found"));
    };
    if let Err(error) = runtime.oauth_state.handle_callback_url(callback_url).await {
        let attempt = manager
            .fail_attempt(
                attempt_id,
                "Noema could not complete MCP OAuth authorization.",
            )
            .await;
        return attempt
            .map(Into::into)
            .ok_or_else(|| graphql_error(format!("MCP OAuth setup attempt failed: {error}")));
    }
    let secrets = match oauth_secret_material(&runtime.oauth_state, runtime.setup.secrets).await {
        Ok(secrets) => secrets,
        Err(error) => {
            let attempt = manager
                .fail_attempt(attempt_id, "Noema could not store MCP OAuth credentials.")
                .await;
            return attempt.map(Into::into).ok_or_else(|| graphql_error(error));
        }
    };
    runtime.setup.secrets = secrets;
    let store = state.store()?;
    let paths = state.paths()?;
    let setup_result = create_setup_service(store, paths, runtime.setup, |server, secrets| {
        state.mcp_setup_transport(server, Some(secrets))
    })
    .await
    .map_err(graphql_error)?;
    let status = setup_result.setup_status;
    let attempt = if matches!(
        status,
        crate::mcp::setup::McpSetupStatus::ReadyForCalibration
    ) {
        manager.complete_attempt(attempt_id, setup_result).await
    } else {
        manager
            .fail_attempt(
                attempt_id,
                "Noema completed OAuth, but could not list tools from this MCP server.",
            )
            .await
    };
    attempt
        .map(Into::into)
        .ok_or_else(|| graphql_error("MCP OAuth setup attempt was not found"))
}

pub(super) async fn continue_mcp_server_setup(
    state: &GraphqlState,
    input: GraphqlContinueMcpServerSetupInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let store = state.store()?;
    let paths = state.paths()?;
    let result = continue_setup_service(
        store,
        paths,
        ContinueMcpServerSetup {
            mcp_server_id: input.mcp_server_id,
            secrets: McpSecretMaterial {
                env: json_string_map(input.secret_env, "secretEnv")?,
                headers: json_string_map(input.secret_headers, "secretHeaders")?,
                oauth_client_credentials: parse_oauth_client_credentials(
                    input.oauth_client_credentials,
                )?,
                oauth_credentials: None,
            },
        },
        |server, secrets| state.mcp_setup_transport(server, Some(secrets)),
    )
    .await
    .map_err(graphql_error)?;
    Ok(result.into())
}

pub(super) async fn delete_mcp_server(state: &GraphqlState, mcp_server_id: String) -> Result<bool> {
    let store = state.store()?;
    let paths = state.paths()?;
    if store
        .get_mcp_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?
        .is_none()
    {
        return Ok(false);
    }
    crate::mcp::secrets::remove_mcp_secrets_dir(&paths.mcp_server_home(&mcp_server_id))
        .map_err(graphql_error)?;
    let deleted = store
        .delete_mcp_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?;
    Ok(deleted)
}

pub(super) async fn trusted_identity_selectors(
    state: &GraphqlState,
    owner_scope_id: String,
) -> Result<Vec<GraphqlTrustedIdentitySelector>> {
    let store = state.store()?;
    let selectors = store
        .list_trusted_identity_selectors(&owner_scope_id)
        .await
        .map_err(graphql_error)?;
    Ok(selectors.into_iter().map(Into::into).collect())
}

pub(super) async fn mcp_approval_requests(
    state: &GraphqlState,
    status: Option<String>,
) -> Result<Vec<GraphqlMcpApprovalRequest>> {
    let store = state.store()?;
    let approvals = store
        .list_mcp_approval_requests(status.as_deref())
        .await
        .map_err(graphql_error)?;
    Ok(approvals.into_iter().map(Into::into).collect())
}

pub(super) async fn save_tool_calibration(
    state: &GraphqlState,
    input: GraphqlSaveToolCalibrationInput,
) -> Result<GraphqlToolCalibration> {
    let calibration = parse_save_tool_calibration_input(input)?;
    let store = state.store()?;
    let saved = store
        .save_tool_calibration(calibration)
        .await
        .map_err(graphql_error)?;
    Ok(saved.into())
}

pub(super) async fn save_tool_calibrations(
    state: &GraphqlState,
    inputs: Vec<GraphqlSaveToolCalibrationInput>,
) -> Result<Vec<GraphqlToolCalibration>> {
    let calibrations = inputs
        .into_iter()
        .map(parse_save_tool_calibration_input)
        .collect::<Result<Vec<_>>>()?;

    let store = state.store()?;
    let saved = store
        .save_tool_calibrations(calibrations)
        .await
        .map_err(graphql_error)?;
    Ok(saved.into_iter().map(Into::into).collect())
}

fn parse_save_tool_calibration_input(
    input: GraphqlSaveToolCalibrationInput,
) -> Result<NewToolCalibration> {
    let read_classification =
        parse_graphql_trust_classification(&input.read_classification, "readClassification")?;
    let write_classification =
        parse_graphql_trust_classification(&input.write_classification, "writeClassification")?;
    let export_classification =
        parse_graphql_trust_classification(&input.export_classification, "exportClassification")?;
    let status = parse_graphql_calibration_status(&input.status)?;
    let owner_extractors = input
        .owner_extractors
        .into_iter()
        .map(parse_graphql_owner_extractor)
        .collect::<Result<Vec<_>>>()?;

    Ok(NewToolCalibration {
        calibration_id: input.calibration_id,
        mcp_tool_id: input.mcp_tool_id,
        read_classification,
        write_classification,
        export_classification,
        owner_extractors,
        status,
        reviewed_by: input.reviewed_by,
        reviewed_metadata_fingerprint: input.reviewed_metadata_fingerprint,
    })
}

fn parse_graphql_trust_classification(
    value: &str,
    field_name: &'static str,
) -> Result<McpTrustClassification> {
    match value {
        "none" => Ok(McpTrustClassification::None),
        "trusted" => Ok(McpTrustClassification::Trusted),
        "untrusted" => Ok(McpTrustClassification::Untrusted),
        "mixed" => Ok(McpTrustClassification::Mixed),
        _ => Err(graphql_error(format!(
            "invalid {field_name}: expected one of none, trusted, untrusted, mixed"
        ))),
    }
}

fn parse_create_mcp_server_input(input: GraphqlCreateMcpServerInput) -> Result<NewMcpServerSetup> {
    let transport_kind = parse_graphql_transport_kind(&input.transport_kind)?;
    match transport_kind {
        McpTransportKind::Stdio => {
            if input.http.is_some() {
                return Err(graphql_error(
                    "invalid MCP setup input: http cannot be set for stdio transport",
                ));
            }
            let stdio = input.stdio.ok_or_else(|| {
                graphql_error("invalid MCP setup input: stdio config is required")
            })?;
            let env = json_string_map(stdio.env, "env")?;
            let secret_env = json_string_map(stdio.secret_env, "secretEnv")?;
            Ok(NewMcpServerSetup {
                display_name: input.display_name,
                transport_kind,
                safe_config: serde_json::json!({
                    "command": stdio.command,
                    "args": stdio.args,
                    "cwd": stdio.cwd,
                    "env": env
                }),
                secrets: McpSecretMaterial {
                    env: secret_env,
                    headers: BTreeMap::new(),
                    oauth_client_credentials: None,
                    oauth_credentials: None,
                },
                browser_oauth_supported: false,
            })
        }
        McpTransportKind::Sse | McpTransportKind::StreamableHttp => {
            if input.stdio.is_some() {
                return Err(graphql_error(
                    "invalid MCP setup input: stdio cannot be set for HTTP transport",
                ));
            }
            let http = input
                .http
                .ok_or_else(|| graphql_error("invalid MCP setup input: http config is required"))?;
            let headers = json_string_map(http.headers, "headers")?;
            let secret_headers = json_string_map(http.secret_headers, "secretHeaders")?;
            let oauth_client_credentials =
                parse_oauth_client_credentials(http.oauth_client_credentials)?;
            Ok(NewMcpServerSetup {
                display_name: input.display_name,
                transport_kind,
                safe_config: serde_json::json!({
                    "url": http.url,
                    "headers": headers
                }),
                secrets: McpSecretMaterial {
                    env: BTreeMap::new(),
                    headers: secret_headers,
                    oauth_client_credentials,
                    oauth_credentials: None,
                },
                browser_oauth_supported: false,
            })
        }
    }
}

fn parse_oauth_client_credentials(
    input: Option<GraphqlMcpOAuthClientCredentialsInput>,
) -> Result<Option<McpOAuthClientCredentials>> {
    let Some(input) = input else {
        return Ok(None);
    };
    let client_id = input.client_id.trim();
    let client_secret = input.client_secret.trim();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(graphql_error(
            "invalid MCP OAuth client credentials: clientId and clientSecret are required",
        ));
    }
    Ok(Some(McpOAuthClientCredentials {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        scopes: input
            .scopes
            .into_iter()
            .map(|scope| scope.trim().to_string())
            .filter(|scope| !scope.is_empty())
            .collect(),
    }))
}

fn parse_graphql_transport_kind(value: &str) -> Result<McpTransportKind> {
    match value {
        "stdio" => Ok(McpTransportKind::Stdio),
        "sse" => Ok(McpTransportKind::Sse),
        "streamable_http" => Ok(McpTransportKind::StreamableHttp),
        _ => Err(graphql_error(
            "invalid transportKind: expected one of stdio, sse, streamable_http",
        )),
    }
}

fn json_string_map(
    value: Option<Json<Value>>,
    field_name: &'static str,
) -> Result<BTreeMap<String, String>> {
    let Some(Json(value)) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value.as_object().ok_or_else(|| {
        graphql_error(format!(
            "invalid MCP {field_name} map: expected object with string values"
        ))
    })?;
    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|string| (key.clone(), string.to_string()))
                .ok_or_else(|| {
                    graphql_error(format!(
                        "invalid MCP {field_name} map: expected object with string values"
                    ))
                })
        })
        .collect()
}

fn parse_graphql_calibration_status(value: &str) -> Result<McpCalibrationStatus> {
    match value {
        "needs_review" => Ok(McpCalibrationStatus::NeedsReview),
        "blocked_unresolved_ownership" => Ok(McpCalibrationStatus::BlockedUnresolvedOwnership),
        "ready" => Ok(McpCalibrationStatus::Ready),
        "disabled" => Ok(McpCalibrationStatus::Disabled),
        _ => Err(graphql_error(
            "invalid status: expected one of needs_review, blocked_unresolved_ownership, ready, disabled",
        )),
    }
}

fn parse_graphql_owner_extractor(input: GraphqlOwnerExtractorInput) -> Result<OwnerExtractor> {
    if input.path.is_empty() {
        return Err(graphql_error(
            "invalid owner extractor path: cannot be empty",
        ));
    }

    Ok(OwnerExtractor {
        source: parse_graphql_owner_extractor_source(&input.source)?,
        selector_kind: parse_graphql_selector_kind(&input.selector_kind)?,
        path: input.path,
    })
}

fn parse_graphql_owner_extractor_source(value: &str) -> Result<OwnerExtractorSource> {
    match value {
        "arguments" => Ok(OwnerExtractorSource::Arguments),
        "structured_content" => Ok(OwnerExtractorSource::StructuredContent),
        "metadata" => Ok(OwnerExtractorSource::Metadata),
        "resource_uri" => Ok(OwnerExtractorSource::ResourceUri),
        "built_in_adapter" => Ok(OwnerExtractorSource::BuiltInAdapter),
        _ => Err(graphql_error(
            "invalid owner extractor source: expected one of arguments, structured_content, metadata, resource_uri, built_in_adapter",
        )),
    }
}

fn parse_graphql_selector_kind(value: &str) -> Result<TrustedIdentitySelectorKind> {
    match value {
        "email" => Ok(TrustedIdentitySelectorKind::Email),
        "phone" => Ok(TrustedIdentitySelectorKind::Phone),
        "domain" => Ok(TrustedIdentitySelectorKind::Domain),
        _ => Err(graphql_error(
            "invalid owner extractor selectorKind: expected one of email, phone, domain",
        )),
    }
}
