use serde::Deserialize;
use serde_json::{Map, Value};
use surrealdb::types::SurrealValue;

use crate::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, OwnerExtractor,
    TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};

use super::{NoemaStore, StoreError, ids::now_string};

/// Input for creating an MCP server metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
}

/// Persisted MCP server settings read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerRecord {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport/configuration metadata.
    pub safe_config: Value,
    /// Whether this server is enabled.
    pub enabled: bool,
    /// Last known server health.
    pub health_status: McpServerHealthStatus,
    /// Last known server authentication state.
    pub auth_status: McpServerAuthStatus,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
}

/// Last known MCP server health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerHealthStatus {
    /// Health has not been checked.
    Unknown,
    /// Server is reachable and healthy.
    Healthy,
    /// Server is unavailable.
    Unavailable,
}

/// Last known MCP server authentication state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerAuthStatus {
    /// Server does not require authentication.
    None,
    /// Server requires authentication.
    NeedsAuth,
    /// Server is authenticated.
    Authenticated,
    /// Authentication is unavailable or failed externally.
    Unavailable,
}

/// Input captured from MCP tool discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpTool {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Value,
    /// Optional MCP output schema.
    pub output_schema: Option<Value>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Value,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
}

/// Persisted MCP tool discovery read model.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolRecord {
    /// Durable MCP tool id.
    pub mcp_tool_id: String,
    /// Owning MCP server id.
    pub mcp_server_id: String,
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    pub description: Option<String>,
    /// MCP input schema.
    pub input_schema: Value,
    /// Optional MCP output schema.
    pub output_schema: Option<Value>,
    /// MCP annotations captured as non-authoritative setup hints.
    pub annotations: Value,
    /// Fingerprint of the metadata snapshot.
    pub metadata_fingerprint: String,
    /// Discovery timestamp string.
    pub discovered_at: String,
}

/// Input for saving reviewed MCP tool calibration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewToolCalibration {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: McpTrustClassification,
    /// Effective write classification.
    pub write_classification: McpTrustClassification,
    /// Effective export classification.
    pub export_classification: McpTrustClassification,
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<OwnerExtractor>,
    /// Agents allowed to see/use this calibration.
    pub enabled_agent_ids: Vec<String>,
    /// Governable scopes where this calibration is enabled.
    pub enabled_scope_ids: Vec<String>,
    /// Review/gateway readiness status.
    pub status: McpCalibrationStatus,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Persisted MCP tool calibration read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCalibrationRecord {
    /// Durable calibration id.
    pub calibration_id: String,
    /// Calibrated MCP tool id.
    pub mcp_tool_id: String,
    /// Effective read classification.
    pub read_classification: McpTrustClassification,
    /// Effective write classification.
    pub write_classification: McpTrustClassification,
    /// Effective export classification.
    pub export_classification: McpTrustClassification,
    /// Deterministic owner extractors configured for this tool.
    pub owner_extractors: Vec<OwnerExtractor>,
    /// Agents allowed to see/use this calibration.
    pub enabled_agent_ids: Vec<String>,
    /// Governable scopes where this calibration is enabled.
    pub enabled_scope_ids: Vec<String>,
    /// Review/gateway readiness status.
    pub status: McpCalibrationStatus,
    /// Actor who reviewed the calibration, when reviewed.
    pub reviewed_by: Option<String>,
    /// Tool metadata fingerprint reviewed by the actor.
    pub reviewed_metadata_fingerprint: Option<String>,
}

/// Input for creating a trusted identity selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTrustedIdentitySelector {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// Unnormalized user-provided selector value.
    pub raw_value: String,
    /// Selector effect.
    pub effect: TrustedIdentitySelectorEffect,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
}

/// Trusted identity selector effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedIdentitySelectorEffect {
    /// Trust this identity for the owner scope.
    Trust,
    /// Restrict this identity for the owner scope.
    Restrict,
}

/// Persisted trusted identity selector read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedIdentitySelectorRecord {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// Normalized selector value.
    pub normalized_value: String,
    /// Selector effect.
    pub effect: TrustedIdentitySelectorEffect,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
    /// Revocation timestamp string, when revoked.
    pub revoked_at: Option<String>,
}

/// Input for creating a durable MCP approval request.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpApprovalRequest {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id.
    pub tool_invocation_id: String,
    /// Related MCP server id, when available.
    pub mcp_server_id: Option<String>,
    /// Related MCP tool id, when available.
    pub mcp_tool_id: Option<String>,
    /// Actor requesting approval.
    pub requester_actor_id: String,
    /// Governable owner scope for the approval.
    pub owner_scope_id: String,
    /// Active governable scope for the approval.
    pub active_scope_id: String,
    /// Destination or recipient summary.
    pub destination_summary: String,
    /// Data source summary.
    pub data_source_summary: String,
    /// Source owner identity label.
    pub source_owner_identity: String,
    /// Source owner trust label.
    pub source_owner_trust: String,
    /// Destination owner identity label.
    pub destination_owner_identity: String,
    /// Destination owner trust label.
    pub destination_owner_trust: String,
    /// What leaves the MCP destination trust boundary.
    pub export_summary: String,
    /// Safe payload preview for review surfaces.
    pub payload_preview: Value,
}

/// Durable MCP approval request metadata safe to show in Settings.
#[derive(Debug, Clone, PartialEq)]
pub struct McpApprovalRequestRecord {
    /// Durable approval request id.
    pub approval_id: String,
    /// Safe human-readable action summary.
    pub action_summary: String,
    /// Related tool invocation id.
    pub tool_invocation_id: String,
    /// Related MCP server id, when available.
    pub mcp_server_id: Option<String>,
    /// Related MCP tool id, when available.
    pub mcp_tool_id: Option<String>,
    /// Actor requesting approval.
    pub requester_actor_id: String,
    /// Governable owner scope for the approval.
    pub owner_scope_id: String,
    /// Active governable scope for the approval.
    pub active_scope_id: String,
    /// Destination or recipient summary.
    pub destination_summary: String,
    /// Data source summary.
    pub data_source_summary: String,
    /// Source owner identity label.
    pub source_owner_identity: String,
    /// Source owner trust label.
    pub source_owner_trust: String,
    /// Destination owner identity label.
    pub destination_owner_identity: String,
    /// Destination owner trust label.
    pub destination_owner_trust: String,
    /// What leaves the MCP destination trust boundary.
    pub export_summary: String,
    /// Safe payload preview for review surfaces.
    pub payload_preview: Value,
    /// Current approval status.
    pub status: String,
    /// Actor who decided the request, when decided.
    pub decision_actor_id: Option<String>,
    /// Safe decision comment, when available.
    pub decision_comment: Option<String>,
    /// Decision timestamp string, when decided.
    pub decided_at: Option<String>,
}

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

    /// Save reviewed calibration for one MCP tool.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails, or
    /// when a stored enum is invalid.
    pub async fn save_tool_calibration(
        &self,
        calibration: NewToolCalibration,
    ) -> Result<ToolCalibrationRecord, StoreError> {
        self.validate_tool_calibration(&calibration).await?;
        let owner_extractors =
            serde_json::to_value(&calibration.owner_extractors).map_err(|error| {
                StoreError::Schema(format!("invalid owner extractor serialization: {error}"))
            })?;
        self.db
            .query(
                r#"
                UPSERT type::record('tool_calibrations', $record_id) SET
                  calibration_id = $calibration_id,
                  mcp_tool_id = $mcp_tool_id,
                  read_classification = $read_classification,
                  write_classification = $write_classification,
                  export_classification = $export_classification,
                  owner_extractors = $owner_extractors,
                  enabled_agent_ids = $enabled_agent_ids,
                  enabled_scope_ids = $enabled_scope_ids,
                  status = $status,
                  reviewed_by = $reviewed_by,
                  reviewed_metadata_fingerprint = $reviewed_metadata_fingerprint,
                  updated_at = time::now();
                "#,
            )
            .bind((
                "record_id",
                mcp_record_fragment(&calibration.calibration_id),
            ))
            .bind(("calibration_id", calibration.calibration_id.clone()))
            .bind(("mcp_tool_id", calibration.mcp_tool_id.clone()))
            .bind((
                "read_classification",
                calibration.read_classification.as_str().to_string(),
            ))
            .bind((
                "write_classification",
                calibration.write_classification.as_str().to_string(),
            ))
            .bind((
                "export_classification",
                calibration.export_classification.as_str().to_string(),
            ))
            .bind(("owner_extractors", owner_extractors))
            .bind(("enabled_agent_ids", calibration.enabled_agent_ids))
            .bind(("enabled_scope_ids", calibration.enabled_scope_ids))
            .bind(("status", calibration.status.as_str().to_string()))
            .bind(("reviewed_by", calibration.reviewed_by))
            .bind((
                "reviewed_metadata_fingerprint",
                calibration.reviewed_metadata_fingerprint,
            ))
            .await?
            .check()?;
        self.get_tool_calibration(&calibration.mcp_tool_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing tool calibration after save: {}",
                    calibration.mcp_tool_id
                ))
            })
    }

    async fn validate_tool_calibration(
        &self,
        calibration: &NewToolCalibration,
    ) -> Result<(), StoreError> {
        let tool = self
            .get_mcp_tool(&calibration.mcp_tool_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "cannot calibrate missing MCP tool: {}",
                    calibration.mcp_tool_id
                ))
            })?;

        if let Some(existing) = self
            .get_tool_calibration_by_calibration_id(&calibration.calibration_id)
            .await?
            && existing.mcp_tool_id != calibration.mcp_tool_id
        {
            return Err(StoreError::Schema(format!(
                "tool calibration {} already belongs to {}",
                calibration.calibration_id, existing.mcp_tool_id
            )));
        }

        if let Some(existing) = self.get_tool_calibration(&calibration.mcp_tool_id).await?
            && existing.calibration_id != calibration.calibration_id
        {
            return Err(StoreError::Schema(format!(
                "MCP tool {} already has calibration {}",
                calibration.mcp_tool_id, existing.calibration_id
            )));
        }

        if calibration.status.requires_reviewed_metadata() {
            calibration
                .reviewed_by
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    StoreError::Schema(
                        "reviewed MCP tool calibration requires reviewed_by".to_string(),
                    )
                })?;
            let reviewed_fingerprint = calibration
                .reviewed_metadata_fingerprint
                .as_deref()
                .ok_or_else(|| {
                    StoreError::Schema(
                        "reviewed MCP tool calibration requires reviewed_metadata_fingerprint"
                            .to_string(),
                    )
                })?;
            if reviewed_fingerprint != tool.metadata_fingerprint {
                return Err(StoreError::Schema(format!(
                    "reviewed metadata fingerprint does not match current MCP tool metadata for {}",
                    calibration.mcp_tool_id
                )));
            }
            if calibration.status == McpCalibrationStatus::Ready
                && calibration.has_mixed_classification()
                && calibration.owner_extractors.is_empty()
            {
                return Err(StoreError::Schema(format!(
                    "ready mixed MCP tool calibration requires an owner extractor: {}",
                    calibration.mcp_tool_id
                )));
            }
        }

        Ok(())
    }

    async fn get_tool_calibration_by_calibration_id(
        &self,
        calibration_id: &str,
    ) -> Result<Option<ToolCalibrationRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT calibration_id, mcp_tool_id, read_classification,
                  write_classification, export_classification, owner_extractors,
                  enabled_agent_ids, enabled_scope_ids, status, reviewed_by,
                  reviewed_metadata_fingerprint
                FROM tool_calibrations
                WHERE calibration_id = $calibration_id
                LIMIT 1;
                "#,
            )
            .bind(("calibration_id", calibration_id.to_string()))
            .await?;
        let rows: Vec<ToolCalibrationRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(tool_calibration_from_row)
            .transpose()
    }

    async fn invalidate_tool_calibration_review(
        &self,
        mcp_tool_id: &str,
    ) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPDATE tool_calibrations SET
                  status = 'needs_review',
                  reviewed_by = NONE,
                  reviewed_metadata_fingerprint = NONE,
                  updated_at = time::now()
                WHERE mcp_tool_id = $mcp_tool_id;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?
            .check()?;
        Ok(())
    }

    /// Return reviewed calibration for one MCP tool id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_tool_calibration(
        &self,
        mcp_tool_id: &str,
    ) -> Result<Option<ToolCalibrationRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT calibration_id, mcp_tool_id, read_classification,
                  write_classification, export_classification, owner_extractors,
                  enabled_agent_ids, enabled_scope_ids, status, reviewed_by,
                  reviewed_metadata_fingerprint
                FROM tool_calibrations
                WHERE mcp_tool_id = $mcp_tool_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?;
        let rows: Vec<ToolCalibrationRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(tool_calibration_from_row)
            .transpose()
    }

    /// Create one trusted identity selector after normalizing the raw value.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the raw selector value is invalid, or when
    /// the embedded store write/read fails.
    pub async fn create_trusted_identity_selector(
        &self,
        selector: NewTrustedIdentitySelector,
    ) -> Result<TrustedIdentitySelectorRecord, StoreError> {
        let normalized_value =
            normalize_trusted_identity_value(selector.selector_kind, &selector.raw_value)
                .ok_or_else(|| {
                    StoreError::Schema(format!(
                        "invalid trusted identity selector value for {}",
                        selector.selector_kind.as_str()
                    ))
                })?;
        self.db
            .query(
                r#"
                CREATE type::record('trusted_identity_selectors', $record_id) SET
                  selector_id = $selector_id,
                  owner_scope_id = $owner_scope_id,
                  selector_kind = $selector_kind,
                  normalized_value = $normalized_value,
                  effect = $effect,
                  issuer_actor_id = $issuer_actor_id,
                  revoked_at = NONE,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&selector.selector_id)))
            .bind(("selector_id", selector.selector_id.clone()))
            .bind(("owner_scope_id", selector.owner_scope_id))
            .bind(("selector_kind", selector.selector_kind.as_str().to_string()))
            .bind(("normalized_value", normalized_value))
            .bind(("effect", selector.effect.as_str().to_string()))
            .bind(("issuer_actor_id", selector.issuer_actor_id))
            .await?
            .check()?;
        self.get_trusted_identity_selector(&selector.selector_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing trusted identity selector after create: {}",
                    selector.selector_id
                ))
            })
    }

    /// Return one trusted identity selector by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_trusted_identity_selector(
        &self,
        selector_id: &str,
    ) -> Result<Option<TrustedIdentitySelectorRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
                  effect, issuer_actor_id, revoked_at
                FROM trusted_identity_selectors
                WHERE selector_id = $selector_id
                LIMIT 1;
                "#,
            )
            .bind(("selector_id", selector_id.to_string()))
            .await?;
        let rows: Vec<TrustedIdentitySelectorRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(trusted_identity_selector_from_row)
            .transpose()
    }

    /// List trusted identity selectors for one owner scope in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn list_trusted_identity_selectors(
        &self,
        owner_scope_id: &str,
    ) -> Result<Vec<TrustedIdentitySelectorRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
                  effect, issuer_actor_id, revoked_at
                FROM trusted_identity_selectors
                WHERE owner_scope_id = $owner_scope_id;
                "#,
            )
            .bind(("owner_scope_id", owner_scope_id.to_string()))
            .await?;
        let rows: Vec<TrustedIdentitySelectorRow> = response.take(0)?;
        let mut selectors: Vec<TrustedIdentitySelectorRecord> = rows
            .into_iter()
            .map(trusted_identity_selector_from_row)
            .collect::<Result<_, _>>()?;
        selectors.sort_by(|left, right| {
            left.selector_kind
                .as_str()
                .cmp(right.selector_kind.as_str())
                .then_with(|| left.normalized_value.cmp(&right.normalized_value))
        });
        Ok(selectors)
    }

    /// Create one durable MCP approval request.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_mcp_approval_request(
        &self,
        approval: NewMcpApprovalRequest,
    ) -> Result<McpApprovalRequestRecord, StoreError> {
        let payload_preview = sanitize_approval_payload_preview(approval.payload_preview);
        self.db
            .query(
                r#"
                CREATE type::record('approval_requests', $record_id) SET
                  approval_id = $approval_id,
                  action_summary = $action_summary,
                  tool_invocation_id = $tool_invocation_id,
                  mcp_server_id = $mcp_server_id,
                  mcp_tool_id = $mcp_tool_id,
                  requester_actor_id = $requester_actor_id,
                  owner_scope_id = $owner_scope_id,
                  active_scope_id = $active_scope_id,
                  destination_summary = $destination_summary,
                  data_source_summary = $data_source_summary,
                  source_owner_identity = $source_owner_identity,
                  source_owner_trust = $source_owner_trust,
                  destination_owner_identity = $destination_owner_identity,
                  destination_owner_trust = $destination_owner_trust,
                  export_summary = $export_summary,
                  payload_preview = $payload_preview,
                  status = 'pending',
                  decision_actor_id = NONE,
                  decision_comment = NONE,
                  decided_at = NONE,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&approval.approval_id)))
            .bind(("approval_id", approval.approval_id.clone()))
            .bind(("action_summary", approval.action_summary))
            .bind(("tool_invocation_id", approval.tool_invocation_id))
            .bind(("mcp_server_id", approval.mcp_server_id))
            .bind(("mcp_tool_id", approval.mcp_tool_id))
            .bind(("requester_actor_id", approval.requester_actor_id))
            .bind(("owner_scope_id", approval.owner_scope_id))
            .bind(("active_scope_id", approval.active_scope_id))
            .bind(("destination_summary", approval.destination_summary))
            .bind(("data_source_summary", approval.data_source_summary))
            .bind(("source_owner_identity", approval.source_owner_identity))
            .bind(("source_owner_trust", approval.source_owner_trust))
            .bind((
                "destination_owner_identity",
                approval.destination_owner_identity,
            ))
            .bind(("destination_owner_trust", approval.destination_owner_trust))
            .bind(("export_summary", approval.export_summary))
            .bind(("payload_preview", payload_preview))
            .await?
            .check()?;
        self.get_mcp_approval_request(&approval.approval_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing MCP approval request after create: {}",
                    approval.approval_id
                ))
            })
    }

    /// List durable MCP approval requests, optionally filtered by status.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn list_mcp_approval_requests(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<McpApprovalRequestRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT approval_id, action_summary, tool_invocation_id, mcp_server_id,
                  mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
                  destination_summary, data_source_summary, source_owner_identity,
                  source_owner_trust, destination_owner_identity, destination_owner_trust,
                  export_summary, payload_preview, status, decision_actor_id,
                  decision_comment, decided_at
                FROM approval_requests
                WHERE $status = NONE OR status = $status;
                "#,
            )
            .bind(("status", status.map(str::to_string)))
            .await?;
        let rows: Vec<McpApprovalRequestRow> = response.take(0)?;
        let mut approvals: Vec<McpApprovalRequestRecord> = rows
            .into_iter()
            .map(mcp_approval_request_from_row)
            .collect();
        approvals.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
        Ok(approvals)
    }

    async fn get_mcp_approval_request(
        &self,
        approval_id: &str,
    ) -> Result<Option<McpApprovalRequestRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT approval_id, action_summary, tool_invocation_id, mcp_server_id,
                  mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
                  destination_summary, data_source_summary, source_owner_identity,
                  source_owner_trust, destination_owner_identity, destination_owner_trust,
                  export_summary, payload_preview, status, decision_actor_id,
                  decision_comment, decided_at
                FROM approval_requests
                WHERE approval_id = $approval_id
                LIMIT 1;
                "#,
            )
            .bind(("approval_id", approval_id.to_string()))
            .await?;
        let rows: Vec<McpApprovalRequestRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(mcp_approval_request_from_row))
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct McpServerRow {
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
struct McpToolRow {
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
struct ToolCalibrationRow {
    calibration_id: String,
    mcp_tool_id: String,
    read_classification: String,
    write_classification: String,
    export_classification: String,
    owner_extractors: Value,
    enabled_agent_ids: Vec<String>,
    enabled_scope_ids: Vec<String>,
    status: String,
    reviewed_by: Option<String>,
    reviewed_metadata_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct TrustedIdentitySelectorRow {
    selector_id: String,
    owner_scope_id: String,
    selector_kind: String,
    normalized_value: String,
    effect: String,
    issuer_actor_id: String,
    revoked_at: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct McpApprovalRequestRow {
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

fn mcp_server_from_row(row: McpServerRow) -> Result<McpServerRecord, StoreError> {
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

fn mcp_tool_from_row(row: McpToolRow) -> McpToolRecord {
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

fn tool_calibration_from_row(row: ToolCalibrationRow) -> Result<ToolCalibrationRecord, StoreError> {
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
        enabled_agent_ids: row.enabled_agent_ids,
        enabled_scope_ids: row.enabled_scope_ids,
        status: parse_mcp_calibration_status(&row.status)?,
        reviewed_by: row.reviewed_by,
        reviewed_metadata_fingerprint: row.reviewed_metadata_fingerprint,
    })
}

fn trusted_identity_selector_from_row(
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

fn mcp_approval_request_from_row(row: McpApprovalRequestRow) -> McpApprovalRequestRecord {
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

fn sanitize_approval_payload_preview(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(sanitize_preview_object(object)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .take(10)
                .map(sanitize_approval_payload_preview)
                .collect(),
        ),
        Value::String(value) => Value::String(truncate_preview_string(value)),
        other => other,
    }
}

fn sanitize_preview_object(object: Map<String, Value>) -> Map<String, Value> {
    object
        .into_iter()
        .take(20)
        .map(|(key, value)| {
            let sanitized_value = if approval_preview_key_is_sensitive(&key) {
                Value::String("[redacted]".to_string())
            } else {
                sanitize_approval_payload_preview(value)
            };
            (key, sanitized_value)
        })
        .collect()
}

fn approval_preview_key_is_sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "credential",
        "api_key",
        "apikey",
        "private_key",
        "authorization",
        "auth",
        "cookie",
        "session",
        "set-cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn truncate_preview_string(value: String) -> String {
    const MAX_PREVIEW_CHARS: usize = 240;
    if value.chars().count() <= MAX_PREVIEW_CHARS {
        return value;
    }

    let mut truncated = value.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn parse_mcp_transport_kind(value: &str) -> Result<McpTransportKind, StoreError> {
    match value {
        "stdio" => Ok(McpTransportKind::Stdio),
        "http_sse" => Ok(McpTransportKind::HttpSse),
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

impl McpServerHealthStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Unavailable => "unavailable",
        }
    }
}

impl McpServerAuthStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NeedsAuth => "needs_auth",
            Self::Authenticated => "authenticated",
            Self::Unavailable => "unavailable",
        }
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

impl TrustedIdentitySelectorEffect {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Trust => "trust",
            Self::Restrict => "restrict",
        }
    }
}

impl McpCalibrationStatus {
    const fn requires_reviewed_metadata(self) -> bool {
        matches!(self, Self::BlockedUnresolvedOwnership | Self::Ready)
    }
}

impl NewToolCalibration {
    fn has_mixed_classification(&self) -> bool {
        [
            self.read_classification,
            self.write_classification,
            self.export_classification,
        ]
        .contains(&McpTrustClassification::Mixed)
    }
}

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

fn mcp_record_fragment(id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut fragment = String::with_capacity("mcp_".len() + id.len() * 2);
    fragment.push_str("mcp_");
    for byte in id.bytes() {
        fragment.push(HEX[(byte >> 4) as usize] as char);
        fragment.push(HEX[(byte & 0x0f) as usize] as char);
    }
    fragment
}
