use rusqlite::Row;

use super::{
    McpApprovalRequestRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, ToolCalibrationRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};
use crate::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, TrustedIdentitySelectorKind,
    store::{
        StoreError,
        ids::{Timestamp, invalid_enum},
    },
};

pub(super) fn bool_from_i64(value: i64) -> bool {
    value != 0
}

pub(super) fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(super) fn mcp_server_from_row(row: &Row<'_>) -> rusqlite::Result<McpServerRecord> {
    let safe_config_json: String = row.get(3)?;
    let transport_kind: String = row.get(2)?;
    let health_status: String = row.get(5)?;
    let auth_status: String = row.get(6)?;
    let tool_count: i64 = row.get(7)?;
    Ok(McpServerRecord {
        mcp_server_id: row.get(0)?,
        display_name: row.get(1)?,
        transport_kind: parse_mcp_transport_kind(&transport_kind).map_err(to_sql_error)?,
        safe_config: serde_json::from_str(&safe_config_json).map_err(to_sql_error)?,
        enabled: bool_from_i64(row.get(4)?),
        health_status: parse_mcp_health_status(&health_status).map_err(to_sql_error)?,
        auth_status: parse_mcp_auth_status(&auth_status).map_err(to_sql_error)?,
        tool_count: usize::try_from(tool_count).map_err(to_sql_error)?,
    })
}

pub(super) fn mcp_tool_from_row(row: &Row<'_>) -> rusqlite::Result<McpToolRecord> {
    let input_schema_json: String = row.get(4)?;
    let output_schema_json: Option<String> = row.get(5)?;
    let annotations_json: String = row.get(6)?;
    let discovered_at: String = row.get(8)?;
    Timestamp::parse(&discovered_at).map_err(to_sql_error)?;
    Ok(McpToolRecord {
        mcp_tool_id: row.get(0)?,
        mcp_server_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        input_schema: serde_json::from_str(&input_schema_json).map_err(to_sql_error)?,
        output_schema: output_schema_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(to_sql_error)?,
        annotations: serde_json::from_str(&annotations_json).map_err(to_sql_error)?,
        metadata_fingerprint: row.get(7)?,
        discovered_at,
    })
}

pub(super) fn tool_calibration_from_row(row: &Row<'_>) -> rusqlite::Result<ToolCalibrationRecord> {
    let read_classification: String = row.get(2)?;
    let write_classification: String = row.get(3)?;
    let export_classification: String = row.get(4)?;
    let owner_extractors_json: String = row.get(5)?;
    let status: String = row.get(6)?;
    Ok(ToolCalibrationRecord {
        calibration_id: row.get(0)?,
        mcp_tool_id: row.get(1)?,
        read_classification: parse_mcp_trust_classification(&read_classification)
            .map_err(to_sql_error)?,
        write_classification: parse_mcp_trust_classification(&write_classification)
            .map_err(to_sql_error)?,
        export_classification: parse_mcp_trust_classification(&export_classification)
            .map_err(to_sql_error)?,
        owner_extractors: serde_json::from_str(&owner_extractors_json).map_err(to_sql_error)?,
        status: parse_mcp_calibration_status(&status).map_err(to_sql_error)?,
        reviewed_by: row.get(7)?,
        reviewed_metadata_fingerprint: row.get(8)?,
    })
}

pub(super) fn trusted_identity_selector_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<TrustedIdentitySelectorRecord> {
    let selector_kind: String = row.get(2)?;
    let effect: String = row.get(4)?;
    Ok(TrustedIdentitySelectorRecord {
        selector_id: row.get(0)?,
        owner_scope_id: row.get(1)?,
        selector_kind: parse_trusted_identity_selector_kind(&selector_kind)
            .map_err(to_sql_error)?,
        normalized_value: row.get(3)?,
        effect: parse_trusted_identity_selector_effect(&effect).map_err(to_sql_error)?,
        issuer_actor_id: row.get(5)?,
        revoked_at: validated_optional_timestamp(row.get(6)?)?,
    })
}

pub(super) fn mcp_approval_request_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<McpApprovalRequestRecord> {
    let payload_preview_json: String = row.get(15)?;
    Ok(McpApprovalRequestRecord {
        approval_id: row.get(0)?,
        action_summary: row.get(1)?,
        tool_invocation_id: row.get(2)?,
        mcp_server_id: row.get(3)?,
        mcp_tool_id: row.get(4)?,
        requester_actor_id: row.get(5)?,
        owner_scope_id: row.get(6)?,
        active_scope_id: row.get(7)?,
        destination_summary: row.get(8)?,
        data_source_summary: row.get(9)?,
        source_owner_identity: row.get(10)?,
        source_owner_trust: row.get(11)?,
        destination_owner_identity: row.get(12)?,
        destination_owner_trust: row.get(13)?,
        export_summary: row.get(14)?,
        payload_preview: serde_json::from_str(&payload_preview_json).map_err(to_sql_error)?,
        status: row.get(16)?,
        decision_actor_id: row.get(17)?,
        decision_comment: row.get(18)?,
        decided_at: validated_optional_timestamp(row.get(19)?)?,
    })
}

fn validated_optional_timestamp(value: Option<String>) -> rusqlite::Result<Option<String>> {
    value
        .map(|timestamp| {
            Timestamp::parse(&timestamp)
                .map(|_| timestamp)
                .map_err(to_sql_error)
        })
        .transpose()
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

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
