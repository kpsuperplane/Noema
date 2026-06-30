use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{McpTransportKind, TrustedIdentitySelectorKind, normalize_trusted_identity_value};

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
struct TrustedIdentitySelectorRow {
    selector_id: String,
    owner_scope_id: String,
    selector_kind: String,
    normalized_value: String,
    effect: String,
    issuer_actor_id: String,
    revoked_at: Option<String>,
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
