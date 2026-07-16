use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{OptionalExtension, params};

use super::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, NewMcpServer,
    rows::mcp_server_from_row,
};
use crate::store::{
    NoemaStore, StoreError,
    sqlite::{json_to_string, now_timestamp_sql},
};

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
        let safe_config_json = json_to_string(&server.safe_config)?;
        let authority_generation = random_authority_generation()?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO mcp_servers (
                  mcp_server_id, display_name, transport_kind, safe_config_json,
                  auth_status, health_status, enabled, metadata_fingerprint, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, 'none', 'unknown', 0, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                params![
                    server.mcp_server_id,
                    server.display_name,
                    server.transport_kind.as_str(),
                    safe_config_json,
                    authority_generation,
                ],
            )?;
            Ok(())
        })
        .await?;
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
        self.with_connection(|conn| {
            conn.query_row(
                MCP_SERVER_SELECT_WITH_TOOL_COUNT,
                params![mcp_server_id],
                mcp_server_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List MCP servers in deterministic Settings display order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn list_mcp_servers(&self) -> Result<Vec<McpServerRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
                  m.enabled AND EXISTS (
                    SELECT 1
                    FROM mcp_tools eligible_t
                    JOIN tool_calibrations c ON c.mcp_tool_id = eligible_t.mcp_tool_id
                    WHERE eligible_t.mcp_server_id = m.mcp_server_id
                      AND c.status = 'ready'
                      AND c.read_classification IN ('trusted', 'untrusted')
                      AND c.write_classification = 'none'
                      AND c.export_classification = 'none'
                      AND c.reviewed_metadata_fingerprint = eligible_t.metadata_fingerprint
                  ) AS enabled,
                  m.health_status, m.auth_status, COUNT(t.mcp_tool_id) AS tool_count,
                  COALESCE(m.metadata_fingerprint, '') AS authority_generation
                FROM mcp_servers m
                LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
                GROUP BY m.mcp_server_id
                ORDER BY m.display_name, m.mcp_server_id
                "#,
            )?;
            let rows = statement.query_map([], mcp_server_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
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

        self.with_connection(|conn| {
            conn.execute(
                r#"
                DELETE FROM tool_calibrations
                WHERE mcp_tool_id IN (
                  SELECT mcp_tool_id FROM mcp_tools WHERE mcp_server_id = ?1
                )
                "#,
                params![mcp_server_id],
            )?;
            conn.execute(
                "DELETE FROM mcp_tools WHERE mcp_server_id = ?1",
                params![mcp_server_id],
            )?;
            conn.execute(
                "DELETE FROM mcp_servers WHERE mcp_server_id = ?1",
                params![mcp_server_id],
            )?;
            Ok(())
        })
        .await?;
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
        self.with_connection(|conn| {
            conn.execute(
                format!(
                    r#"
                    UPDATE mcp_servers SET
                      health_status = ?2,
                      auth_status = ?3,
                      updated_at = {}
                    WHERE mcp_server_id = ?1
                    "#,
                    now_timestamp_sql()
                )
                .as_str(),
                params![mcp_server_id, health_status.as_str(), auth_status.as_str()],
            )?;
            Ok(())
        })
        .await?;

        self.get_mcp_server(mcp_server_id).await?.ok_or_else(|| {
            StoreError::Schema(format!(
                "missing MCP server after status update: {mcp_server_id}"
            ))
        })
    }

    /// Replace connection-defining metadata and rotate invocation authority
    /// when either the transport or safe configuration changes.
    pub(crate) async fn update_mcp_server_connection_identity(
        &self,
        mcp_server_id: &str,
        transport_kind: crate::McpTransportKind,
        safe_config: serde_json::Value,
    ) -> Result<McpServerRecord, StoreError> {
        let safe_config_json = json_to_string(&safe_config)?;
        let authority_generation = random_authority_generation()?;
        self.with_connection(|conn| {
            conn.execute(
                format!(
                    r#"
                    UPDATE mcp_servers SET
                      transport_kind = ?2,
                      safe_config_json = ?3,
                      metadata_fingerprint = CASE
                        WHEN transport_kind <> ?2 OR safe_config_json <> ?3 THEN ?4
                        ELSE metadata_fingerprint
                      END,
                      updated_at = {}
                    WHERE mcp_server_id = ?1
                    "#,
                    now_timestamp_sql()
                )
                .as_str(),
                params![
                    mcp_server_id,
                    transport_kind.as_str(),
                    safe_config_json,
                    authority_generation,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_mcp_server(mcp_server_id).await?.ok_or_else(|| {
            StoreError::Schema(format!(
                "missing MCP server after connection identity update: {mcp_server_id}"
            ))
        })
    }
}

const MCP_SERVER_SELECT_WITH_TOOL_COUNT: &str = r#"
SELECT m.mcp_server_id, m.display_name, m.transport_kind, m.safe_config_json,
  m.enabled AND EXISTS (
    SELECT 1
    FROM mcp_tools eligible_t
    JOIN tool_calibrations c ON c.mcp_tool_id = eligible_t.mcp_tool_id
    WHERE eligible_t.mcp_server_id = m.mcp_server_id
      AND c.status = 'ready'
      AND c.read_classification IN ('trusted', 'untrusted')
      AND c.write_classification = 'none'
      AND c.export_classification = 'none'
      AND c.reviewed_metadata_fingerprint = eligible_t.metadata_fingerprint
  ) AS enabled,
  m.health_status, m.auth_status, COUNT(t.mcp_tool_id) AS tool_count,
  COALESCE(m.metadata_fingerprint, '') AS authority_generation
FROM mcp_servers m
LEFT JOIN mcp_tools t ON t.mcp_server_id = m.mcp_server_id
WHERE m.mcp_server_id = ?1
GROUP BY m.mcp_server_id
LIMIT 1
"#;

fn random_authority_generation() -> Result<String, StoreError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| StoreError::Schema("failed to create MCP authority generation".to_string()))?;
    let mut generation = String::with_capacity("mcp_generation:".len() + bytes.len() * 2);
    generation.push_str("mcp_generation:");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(generation, "{byte:02x}");
    }
    Ok(generation)
}
