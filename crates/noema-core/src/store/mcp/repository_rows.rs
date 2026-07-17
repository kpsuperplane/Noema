use std::{collections::BTreeMap, str::FromStr};

use noema_capabilities_mcp::{
    McpCalibrationStatus, McpControlPlaneServer, McpControlPlaneTool, McpInvocationSnapshot,
    McpRepositoryResult, McpServerRecord, McpToolRecord, McpTrustClassification,
    ToolCalibrationRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use super::repo_sql_error;

pub(super) struct ExistingTool {
    pub(super) mcp_tool_id: String,
    pub(super) metadata_fingerprint: String,
}

struct JoinedControlPlaneRow {
    server: McpServerRecord,
    tool: Option<McpControlPlaneTool>,
}

pub(super) fn server_record_on_connection(
    connection: &Connection,
    mcp_server_id: &str,
) -> McpRepositoryResult<Option<McpServerRecord>> {
    connection
        .query_row(
            SERVER_RECORD_SQL,
            params![mcp_server_id],
            server_record_from_row,
        )
        .optional()
        .map_err(repo_sql_error)
}

pub(super) fn control_plane_server_on_connection(
    connection: &Connection,
    mcp_server_id: &str,
) -> McpRepositoryResult<Option<McpControlPlaneServer>> {
    let mut statement = connection
        .prepare(CONTROL_PLANE_SERVER_SQL)
        .map_err(repo_sql_error)?;
    let rows = statement
        .query_map(params![mcp_server_id], joined_control_plane_row)
        .map_err(repo_sql_error)?;
    let mut catalog = collect_control_plane_rows(rows)?;
    Ok(catalog.pop())
}

pub(super) fn control_plane_catalog_on_connection(
    connection: &Connection,
) -> McpRepositoryResult<Vec<McpControlPlaneServer>> {
    let mut statement = connection
        .prepare(CONTROL_PLANE_CATALOG_SQL)
        .map_err(repo_sql_error)?;
    let rows = statement
        .query_map([], joined_control_plane_row)
        .map_err(repo_sql_error)?;
    collect_control_plane_rows(rows)
}

pub(super) fn invocation_snapshot_on_connection(
    connection: &Connection,
    mcp_server_id: &str,
    mcp_tool_id: &str,
) -> McpRepositoryResult<Option<McpInvocationSnapshot>> {
    connection
        .query_row(
            INVOCATION_SNAPSHOT_SQL,
            params![mcp_server_id, mcp_tool_id],
            |row| {
                let server = server_record_from_row(row)?;
                let tool = tool_record_from_row(row, 9)?.ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(9, "mcp_tool_id".to_string(), Type::Null)
                })?;
                let calibration = calibration_record_from_row(row, 18)?;
                Ok(McpInvocationSnapshot {
                    server,
                    tool,
                    calibration,
                })
            },
        )
        .optional()
        .map_err(repo_sql_error)
}

pub(super) fn existing_tools_by_name_on_connection(
    connection: &Connection,
    mcp_server_id: &str,
) -> McpRepositoryResult<BTreeMap<String, ExistingTool>> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT mcp_tool_id, name, metadata_fingerprint
            FROM mcp_tools
            WHERE mcp_server_id = ?1
            ORDER BY name, mcp_tool_id
            "#,
        )
        .map_err(repo_sql_error)?;
    let rows = statement
        .query_map(params![mcp_server_id], |row| {
            Ok((
                row.get::<_, String>(1)?,
                ExistingTool {
                    mcp_tool_id: row.get(0)?,
                    metadata_fingerprint: row.get(2)?,
                },
            ))
        })
        .map_err(repo_sql_error)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(repo_sql_error)
}

pub(super) fn calibration_on_connection(
    connection: &Connection,
    column: &'static str,
    id: &str,
) -> McpRepositoryResult<Option<ToolCalibrationRecord>> {
    let sql = match column {
        "calibration_id" => CALIBRATION_BY_ID_SQL,
        "mcp_tool_id" => CALIBRATION_BY_TOOL_SQL,
        _ => return Err(super::invariant_error()),
    };
    connection
        .query_row(sql, params![id], |row| {
            calibration_record_from_row(row, 0)?.ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(0, "calibration_id".to_string(), Type::Null)
            })
        })
        .optional()
        .map_err(repo_sql_error)
}

fn collect_control_plane_rows(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<JoinedControlPlaneRow>>,
) -> McpRepositoryResult<Vec<McpControlPlaneServer>> {
    let mut catalog: Vec<McpControlPlaneServer> = Vec::new();
    for row in rows {
        let row = row.map_err(repo_sql_error)?;
        if let Some(server) = catalog
            .last_mut()
            .filter(|server| server.server.mcp_server_id == row.server.mcp_server_id)
        {
            if let Some(tool) = row.tool {
                server.tools.push(tool);
            }
            continue;
        }
        let mut tools = Vec::new();
        if let Some(tool) = row.tool {
            tools.push(tool);
        }
        catalog.push(McpControlPlaneServer {
            server: row.server,
            tools,
        });
    }
    Ok(catalog)
}

fn joined_control_plane_row(row: &Row<'_>) -> rusqlite::Result<JoinedControlPlaneRow> {
    let server = server_record_from_row(row)?;
    let tool = tool_record_from_row(row, 9)?;
    let calibration = calibration_record_from_row(row, 18)?;
    if tool.is_none() && calibration.is_some() {
        return Err(rusqlite::Error::InvalidColumnType(
            18,
            "calibration_id".to_string(),
            Type::Text,
        ));
    }
    Ok(JoinedControlPlaneRow {
        server,
        tool: tool.map(|tool| McpControlPlaneTool { tool, calibration }),
    })
}

fn server_record_from_row(row: &Row<'_>) -> rusqlite::Result<McpServerRecord> {
    let tool_count = row.get::<_, i64>(7)?;
    Ok(McpServerRecord {
        mcp_server_id: row.get(0)?,
        display_name: row.get(1)?,
        transport_kind: parse_persisted(row.get::<_, String>(2)?, 2)?,
        safe_config: parse_json(row.get(3)?, 3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        health_status: parse_persisted(row.get::<_, String>(5)?, 5)?,
        auth_status: parse_persisted(row.get::<_, String>(6)?, 6)?,
        tool_count: usize::try_from(tool_count).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(7, Type::Integer, Box::new(error))
        })?,
        authority_generation: row.get(8)?,
    })
}

fn tool_record_from_row(row: &Row<'_>, offset: usize) -> rusqlite::Result<Option<McpToolRecord>> {
    let Some(mcp_tool_id) = row.get::<_, Option<String>>(offset)? else {
        return Ok(None);
    };
    let output_schema = row
        .get::<_, Option<String>>(offset + 5)?
        .map(|value| parse_json(value, offset + 5))
        .transpose()?;
    Ok(Some(McpToolRecord {
        mcp_tool_id,
        mcp_server_id: row.get(offset + 1)?,
        name: row.get(offset + 2)?,
        description: row.get(offset + 3)?,
        input_schema: parse_json(row.get(offset + 4)?, offset + 4)?,
        output_schema,
        annotations: parse_json(row.get(offset + 6)?, offset + 6)?,
        metadata_fingerprint: row.get(offset + 7)?,
        discovered_at: row.get(offset + 8)?,
    }))
}

fn calibration_record_from_row(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<ToolCalibrationRecord>> {
    let Some(calibration_id) = row.get::<_, Option<String>>(offset)? else {
        return Ok(None);
    };
    let read_classification = parse_persisted(row.get(offset + 2)?, offset + 2)?;
    let write_classification = parse_persisted(row.get(offset + 3)?, offset + 3)?;
    let export_classification = parse_persisted(row.get(offset + 4)?, offset + 4)?;
    let mut status = parse_persisted(row.get(offset + 5)?, offset + 5)?;
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
    Ok(Some(ToolCalibrationRecord {
        calibration_id,
        mcp_tool_id: row.get(offset + 1)?,
        read_classification,
        write_classification,
        export_classification,
        status,
        reviewed_by: row.get(offset + 6)?,
        reviewed_metadata_fingerprint: row.get(offset + 7)?,
    }))
}

fn parse_persisted<T>(value: String, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn parse_json(value: String, index: usize) -> rusqlite::Result<serde_json::Value> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

const SERVER_RECORD_SQL: &str = r#"
SELECT
  m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, '')
FROM mcp_servers m
WHERE m.mcp_server_id = ?1
LIMIT 1
"#;

const CONTROL_PLANE_SERVER_SQL: &str = r#"
SELECT
  m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''),
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  c.calibration_id, c.mcp_tool_id, c.read_classification, c.write_classification,
  c.export_classification, c.status, c.reviewed_by, c.reviewed_metadata_fingerprint
FROM mcp_servers m
LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN tool_calibrations c ON c.mcp_tool_id = t.mcp_tool_id
WHERE m.mcp_server_id = ?1
ORDER BY t.name, t.mcp_tool_id
"#;

const CONTROL_PLANE_CATALOG_SQL: &str = r#"
SELECT
  m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''),
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  c.calibration_id, c.mcp_tool_id, c.read_classification, c.write_classification,
  c.export_classification, c.status, c.reviewed_by, c.reviewed_metadata_fingerprint
FROM mcp_servers m
LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN tool_calibrations c ON c.mcp_tool_id = t.mcp_tool_id
ORDER BY m.display_name, m.mcp_server_id, t.name, t.mcp_tool_id
"#;

const INVOCATION_SNAPSHOT_SQL: &str = r#"
SELECT
  m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''),
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  c.calibration_id, c.mcp_tool_id, c.read_classification, c.write_classification,
  c.export_classification, c.status, c.reviewed_by, c.reviewed_metadata_fingerprint
FROM mcp_servers m
JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN tool_calibrations c ON c.mcp_tool_id = t.mcp_tool_id
WHERE m.mcp_server_id = ?1 AND t.mcp_tool_id = ?2
LIMIT 1
"#;

const CALIBRATION_BY_ID_SQL: &str = r#"
SELECT calibration_id, mcp_tool_id, read_classification, write_classification,
  export_classification, status, reviewed_by, reviewed_metadata_fingerprint
FROM tool_calibrations
WHERE calibration_id = ?1
LIMIT 1
"#;

const CALIBRATION_BY_TOOL_SQL: &str = r#"
SELECT calibration_id, mcp_tool_id, read_classification, write_classification,
  export_classification, status, reviewed_by, reviewed_metadata_fingerprint
FROM tool_calibrations
WHERE mcp_tool_id = ?1
LIMIT 1
"#;
