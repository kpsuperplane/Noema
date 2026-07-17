use std::str::FromStr;

use rusqlite::Row;

use noema_capabilities_mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};

use super::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord,
    ToolCalibrationRecord,
};
use crate::StoreError;

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
        authority_generation: row.get(8)?,
    })
}

pub(super) fn mcp_tool_from_row(row: &Row<'_>) -> rusqlite::Result<McpToolRecord> {
    let input_schema_json: String = row.get(4)?;
    let output_schema_json: Option<String> = row.get(5)?;
    let annotations_json: String = row.get(6)?;
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
        discovered_at: row.get(8)?,
    })
}

pub(super) fn tool_calibration_from_row(row: &Row<'_>) -> rusqlite::Result<ToolCalibrationRecord> {
    let read_classification =
        parse_mcp_trust_classification(&row.get::<_, String>(2)?).map_err(to_sql_error)?;
    let write_classification =
        parse_mcp_trust_classification(&row.get::<_, String>(3)?).map_err(to_sql_error)?;
    let export_classification =
        parse_mcp_trust_classification(&row.get::<_, String>(4)?).map_err(to_sql_error)?;
    let mut status =
        parse_mcp_calibration_status(&row.get::<_, String>(5)?).map_err(to_sql_error)?;
    if status == McpCalibrationStatus::Ready
        && [
            read_classification,
            write_classification,
            export_classification,
        ]
        .contains(&McpTrustClassification::Mixed)
    {
        status = McpCalibrationStatus::BlockedUnresolvedOwnership;
    }
    Ok(ToolCalibrationRecord {
        calibration_id: row.get(0)?,
        mcp_tool_id: row.get(1)?,
        read_classification,
        write_classification,
        export_classification,
        status,
        reviewed_by: row.get(6)?,
        reviewed_metadata_fingerprint: row.get(7)?,
    })
}

fn parse_mcp_transport_kind(value: &str) -> Result<McpTransportKind, StoreError> {
    parse_persisted_enum("mcp_transport_kind", value)
}

fn parse_mcp_health_status(value: &str) -> Result<McpServerHealthStatus, StoreError> {
    parse_persisted_enum("mcp_server_health_status", value)
}

fn parse_mcp_auth_status(value: &str) -> Result<McpServerAuthStatus, StoreError> {
    parse_persisted_enum("mcp_server_auth_status", value)
}

fn parse_mcp_trust_classification(value: &str) -> Result<McpTrustClassification, StoreError> {
    parse_persisted_enum("mcp_trust_classification", value)
}

fn parse_mcp_calibration_status(value: &str) -> Result<McpCalibrationStatus, StoreError> {
    parse_persisted_enum("mcp_calibration_status", value)
}

fn parse_persisted_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError>
where
    T: FromStr,
{
    value.parse().map_err(|_| StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
