use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    McpToolRecord, NewMcpTool,
    calibrations::{
        invalidate_tool_calibration_review_on_connection,
        update_mcp_server_enabled_from_calibrations_on_connection,
    },
    rows::mcp_tool_from_row,
};
use crate::store::{
    NoemaStore, StoreError,
    ids::now_string,
    sqlite::{json_to_string, now_timestamp_sql},
};

impl NoemaStore {
    /// Create or refresh one discovered MCP tool metadata row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn upsert_discovered_mcp_tool(
        &self,
        tool: NewMcpTool,
    ) -> Result<McpToolRecord, StoreError> {
        let discovered_at = now_string();
        let input_schema_json = json_to_string(&tool.input_schema)?;
        let output_schema_json = tool
            .output_schema
            .as_ref()
            .map(json_to_string)
            .transpose()?;
        let annotations_json = json_to_string(&tool.annotations)?;

        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let previous = transaction
                .query_row(
                    "SELECT mcp_server_id, metadata_fingerprint FROM mcp_tools WHERE mcp_tool_id = ?1 LIMIT 1",
                    params![&tool.mcp_tool_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let metadata_changed = previous
                .as_ref()
                .is_some_and(|(_, fingerprint)| fingerprint != &tool.metadata_fingerprint);
            let server_changed = previous
                .as_ref()
                .is_some_and(|(server_id, _)| server_id != &tool.mcp_server_id);
            transaction.execute(
                format!(
                    r#"
                    INSERT INTO mcp_tools (
                      mcp_tool_id, mcp_server_id, name, description,
                      input_schema_json, output_schema_json, annotations_json,
                      metadata_fingerprint, discovered_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, {})
                    ON CONFLICT(mcp_tool_id) DO UPDATE SET
                      mcp_server_id = excluded.mcp_server_id,
                      name = excluded.name,
                      description = excluded.description,
                      input_schema_json = excluded.input_schema_json,
                      output_schema_json = excluded.output_schema_json,
                      annotations_json = excluded.annotations_json,
                      metadata_fingerprint = excluded.metadata_fingerprint,
                      discovered_at = excluded.discovered_at,
                      updated_at = excluded.updated_at
                    "#,
                    now_timestamp_sql()
                )
                .as_str(),
                params![
                    &tool.mcp_tool_id,
                    &tool.mcp_server_id,
                    &tool.name,
                    &tool.description,
                    input_schema_json,
                    output_schema_json,
                    annotations_json,
                    &tool.metadata_fingerprint,
                    discovered_at,
                ],
            )?;
            if metadata_changed {
                invalidate_tool_calibration_review_on_connection(&transaction, &tool.mcp_tool_id)?;
            }
            if metadata_changed || server_changed {
                if let Some((previous_server_id, _)) = previous.as_ref()
                    && previous_server_id != &tool.mcp_server_id
                {
                    update_mcp_server_enabled_from_calibrations_on_connection(
                        &transaction,
                        previous_server_id,
                    )?;
                }
                update_mcp_server_enabled_from_calibrations_on_connection(
                    &transaction,
                    &tool.mcp_server_id,
                )?;
            }
            let saved = transaction
                .query_row(
                    MCP_TOOL_SELECT_BY_ID,
                    params![&tool.mcp_tool_id],
                    mcp_tool_from_row,
                )
                .optional()?
                .ok_or_else(|| {
                    StoreError::Schema(format!(
                        "missing MCP tool after upsert: {}",
                        tool.mcp_tool_id
                    ))
                })?;
            transaction.commit()?;
            Ok(saved)
        })
        .await
    }

    /// Return one MCP tool by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_mcp_tool(
        &self,
        mcp_tool_id: &str,
    ) -> Result<Option<McpToolRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                MCP_TOOL_SELECT_BY_ID,
                params![mcp_tool_id],
                mcp_tool_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List discovered MCP tools for one server in deterministic display order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn list_mcp_tools_for_server(
        &self,
        mcp_server_id: &str,
    ) -> Result<Vec<McpToolRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT mcp_tool_id, mcp_server_id, name, description, input_schema_json,
                  output_schema_json, annotations_json, metadata_fingerprint, discovered_at
                FROM mcp_tools
                WHERE mcp_server_id = ?1
                ORDER BY name, mcp_tool_id
                "#,
            )?;
            let rows = statement.query_map(params![mcp_server_id], mcp_tool_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
}

const MCP_TOOL_SELECT_BY_ID: &str = r#"
SELECT mcp_tool_id, mcp_server_id, name, description, input_schema_json,
  output_schema_json, annotations_json, metadata_fingerprint, discovered_at
FROM mcp_tools
WHERE mcp_tool_id = ?1
LIMIT 1
"#;
