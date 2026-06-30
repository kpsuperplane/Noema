use async_graphql::{InputObject, Json, Result, SimpleObject};
use serde_json::Value;

use crate::{
    McpApprovalRequestRecord, McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus,
    McpServerRecord, McpTrustClassification, NewToolCalibration, OwnerExtractor,
    OwnerExtractorSource, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorKind, TrustedIdentitySelectorRecord,
};

use super::{errors::graphql_error, schema::GraphqlState};

/// MCP server metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
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
        }
    }
}

/// Save reviewed MCP tool calibration.
#[derive(Clone, Debug, InputObject)]
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
    /// Agents allowed to see/use this calibration.
    pub enabled_agent_ids: Vec<String>,
    /// Governable scopes where this calibration is enabled.
    pub enabled_scope_ids: Vec<String>,
    /// Review/gateway readiness status.
    pub status: String,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Deterministic owner extractor input for MCP ownership resolution.
#[derive(Clone, Debug, InputObject)]
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
pub struct GraphqlMcpApprovalRequest {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id, when available.
    pub tool_invocation_id: Option<String>,
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

pub(super) async fn mcp_servers(state: &GraphqlState) -> Result<Vec<GraphqlMcpServer>> {
    let store = state.store()?;
    let servers = store.list_mcp_servers().await.map_err(graphql_error)?;
    Ok(servers.into_iter().map(Into::into).collect())
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

    let store = state.store()?;
    let calibration = store
        .save_tool_calibration(NewToolCalibration {
            calibration_id: input.calibration_id,
            mcp_tool_id: input.mcp_tool_id,
            read_classification,
            write_classification,
            export_classification,
            owner_extractors,
            enabled_agent_ids: input.enabled_agent_ids,
            enabled_scope_ids: input.enabled_scope_ids,
            status,
            reviewed_by: input.reviewed_by,
            reviewed_metadata_fingerprint: input.reviewed_metadata_fingerprint,
        })
        .await
        .map_err(graphql_error)?;
    Ok(calibration.into())
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
