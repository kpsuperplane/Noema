use async_graphql::{Result, SimpleObject};

use crate::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, TrustedIdentitySelectorEffect,
    TrustedIdentitySelectorRecord,
};

use super::{errors::graphql_error, schema::GraphqlState};

/// MCP server metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMcpServer {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// Human-visible server name.
    pub display_name: String,
    /// Transport used to connect to the server.
    pub transport_kind: String,
    /// Whether this server is enabled.
    pub enabled: bool,
    /// Last known server health.
    pub health_status: String,
    /// Last known server authentication state.
    pub auth_status: String,
    /// Number of discovered tools for this server.
    pub tool_count: usize,
}

impl From<McpServerRecord> for GraphqlMcpServer {
    fn from(server: McpServerRecord) -> Self {
        Self {
            mcp_server_id: server.mcp_server_id,
            display_name: server.display_name,
            transport_kind: server.transport_kind.as_str().to_string(),
            enabled: server.enabled,
            health_status: health_status_label(server.health_status).to_string(),
            auth_status: auth_status_label(server.auth_status).to_string(),
            tool_count: server.tool_count,
        }
    }
}

/// Trusted identity selector metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlTrustedIdentitySelector {
    /// Durable selector id.
    pub selector_id: String,
    /// Governable owner scope the selector belongs to.
    pub owner_scope_id: String,
    /// Selector type.
    pub selector_kind: String,
    /// Normalized selector value.
    pub normalized_value: String,
    /// Selector effect.
    pub effect: String,
    /// Actor that issued this selector.
    pub issuer_actor_id: String,
}

impl From<TrustedIdentitySelectorRecord> for GraphqlTrustedIdentitySelector {
    fn from(selector: TrustedIdentitySelectorRecord) -> Self {
        Self {
            selector_id: selector.selector_id,
            owner_scope_id: selector.owner_scope_id,
            selector_kind: selector.selector_kind.as_str().to_string(),
            normalized_value: selector.normalized_value,
            effect: selector_effect_label(selector.effect).to_string(),
            issuer_actor_id: selector.issuer_actor_id,
        }
    }
}

const fn health_status_label(status: McpServerHealthStatus) -> &'static str {
    match status {
        McpServerHealthStatus::Unknown => "unknown",
        McpServerHealthStatus::Healthy => "healthy",
        McpServerHealthStatus::Unavailable => "unavailable",
    }
}

const fn auth_status_label(status: McpServerAuthStatus) -> &'static str {
    match status {
        McpServerAuthStatus::None => "none",
        McpServerAuthStatus::NeedsAuth => "needs_auth",
        McpServerAuthStatus::Authenticated => "authenticated",
        McpServerAuthStatus::Unavailable => "unavailable",
    }
}

const fn selector_effect_label(effect: TrustedIdentitySelectorEffect) -> &'static str {
    match effect {
        TrustedIdentitySelectorEffect::Trust => "trust",
        TrustedIdentitySelectorEffect::Restrict => "restrict",
    }
}

pub(super) async fn mcp_servers(state: &GraphqlState) -> Result<Vec<GraphqlMcpServer>> {
    let store = state.store()?;
    let servers = store.list_mcp_servers().await.map_err(graphql_error)?;
    Ok(servers.into_iter().map(Into::into).collect())
}

pub(super) async fn trusted_identity_selectors(
    state: &GraphqlState,
    owner_scope_id: String,
) -> Result<Vec<GraphqlTrustedIdentitySelector>> {
    let store = state.store()?;
    let selectors = store
        .list_trusted_identity_selectors(&owner_scope_id)
        .await
        .map_err(graphql_error)?;
    Ok(selectors.into_iter().map(Into::into).collect())
}
