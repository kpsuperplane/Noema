//! HTTP MCP metadata transports.

use std::collections::{BTreeMap, HashMap};

use crate::{
    McpServerRecord,
    mcp::{
        client::{
            DiscoveredMcpTool, McpClientError, McpDiagnosticContext, McpTransport,
            call_tool_params, call_tool_result_value, discovered_tool_from_rmcp, string_field,
            string_map_field,
        },
        oauth::oauth_state_for_mcp_url,
        secrets::{
            McpOAuthClientCredentials, McpOAuthStoredCredentials, McpSecretMaterial,
            now_epoch_seconds,
        },
    },
};
use http::{HeaderName as RmcpHeaderName, HeaderValue as RmcpHeaderValue};
use noema_home::SystemErrorLogger;
use rmcp::{
    ServiceExt,
    transport::{
        ClientCredentialsConfig, StreamableHttpClientTransport, auth::OAuthTokenResponse,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};

const OAUTH_REFRESH_SKEW_SECONDS: u64 = 30;

/// Metadata-only MCP transport over Streamable HTTP.
pub struct StreamableHttpMcpTransport {
    url: String,
    headers: BTreeMap<String, String>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
    oauth_credentials: Option<McpOAuthStoredCredentials>,
    discovered_tools: Option<Vec<DiscoveredMcpTool>>,
    diagnostics: Option<SystemErrorLogger>,
    diagnostic_mcp_server_id: Option<String>,
}

impl StreamableHttpMcpTransport {
    /// Create a Streamable HTTP MCP metadata transport.
    #[must_use]
    pub const fn new(url: String, headers: BTreeMap<String, String>) -> Self {
        Self {
            url,
            headers,
            oauth_client_credentials: None,
            oauth_credentials: None,
            discovered_tools: None,
            diagnostics: None,
            diagnostic_mcp_server_id: None,
        }
    }

    /// Build a Streamable HTTP transport from persisted safe config and disk-backed secrets.
    ///
    /// # Errors
    ///
    /// Returns an error when the persisted config does not have the expected
    /// HTTP shape.
    pub fn from_server_config(
        server: &McpServerRecord,
        secrets: &McpSecretMaterial,
    ) -> Result<Self, String> {
        let config = http_config_from_server(server, secrets, "streamable_http")?;
        Ok(Self {
            url: config.url,
            headers: config.headers,
            oauth_client_credentials: config.oauth_client_credentials,
            oauth_credentials: config.oauth_credentials,
            discovered_tools: None,
            diagnostics: None,
            diagnostic_mcp_server_id: None,
        })
    }

    /// Return current stored browser OAuth credentials after any refresh.
    #[must_use]
    pub fn oauth_credentials(&self) -> Option<&McpOAuthStoredCredentials> {
        self.oauth_credentials.as_ref()
    }

    /// Attach developer diagnostics for malformed MCP responses.
    #[must_use]
    pub fn with_diagnostics(
        mut self,
        diagnostics: Option<SystemErrorLogger>,
        mcp_server_id: Option<String>,
    ) -> Self {
        self.diagnostics = diagnostics;
        self.diagnostic_mcp_server_id = mcp_server_id;
        self
    }

    fn diagnostic_context(&self, method: &'static str) -> McpDiagnosticContext {
        McpDiagnosticContext::new(
            self.diagnostics.clone(),
            self.diagnostic_mcp_server_id.clone(),
            Some("streamable_http".to_string()),
            method,
        )
    }
}

impl McpTransport for StreamableHttpMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.clone())
            .custom_headers(rmcp_headers(&self.headers)?);
        if let Some(token) = oauth_access_token(
            &self.url,
            self.oauth_client_credentials.as_ref(),
            self.oauth_credentials.as_ref(),
        )
        .await?
        {
            self.oauth_credentials = token.stored_credentials.or(self.oauth_credentials.take());
            config = config.auth_header(token.access_token);
        }
        let transport = StreamableHttpClientTransport::from_config(config);
        let mut service = ().serve(transport).await.map_err(rmcp_initialize_error)?;
        let diagnostics = self.diagnostic_context("tools/list");
        let tools = service
            .peer()
            .list_all_tools()
            .await
            .map_err(|error| McpClientError::Transport(format!("MCP tools/list failed: {error}")))?
            .into_iter()
            .map(|tool| {
                let raw_tool = format!("{tool:?}");
                discovered_tool_from_rmcp(tool).inspect_err(|error| {
                    diagnostics.log_malformed(error, json!({ "sdk_tool_debug": raw_tool }));
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let _ = service.close().await;
        self.discovered_tools = Some(tools);
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
        self.discovered_tools.clone().ok_or_else(|| {
            McpClientError::Transport(
                "MCP Streamable HTTP transport is not initialized".to_string(),
            )
        })
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, McpClientError> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.clone())
            .custom_headers(rmcp_headers(&self.headers)?);
        if let Some(token) = oauth_access_token(
            &self.url,
            self.oauth_client_credentials.as_ref(),
            self.oauth_credentials.as_ref(),
        )
        .await?
        {
            self.oauth_credentials = token.stored_credentials.or(self.oauth_credentials.take());
            config = config.auth_header(token.access_token);
        }
        let transport = StreamableHttpClientTransport::from_config(config);
        let mut service = ().serve(transport).await.map_err(rmcp_initialize_error)?;
        let diagnostics = self.diagnostic_context("tools/call");
        let result = service
            .peer()
            .call_tool(call_tool_params(name, arguments)?)
            .await
            .map_err(|error| McpClientError::Transport(format!("MCP tools/call failed: {error}")))
            .and_then(|result| {
                let raw_result = format!("{result:?}");
                call_tool_result_value(result).inspect_err(|error| {
                    diagnostics
                        .log_malformed(error, json!({ "sdk_call_result_debug": raw_result }));
                })
            });
        let _ = service.close().await;
        result
    }
}

struct HttpConfig {
    url: String,
    headers: BTreeMap<String, String>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
    oauth_credentials: Option<McpOAuthStoredCredentials>,
}

struct OAuthAccessToken {
    access_token: String,
    stored_credentials: Option<McpOAuthStoredCredentials>,
}

fn http_config_from_server(
    server: &McpServerRecord,
    secrets: &McpSecretMaterial,
    transport_label: &'static str,
) -> Result<HttpConfig, String> {
    let object = server
        .safe_config
        .as_object()
        .ok_or_else(|| format!("MCP {transport_label} config must be an object"))?;
    let url = string_field(object, "url")?;
    let mut headers = string_map_field(object, "headers")?;
    headers.extend(secrets.headers.clone());
    Ok(HttpConfig {
        url,
        headers,
        oauth_client_credentials: secrets.oauth_client_credentials.clone(),
        oauth_credentials: secrets.oauth_credentials.clone(),
    })
}

async fn oauth_access_token(
    url: &str,
    client_credentials: Option<&McpOAuthClientCredentials>,
    stored_credentials: Option<&McpOAuthStoredCredentials>,
) -> Result<Option<OAuthAccessToken>, McpClientError> {
    if let Some(credentials) = stored_credentials {
        return stored_oauth_access_token(url, credentials).await.map(Some);
    }
    let Some(credentials) = client_credentials else {
        return Ok(None);
    };
    let mut oauth_state = oauth_state_for_mcp_url(url).await.map_err(|error| {
        McpClientError::AuthRequired(format!("MCP OAuth initialization failed: {error}"))
    })?;
    oauth_state
        .authenticate_client_credentials(ClientCredentialsConfig::ClientSecret {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            scopes: credentials.scopes.clone(),
            resource: Some(url.to_string()),
        })
        .await
        .map_err(|error| {
            McpClientError::AuthRequired(format!("MCP OAuth client credentials failed: {error}"))
        })?;
    let manager = oauth_state.into_authorization_manager().ok_or_else(|| {
        McpClientError::AuthRequired("MCP OAuth did not produce an authorized session".to_string())
    })?;
    manager
        .get_access_token()
        .await
        .map(|access_token| {
            Some(OAuthAccessToken {
                access_token,
                stored_credentials: None,
            })
        })
        .map_err(|error| {
            McpClientError::AuthRequired(format!("MCP OAuth token unavailable: {error}"))
        })
}

async fn stored_oauth_access_token(
    url: &str,
    credentials: &McpOAuthStoredCredentials,
) -> Result<OAuthAccessToken, McpClientError> {
    let token_response: OAuthTokenResponse =
        serde_json::from_value(credentials.token_response.clone()).map_err(|error| {
            McpClientError::AuthRequired(format!("MCP OAuth credentials are invalid: {error}"))
        })?;
    let should_refresh = stored_oauth_credentials_need_refresh(credentials, now_epoch_seconds());
    let mut oauth_state = oauth_state_for_mcp_url(url).await.map_err(|error| {
        McpClientError::AuthRequired(format!("MCP OAuth initialization failed: {error}"))
    })?;
    oauth_state
        .set_credentials(&credentials.client_id, token_response)
        .await
        .map_err(|error| {
            McpClientError::AuthRequired(format!("MCP OAuth credentials failed: {error}"))
        })?;
    let manager = oauth_state.into_authorization_manager().ok_or_else(|| {
        McpClientError::AuthRequired("MCP OAuth did not produce an authorized session".to_string())
    })?;
    if should_refresh {
        manager.refresh_token().await.map_err(|error| {
            McpClientError::AuthRequired(format!("MCP OAuth token refresh failed: {error}"))
        })?;
    }
    let access_token = manager.get_access_token().await.map_err(|error| {
        McpClientError::AuthRequired(format!("MCP OAuth token unavailable: {error}"))
    })?;
    let stored_credentials = if should_refresh {
        Some(refreshed_oauth_credentials(&manager, credentials).await?)
    } else {
        None
    };
    Ok(OAuthAccessToken {
        access_token,
        stored_credentials,
    })
}

async fn refreshed_oauth_credentials(
    manager: &rmcp::transport::auth::AuthorizationManager,
    previous: &McpOAuthStoredCredentials,
) -> Result<McpOAuthStoredCredentials, McpClientError> {
    let (client_id, token_response) = manager.get_credentials().await.map_err(|error| {
        McpClientError::AuthRequired(format!(
            "MCP OAuth refreshed credentials unavailable: {error}"
        ))
    })?;
    let token_response = token_response.ok_or_else(|| {
        McpClientError::AuthRequired(
            "MCP OAuth refresh did not return token credentials".to_string(),
        )
    })?;
    let token_response = serde_json::to_value(token_response).map_err(|error| {
        McpClientError::AuthRequired(format!(
            "MCP OAuth refreshed credentials could not be stored: {error}"
        ))
    })?;
    Ok(McpOAuthStoredCredentials {
        client_id,
        token_response: preserve_refresh_token(token_response, &previous.token_response),
        token_received_at: Some(now_epoch_seconds()),
    })
}

fn stored_oauth_credentials_need_refresh(
    credentials: &McpOAuthStoredCredentials,
    now_epoch_seconds: u64,
) -> bool {
    let Some(expires_in) = token_expires_in_seconds(&credentials.token_response) else {
        return false;
    };
    let Some(received_at) = credentials.token_received_at else {
        return token_response_has_refresh_token(&credentials.token_response);
    };
    let elapsed = now_epoch_seconds.saturating_sub(received_at);
    let remaining = expires_in.saturating_sub(elapsed);
    remaining < OAUTH_REFRESH_SKEW_SECONDS
}

fn token_expires_in_seconds(token_response: &Value) -> Option<u64> {
    token_response.get("expires_in").and_then(Value::as_u64)
}

fn token_response_has_refresh_token(token_response: &Value) -> bool {
    token_response
        .get("refresh_token")
        .and_then(Value::as_str)
        .is_some_and(|token| !token.is_empty())
}

fn preserve_refresh_token(mut updated: Value, previous: &Value) -> Value {
    let Some(previous_refresh_token) = previous.get("refresh_token").cloned() else {
        return updated;
    };
    let Some(object) = updated.as_object_mut() else {
        return updated;
    };
    object
        .entry("refresh_token".to_string())
        .or_insert(previous_refresh_token);
    updated
}

fn rmcp_initialize_error(error: rmcp::service::ClientInitializeError) -> McpClientError {
    let message = error.to_string();
    if message.contains("AuthRequired") || message.contains("Auth required") {
        McpClientError::AuthRequired(
            "MCP Streamable HTTP server requires authentication".to_string(),
        )
    } else {
        McpClientError::Transport(format!("MCP Streamable HTTP initialize failed: {message}"))
    }
}

fn rmcp_headers(
    headers: &BTreeMap<String, String>,
) -> Result<HashMap<RmcpHeaderName, RmcpHeaderValue>, McpClientError> {
    headers
        .iter()
        .map(|(key, value)| {
            let name = RmcpHeaderName::from_bytes(key.as_bytes()).map_err(|error| {
                McpClientError::Transport(format!("invalid MCP HTTP header name {key}: {error}"))
            })?;
            let value = RmcpHeaderValue::from_str(value).map_err(|error| {
                McpClientError::Transport(format!(
                    "invalid MCP HTTP header value for {key}: {error}"
                ))
            })?;
            Ok((name, value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn streamable_from_server_config_merges_safe_headers_and_secret_headers() {
        let server = McpServerRecord {
            mcp_server_id: "mcp:remote".to_string(),
            display_name: "Remote".to_string(),
            transport_kind: crate::McpTransportKind::StreamableHttp,
            safe_config: json!({
                "url": "https://example.com/mcp",
                "headers": { "X-Team": "infra", "Authorization": "placeholder" }
            }),
            enabled: true,
            health_status: crate::McpServerHealthStatus::Unknown,
            auth_status: crate::McpServerAuthStatus::None,
            tool_count: 0,
            authority_generation: "test-generation".to_string(),
        };
        let secrets = McpSecretMaterial {
            env: BTreeMap::new(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer secret".to_string())]),
            oauth_client_credentials: None,
            oauth_credentials: None,
        };

        let transport =
            StreamableHttpMcpTransport::from_server_config(&server, &secrets).expect("transport");

        assert_eq!(transport.url, "https://example.com/mcp");
        assert_eq!(
            transport.headers.get("X-Team").map(String::as_str),
            Some("infra")
        );
        assert_eq!(
            transport.headers.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }

    #[test]
    fn stored_oauth_credentials_need_refresh_when_received_token_is_expiring() {
        let credentials = McpOAuthStoredCredentials {
            client_id: "client".to_string(),
            token_response: json!({
                "access_token": "old-token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": "refresh-token"
            }),
            token_received_at: Some(1_000),
        };

        assert!(!stored_oauth_credentials_need_refresh(&credentials, 1_600));
        assert!(stored_oauth_credentials_need_refresh(&credentials, 4_571));
    }

    #[test]
    fn stored_oauth_credentials_refresh_legacy_expiring_token_when_refresh_token_exists() {
        let credentials = McpOAuthStoredCredentials {
            client_id: "client".to_string(),
            token_response: json!({
                "access_token": "legacy-token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": "refresh-token"
            }),
            token_received_at: None,
        };

        assert!(stored_oauth_credentials_need_refresh(&credentials, 1_000));
    }
}
