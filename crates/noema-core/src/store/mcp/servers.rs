use super::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, NewMcpServer, mcp_record_fragment,
    rows::{McpServerRow, mcp_server_from_row},
};
use crate::store::{NoemaStore, StoreError};

impl NoemaStore {
    /// Create one MCP server metadata row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_mcp_server(
        &self,
        server: NewMcpServer,
    ) -> Result<McpServerRecord, StoreError> {
        self.db
            .query(
                r#"
                CREATE type::record('mcp_servers', $record_id) SET
                  mcp_server_id = $mcp_server_id,
                  display_name = $display_name,
                  transport_kind = $transport_kind,
                  safe_config = $safe_config,
                  auth_status = 'none',
                  health_status = 'unknown',
                  enabled = false,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&server.mcp_server_id)))
            .bind(("mcp_server_id", server.mcp_server_id.clone()))
            .bind(("display_name", server.display_name))
            .bind(("transport_kind", server.transport_kind.as_str().to_string()))
            .bind(("safe_config", server.safe_config))
            .await?
            .check()?;
        self.get_mcp_server(&server.mcp_server_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing MCP server after create: {}",
                    server.mcp_server_id
                ))
            })
    }

    /// Return one MCP server by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_mcp_server(
        &self,
        mcp_server_id: &str,
    ) -> Result<Option<McpServerRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_server_id, display_name, transport_kind, safe_config,
                  enabled, health_status, auth_status,
                  count((SELECT VALUE id FROM mcp_tools WHERE mcp_server_id = $mcp_server_id)) AS tool_count
                FROM mcp_servers
                WHERE mcp_server_id = $mcp_server_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .await?;
        let rows: Vec<McpServerRow> = response.take(0)?;
        rows.into_iter().next().map(mcp_server_from_row).transpose()
    }

    /// List MCP servers in deterministic Settings display order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn list_mcp_servers(&self) -> Result<Vec<McpServerRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_server_id, display_name, transport_kind, safe_config,
                  enabled, health_status, auth_status,
                  count((SELECT VALUE id FROM mcp_tools WHERE mcp_server_id = $parent.mcp_server_id)) AS tool_count
                FROM mcp_servers;
                "#,
            )
            .await?;
        let rows: Vec<McpServerRow> = response.take(0)?;
        let mut servers: Vec<McpServerRecord> = rows
            .into_iter()
            .map(mcp_server_from_row)
            .collect::<Result<_, _>>()?;
        servers.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.mcp_server_id.cmp(&right.mcp_server_id))
        });
        Ok(servers)
    }

    /// Delete one MCP server plus discovered tool and calibration rows.
    ///
    /// Historical approval and audit rows are intentionally retained as audit
    /// records even after the server configuration is removed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read or delete fails.
    pub async fn delete_mcp_server(&self, mcp_server_id: &str) -> Result<bool, StoreError> {
        if self.get_mcp_server(mcp_server_id).await?.is_none() {
            return Ok(false);
        }

        let tools = self.list_mcp_tools_for_server(mcp_server_id).await?;
        for tool in tools {
            self.db
                .query(
                    r#"
                    DELETE tool_calibrations WHERE mcp_tool_id = $mcp_tool_id;
                    DELETE mcp_tools WHERE mcp_tool_id = $mcp_tool_id;
                    "#,
                )
                .bind(("mcp_tool_id", tool.mcp_tool_id))
                .await?
                .check()?;
        }
        self.db
            .query(
                r#"
                DELETE mcp_servers WHERE mcp_server_id = $mcp_server_id;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .await?
            .check()?;
        Ok(true)
    }

    /// Update MCP server setup health/auth status after metadata discovery.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the server is missing or the embedded store write fails.
    pub async fn update_mcp_server_setup_status(
        &self,
        mcp_server_id: &str,
        health_status: McpServerHealthStatus,
        auth_status: McpServerAuthStatus,
    ) -> Result<McpServerRecord, StoreError> {
        self.db
            .query(
                r#"
                UPDATE mcp_servers SET
                  health_status = $health_status,
                  auth_status = $auth_status,
                  updated_at = time::now()
                WHERE mcp_server_id = $mcp_server_id;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .bind(("health_status", health_status.as_str().to_string()))
            .bind(("auth_status", auth_status.as_str().to_string()))
            .await?
            .check()?;

        self.get_mcp_server(mcp_server_id).await?.ok_or_else(|| {
            StoreError::Schema(format!(
                "missing MCP server after status update: {mcp_server_id}"
            ))
        })
    }
}
