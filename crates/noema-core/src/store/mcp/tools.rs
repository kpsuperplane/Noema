use rusqlite::{OptionalExtension, params};

use super::{McpToolRecord, NewMcpTool, rows::mcp_tool_from_row};
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
        let previous = self.get_mcp_tool(&tool.mcp_tool_id).await?;
        let metadata_changed = previous
            .as_ref()
            .is_some_and(|existing| existing.metadata_fingerprint != tool.metadata_fingerprint);
        let input_schema_json = json_to_string(&tool.input_schema)?;
        let output_schema_json = tool
            .output_schema
            .as_ref()
            .map(json_to_string)
            .transpose()?;
        let annotations_json = json_to_string(&tool.annotations)?;

        self.with_connection(|conn| {
            conn.execute(
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
                    tool.mcp_tool_id,
                    tool.mcp_server_id,
                    tool.name,
                    tool.description,
                    input_schema_json,
                    output_schema_json,
                    annotations_json,
                    tool.metadata_fingerprint,
                    discovered_at,
                ],
            )?;
            Ok(())
        })
        .await?;
        if metadata_changed {
            self.invalidate_tool_calibration_review(&tool.mcp_tool_id)
                .await?;
        }
        self.get_mcp_tool(&tool.mcp_tool_id).await?.ok_or_else(|| {
            StoreError::Schema(format!(
                "missing MCP tool after upsert: {}",
                tool.mcp_tool_id
            ))
        })
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
