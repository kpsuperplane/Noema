use super::{
    McpToolRecord, NewMcpTool, mcp_record_fragment,
    rows::{McpToolRow, mcp_tool_from_row},
};
use crate::store::{NoemaStore, StoreError, ids::now_string};

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
        self.db
            .query(
                r#"
                UPSERT type::record('mcp_tools', $record_id) SET
                  mcp_tool_id = $mcp_tool_id,
                  mcp_server_id = $mcp_server_id,
                  name = $name,
                  description = $description,
                  input_schema = $input_schema,
                  output_schema = $output_schema,
                  annotations = $annotations,
                  metadata_fingerprint = $metadata_fingerprint,
                  discovered_at = $discovered_at,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&tool.mcp_tool_id)))
            .bind(("mcp_tool_id", tool.mcp_tool_id.clone()))
            .bind(("mcp_server_id", tool.mcp_server_id))
            .bind(("name", tool.name))
            .bind(("description", tool.description))
            .bind(("input_schema", tool.input_schema))
            .bind(("output_schema", tool.output_schema))
            .bind(("annotations", tool.annotations))
            .bind(("metadata_fingerprint", tool.metadata_fingerprint))
            .bind(("discovered_at", discovered_at))
            .await?
            .check()?;
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_tool_id, mcp_server_id, name, description, input_schema,
                  output_schema, annotations, metadata_fingerprint, discovered_at
                FROM mcp_tools
                WHERE mcp_tool_id = $mcp_tool_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?;
        let rows: Vec<McpToolRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(mcp_tool_from_row))
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_tool_id, mcp_server_id, name, description, input_schema,
                  output_schema, annotations, metadata_fingerprint, discovered_at
                FROM mcp_tools
                WHERE mcp_server_id = $mcp_server_id;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .await?;
        let rows: Vec<McpToolRow> = response.take(0)?;
        let mut tools: Vec<McpToolRecord> = rows.into_iter().map(mcp_tool_from_row).collect();
        tools.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.mcp_tool_id.cmp(&right.mcp_tool_id))
        });
        Ok(tools)
    }
}
