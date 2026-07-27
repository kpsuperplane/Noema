use std::{collections::BTreeMap, str::FromStr};

use noema_capabilities_mcp::{
    McpControlPlaneServer, McpControlPlaneTool, McpDefinitionRecord, McpInvocationSnapshot,
    McpRepositoryResult, McpServerRecord, McpToolHint, McpToolPolicyRecord, McpToolRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use super::repo_sql_error;
use crate::sqlite::conversion_failure;

pub(super) struct ExistingTool {
    pub(super) mcp_tool_id: String,
    pub(super) metadata_fingerprint: String,
}

pub(super) fn definition_on_connection(
    connection: &Connection,
    mcp_definition_id: &str,
) -> McpRepositoryResult<Option<McpDefinitionRecord>> {
    connection
        .query_row(
            "SELECT mcp_definition_id, display_name, transport_kind, safe_config_json, definition_revision FROM mcp_definitions WHERE mcp_definition_id = ?1 LIMIT 1",
            [mcp_definition_id],
            |row| {
                Ok(McpDefinitionRecord {
                    mcp_definition_id: row.get(0)?,
                    display_name: row.get(1)?,
                    transport_kind: parse_persisted(row.get::<_, String>(2)?, 2)?,
                    safe_config: parse_json(row.get(3)?, 3)?,
                    definition_revision: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(repo_sql_error)
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
                let tool = tool_record_from_row(row, 16)?.ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(16, "mcp_tool_id".to_string(), Type::Null)
                })?;
                let policy = tool_policy_from_row(row, 25)?;
                Ok(McpInvocationSnapshot {
                    server,
                    tool,
                    policy,
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

pub(super) fn tool_policy_on_connection(
    connection: &Connection,
    mcp_tool_id: &str,
) -> McpRepositoryResult<Option<McpToolPolicyRecord>> {
    connection
        .query_row(TOOL_POLICY_BY_TOOL_SQL, params![mcp_tool_id], |row| {
            tool_policy_from_row(row, 0)?.ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(0, "mcp_tool_id".to_string(), Type::Null)
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
    let tool = tool_record_from_row(row, 20)?;
    let policy = tool_policy_from_row(row, 29)?;
    if tool.is_none() && policy.is_some() {
        return Err(rusqlite::Error::InvalidColumnType(
            29,
            "mcp_tool_id".to_string(),
            Type::Text,
        ));
    }
    Ok(JoinedControlPlaneRow {
        server,
        tool: tool.map(|tool| McpControlPlaneTool { tool, policy }),
    })
}

fn server_record_from_row(row: &Row<'_>) -> rusqlite::Result<McpServerRecord> {
    let count = |index| -> rusqlite::Result<usize> {
        usize::try_from(row.get::<_, i64>(index)?)
            .map_err(|error| conversion_failure(index, Type::Integer, error))
    };
    let policy_revision = u64::try_from(row.get::<_, i64>(14)?)
        .map_err(|error| conversion_failure(14, Type::Integer, error))?;
    let mut safe_config = parse_json(row.get(5)?, 5)?;
    let connection_config = parse_json(row.get(6)?, 6)?;
    let Some(safe_object) = safe_config.as_object_mut() else {
        return Err(conversion_failure(
            5,
            Type::Text,
            std::io::Error::other("MCP definition config is not an object"),
        ));
    };
    let Some(connection_object) = connection_config.as_object() else {
        return Err(conversion_failure(
            6,
            Type::Text,
            std::io::Error::other("MCP connection config is not an object"),
        ));
    };
    for (key, value) in connection_object {
        if !value.is_null() {
            safe_object.insert(key.clone(), value.clone());
        }
    }
    Ok(McpServerRecord {
        mcp_server_id: row.get(0)?,
        connection_label: row.get(19)?,
        mcp_definition_id: row.get(1)?,
        definition_revision: row.get(2)?,
        display_name: row.get(3)?,
        transport_kind: parse_persisted(row.get::<_, String>(4)?, 4)?,
        safe_config,
        enabled: row.get::<_, i64>(7)? != 0,
        data_sharing_policy: parse_optional_persisted(row.get(12)?, 12)?,
        unsafe_action_policy: parse_optional_persisted(row.get(13)?, 13)?,
        policy_revision,
        health_status: parse_persisted(row.get::<_, String>(8)?, 8)?,
        auth_status: parse_persisted(row.get::<_, String>(9)?, 9)?,
        tool_count: count(10)?,
        available_tool_count: count(15)?,
        pending_tool_count: count(16)?,
        defaulted_tool_count: count(17)?,
        disabled_tool_count: count(18)?,
        authority_generation: row.get(11)?,
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

fn tool_policy_from_row(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<McpToolPolicyRecord>> {
    let Some(mcp_tool_id) = row.get::<_, Option<String>>(offset)? else {
        return Ok(None);
    };
    let hint = |value_index: usize, source_index: usize| -> rusqlite::Result<McpToolHint> {
        Ok(McpToolHint {
            value: row
                .get::<_, Option<i64>>(value_index)?
                .map(|value| value != 0),
            source: parse_optional_persisted(row.get(source_index)?, source_index)?,
        })
    };
    let policy_revision = u64::try_from(row.get::<_, i64>(offset + 10)?)
        .map_err(|error| conversion_failure(offset + 10, Type::Integer, error))?;
    Ok(Some(McpToolPolicyRecord {
        tool_id: mcp_tool_id,
        read_only: hint(offset + 1, offset + 2)?,
        idempotent: hint(offset + 3, offset + 4)?,
        destructive: hint(offset + 5, offset + 6)?,
        open_world: hint(offset + 7, offset + 8)?,
        status: parse_persisted(row.get(offset + 9)?, offset + 9)?,
        policy_revision,
        source_revision: row.get(offset + 11)?,
    }))
}

fn parse_optional_persisted<T>(value: Option<String>, index: usize) -> rusqlite::Result<Option<T>>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value.map(|value| parse_persisted(value, index)).transpose()
}

fn parse_persisted<T>(value: String, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .parse()
        .map_err(|error| conversion_failure(index, Type::Text, error))
}

fn parse_json(value: String, index: usize) -> rusqlite::Result<serde_json::Value> {
    serde_json::from_str(&value).map_err(|error| conversion_failure(index, Type::Text, error))
}

const SERVER_RECORD_SQL: &str = r#"
SELECT
  m.mcp_server_id, d.mcp_definition_id, d.definition_revision,
  d.display_name, d.transport_kind, d.safe_config_json, m.connection_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''), m.data_sharing_policy, m.unsafe_action_policy,
  m.policy_revision,
  (SELECT COUNT(*) FROM mcp_tools t JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE t.mcp_server_id = m.mcp_server_id AND p.status IN ('ready', 'defaulted') AND p.metadata_fingerprint = t.metadata_fingerprint),
  (SELECT COUNT(*) FROM mcp_tools t JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE t.mcp_server_id = m.mcp_server_id AND p.status = 'pending'),
  (SELECT COUNT(*) FROM mcp_tools t JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE t.mcp_server_id = m.mcp_server_id AND p.status = 'defaulted'),
  (SELECT COUNT(*) FROM mcp_tools t JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE t.mcp_server_id = m.mcp_server_id AND p.status = 'disabled'),
  m.connection_label
FROM mcp_servers m
JOIN mcp_definitions d ON d.mcp_definition_id = m.mcp_definition_id
WHERE m.mcp_server_id = ?1
LIMIT 1
"#;

const CONTROL_PLANE_SERVER_SQL: &str = r#"
SELECT
  m.mcp_server_id, d.mcp_definition_id, d.definition_revision,
  d.display_name, d.transport_kind, d.safe_config_json, m.connection_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''), m.data_sharing_policy, m.unsafe_action_policy,
  m.policy_revision,
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status IN ('ready', 'defaulted') AND p.metadata_fingerprint = counted.metadata_fingerprint),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'pending'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'defaulted'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'disabled'),
  m.connection_label,
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  p.mcp_tool_id, p.read_only, p.read_only_source, p.idempotent, p.idempotent_source,
  p.destructive, p.destructive_source, p.open_world, p.open_world_source,
  p.status, p.policy_revision, p.metadata_fingerprint
FROM mcp_servers m
JOIN mcp_definitions d ON d.mcp_definition_id = m.mcp_definition_id
LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN mcp_tool_policies p ON p.mcp_tool_id = t.mcp_tool_id
WHERE m.mcp_server_id = ?1
ORDER BY t.name, t.mcp_tool_id
"#;

const CONTROL_PLANE_CATALOG_SQL: &str = r#"
SELECT
  m.mcp_server_id, d.mcp_definition_id, d.definition_revision,
  d.display_name, d.transport_kind, d.safe_config_json, m.connection_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''), m.data_sharing_policy, m.unsafe_action_policy,
  m.policy_revision,
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status IN ('ready', 'defaulted') AND p.metadata_fingerprint = counted.metadata_fingerprint),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'pending'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'defaulted'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'disabled'),
  m.connection_label,
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  p.mcp_tool_id, p.read_only, p.read_only_source, p.idempotent, p.idempotent_source,
  p.destructive, p.destructive_source, p.open_world, p.open_world_source,
  p.status, p.policy_revision, p.metadata_fingerprint
FROM mcp_servers m
JOIN mcp_definitions d ON d.mcp_definition_id = m.mcp_definition_id
LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN mcp_tool_policies p ON p.mcp_tool_id = t.mcp_tool_id
ORDER BY d.display_name, d.mcp_definition_id, m.mcp_server_id, t.name, t.mcp_tool_id
"#;

const INVOCATION_SNAPSHOT_SQL: &str = r#"
SELECT
  m.mcp_server_id, d.mcp_definition_id, d.definition_revision,
  d.display_name, d.transport_kind, d.safe_config_json, m.connection_config_json,
  m.enabled, m.health_status, m.auth_status,
  (SELECT COUNT(*) FROM mcp_tools counted WHERE counted.mcp_server_id = m.mcp_server_id),
  COALESCE(m.metadata_fingerprint, ''), m.data_sharing_policy, m.unsafe_action_policy,
  m.policy_revision,
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status IN ('ready', 'defaulted') AND p.metadata_fingerprint = counted.metadata_fingerprint),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'pending'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'defaulted'),
  (SELECT COUNT(*) FROM mcp_tools counted JOIN mcp_tool_policies p USING (mcp_tool_id) WHERE counted.mcp_server_id = m.mcp_server_id AND p.status = 'disabled'),
  m.connection_label,
  t.mcp_tool_id, t.mcp_server_id, t.name, t.description, t.input_schema_json,
  t.output_schema_json, t.annotations_json, t.metadata_fingerprint, t.discovered_at,
  p.mcp_tool_id, p.read_only, p.read_only_source, p.idempotent, p.idempotent_source,
  p.destructive, p.destructive_source, p.open_world, p.open_world_source,
  p.status, p.policy_revision, p.metadata_fingerprint
FROM mcp_servers m
JOIN mcp_definitions d ON d.mcp_definition_id = m.mcp_definition_id
JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
LEFT JOIN mcp_tool_policies p ON p.mcp_tool_id = t.mcp_tool_id
WHERE m.mcp_server_id = ?1 AND t.mcp_tool_id = ?2
LIMIT 1
"#;

const TOOL_POLICY_BY_TOOL_SQL: &str = r#"
SELECT mcp_tool_id, read_only, read_only_source, idempotent, idempotent_source,
  destructive, destructive_source, open_world, open_world_source, status,
  policy_revision, metadata_fingerprint
FROM mcp_tool_policies
WHERE mcp_tool_id = ?1
LIMIT 1
"#;
