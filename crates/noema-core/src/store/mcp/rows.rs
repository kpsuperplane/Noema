use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use super::{
    McpApprovalRequestRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};
use crate::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, TrustedIdentitySelectorKind,
    store::{StoreError, ids::invalid_enum},
};

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct McpServerRow {
    mcp_server_id: String,
    display_name: String,
    transport_kind: String,
    safe_config: Value,
    enabled: bool,
    health_status: String,
    auth_status: String,
    tool_count: usize,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct McpToolRow {
    mcp_tool_id: String,
    mcp_server_id: String,
    name: String,
    description: Option<String>,
    input_schema: Value,
    output_schema: Option<Value>,
    annotations: Value,
    metadata_fingerprint: String,
    discovered_at: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct ToolCalibrationRow {
    calibration_id: String,
    mcp_tool_id: String,
    read_classification: String,
    write_classification: String,
    export_classification: String,
    owner_extractors: Value,
    status: String,
    reviewed_by: Option<String>,
    reviewed_metadata_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct TrustedIdentitySelectorRow {
    selector_id: String,
    owner_scope_id: String,
    selector_kind: String,
    normalized_value: String,
    effect: String,
    issuer_actor_id: String,
    revoked_at: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub(super) struct McpApprovalRequestRow {
    approval_id: String,
    action_summary: String,
    tool_invocation_id: String,
    mcp_server_id: Option<String>,
    mcp_tool_id: Option<String>,
    requester_actor_id: String,
    owner_scope_id: String,
    active_scope_id: String,
    destination_summary: String,
    data_source_summary: String,
    source_owner_identity: String,
    source_owner_trust: String,
    destination_owner_identity: String,
    destination_owner_trust: String,
    export_summary: String,
    payload_preview: Value,
    status: String,
    decision_actor_id: Option<String>,
    decision_comment: Option<String>,
    decided_at: Option<String>,
}

pub(super) fn mcp_server_from_row(row: McpServerRow) -> Result<McpServerRecord, StoreError> {
    Ok(McpServerRecord {
        mcp_server_id: row.mcp_server_id,
        display_name: row.display_name,
        transport_kind: parse_mcp_transport_kind(&row.transport_kind)?,
        safe_config: row.safe_config,
        enabled: row.enabled,
        health_status: parse_mcp_health_status(&row.health_status)?,
        auth_status: parse_mcp_auth_status(&row.auth_status)?,
        tool_count: row.tool_count,
    })
}

pub(super) fn mcp_tool_from_row(row: McpToolRow) -> McpToolRecord {
    McpToolRecord {
        mcp_tool_id: row.mcp_tool_id,
        mcp_server_id: row.mcp_server_id,
        name: row.name,
        description: row.description,
        input_schema: row.input_schema,
        output_schema: row.output_schema,
        annotations: row.annotations,
        metadata_fingerprint: row.metadata_fingerprint,
        discovered_at: row.discovered_at,
    }
}

pub(super) fn tool_calibration_from_row(
    row: ToolCalibrationRow,
) -> Result<ToolCalibrationRecord, StoreError> {
    let owner_extractors = serde_json::from_value(row.owner_extractors).map_err(|error| {
        StoreError::Schema(format!(
            "invalid owner extractors in embedded store: {error}"
        ))
    })?;
    Ok(ToolCalibrationRecord {
        calibration_id: row.calibration_id,
        mcp_tool_id: row.mcp_tool_id,
        read_classification: parse_mcp_trust_classification(&row.read_classification)?,
        write_classification: parse_mcp_trust_classification(&row.write_classification)?,
        export_classification: parse_mcp_trust_classification(&row.export_classification)?,
        owner_extractors,
        status: parse_mcp_calibration_status(&row.status)?,
        reviewed_by: row.reviewed_by,
        reviewed_metadata_fingerprint: row.reviewed_metadata_fingerprint,
    })
}

pub(super) fn trusted_identity_selector_from_row(
    row: TrustedIdentitySelectorRow,
) -> Result<TrustedIdentitySelectorRecord, StoreError> {
    Ok(TrustedIdentitySelectorRecord {
        selector_id: row.selector_id,
        owner_scope_id: row.owner_scope_id,
        selector_kind: parse_trusted_identity_selector_kind(&row.selector_kind)?,
        normalized_value: row.normalized_value,
        effect: parse_trusted_identity_selector_effect(&row.effect)?,
        issuer_actor_id: row.issuer_actor_id,
        revoked_at: row.revoked_at,
    })
}

pub(super) fn mcp_approval_request_from_row(
    row: McpApprovalRequestRow,
) -> McpApprovalRequestRecord {
    McpApprovalRequestRecord {
        approval_id: row.approval_id,
        action_summary: row.action_summary,
        tool_invocation_id: row.tool_invocation_id,
        mcp_server_id: row.mcp_server_id,
        mcp_tool_id: row.mcp_tool_id,
        requester_actor_id: row.requester_actor_id,
        owner_scope_id: row.owner_scope_id,
        active_scope_id: row.active_scope_id,
        destination_summary: row.destination_summary,
        data_source_summary: row.data_source_summary,
        source_owner_identity: row.source_owner_identity,
        source_owner_trust: row.source_owner_trust,
        destination_owner_identity: row.destination_owner_identity,
        destination_owner_trust: row.destination_owner_trust,
        export_summary: row.export_summary,
        payload_preview: row.payload_preview,
        status: row.status,
        decision_actor_id: row.decision_actor_id,
        decision_comment: row.decision_comment,
        decided_at: row.decided_at,
    }
}

fn parse_mcp_transport_kind(value: &str) -> Result<McpTransportKind, StoreError> {
    match value {
        "stdio" => Ok(McpTransportKind::Stdio),
        "sse" => Ok(McpTransportKind::Sse),
        "streamable_http" => Ok(McpTransportKind::StreamableHttp),
        _ => invalid_enum("mcp_transport_kind", value),
    }
}

fn parse_mcp_health_status(value: &str) -> Result<McpServerHealthStatus, StoreError> {
    match value {
        "unknown" => Ok(McpServerHealthStatus::Unknown),
        "healthy" => Ok(McpServerHealthStatus::Healthy),
        "unavailable" => Ok(McpServerHealthStatus::Unavailable),
        _ => invalid_enum("mcp_server_health_status", value),
    }
}

fn parse_mcp_auth_status(value: &str) -> Result<McpServerAuthStatus, StoreError> {
    match value {
        "none" => Ok(McpServerAuthStatus::None),
        "needs_auth" => Ok(McpServerAuthStatus::NeedsAuth),
        "authenticated" => Ok(McpServerAuthStatus::Authenticated),
        "unavailable" => Ok(McpServerAuthStatus::Unavailable),
        _ => invalid_enum("mcp_server_auth_status", value),
    }
}

fn parse_mcp_trust_classification(value: &str) -> Result<McpTrustClassification, StoreError> {
    match value {
        "none" => Ok(McpTrustClassification::None),
        "trusted" => Ok(McpTrustClassification::Trusted),
        "untrusted" => Ok(McpTrustClassification::Untrusted),
        "mixed" => Ok(McpTrustClassification::Mixed),
        _ => invalid_enum("mcp_trust_classification", value),
    }
}

fn parse_mcp_calibration_status(value: &str) -> Result<McpCalibrationStatus, StoreError> {
    match value {
        "needs_review" => Ok(McpCalibrationStatus::NeedsReview),
        "blocked_unresolved_ownership" => Ok(McpCalibrationStatus::BlockedUnresolvedOwnership),
        "ready" => Ok(McpCalibrationStatus::Ready),
        "disabled" => Ok(McpCalibrationStatus::Disabled),
        _ => invalid_enum("mcp_calibration_status", value),
    }
}

fn parse_trusted_identity_selector_kind(
    value: &str,
) -> Result<TrustedIdentitySelectorKind, StoreError> {
    match value {
        "email" => Ok(TrustedIdentitySelectorKind::Email),
        "phone" => Ok(TrustedIdentitySelectorKind::Phone),
        "domain" => Ok(TrustedIdentitySelectorKind::Domain),
        _ => invalid_enum("trusted_identity_selector_kind", value),
    }
}

fn parse_trusted_identity_selector_effect(
    value: &str,
) -> Result<TrustedIdentitySelectorEffect, StoreError> {
    match value {
        "trust" => Ok(TrustedIdentitySelectorEffect::Trust),
        "restrict" => Ok(TrustedIdentitySelectorEffect::Restrict),
        _ => invalid_enum("trusted_identity_selector_effect", value),
    }
}
