//! HTTP MCP metadata transports.

use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    time::Duration,
};

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use http::{HeaderName as RmcpHeaderName, HeaderValue as RmcpHeaderValue};
use reqwest::{
    StatusCode,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use rmcp::{
    ServiceExt,
    transport::{
        ClientCredentialsConfig, StreamableHttpClientTransport, auth::OAuthState,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Map, Value, json};
use tokio::time;

use crate::{
    McpServerRecord,
    mcp::{
        client::{
            DiscoveredMcpTool, McpClientError, McpTransport, discovered_tool_from_rmcp,
            parse_tools_list_result,
        },
        secrets::{McpOAuthClientCredentials, McpSecretMaterial},
    },
};

const SSE_PROTOCOL_VERSION: &str = "2024-11-05";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";

type SseByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

/// Metadata-only MCP transport over Streamable HTTP.
pub struct StreamableHttpMcpTransport {
    url: String,
    headers: BTreeMap<String, String>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
    discovered_tools: Option<Vec<DiscoveredMcpTool>>,
}

impl StreamableHttpMcpTransport {
    /// Create a Streamable HTTP MCP metadata transport.
    #[must_use]
    pub const fn new(url: String, headers: BTreeMap<String, String>) -> Self {
        Self {
            url,
            headers,
            oauth_client_credentials: None,
            discovered_tools: None,
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
            discovered_tools: None,
        })
    }
}

impl McpTransport for StreamableHttpMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.clone())
            .custom_headers(rmcp_headers(&self.headers)?);
        if let Some(token) =
            oauth_access_token(&self.url, self.oauth_client_credentials.as_ref()).await?
        {
            config = config.auth_header(token);
        }
        let transport = StreamableHttpClientTransport::from_config(config);
        let mut service = ().serve(transport).await.map_err(rmcp_initialize_error)?;
        let tools = service
            .peer()
            .list_all_tools()
            .await
            .map_err(|error| McpClientError::Transport(format!("MCP tools/list failed: {error}")))?
            .into_iter()
            .map(discovered_tool_from_rmcp)
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
}

/// Metadata-only MCP transport over legacy HTTP+SSE.
pub struct SseMcpTransport {
    url: String,
    headers: BTreeMap<String, String>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
    client: reqwest::Client,
    endpoint_url: Option<String>,
    stream: Option<SseByteStream>,
    stream_buffer: String,
    next_id: u64,
}

impl SseMcpTransport {
    /// Create a legacy SSE MCP metadata transport.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be created.
    pub fn new(url: String, headers: BTreeMap<String, String>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|error| format!("failed to build MCP SSE client: {error}"))?;
        Ok(Self {
            url,
            headers,
            oauth_client_credentials: None,
            client,
            endpoint_url: None,
            stream: None,
            stream_buffer: String::new(),
            next_id: 1,
        })
    }

    /// Build a legacy SSE transport from persisted safe config and disk-backed secrets.
    ///
    /// # Errors
    ///
    /// Returns an error when the persisted config does not have the expected
    /// HTTP shape.
    pub fn from_server_config(
        server: &McpServerRecord,
        secrets: &McpSecretMaterial,
    ) -> Result<Self, String> {
        let config = http_config_from_server(server, secrets, "sse")?;
        let mut transport = Self::new(config.url, config.headers)?;
        transport.oauth_client_credentials = config.oauth_client_credentials;
        Ok(transport)
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn ensure_connected(&mut self) -> Result<(), McpClientError> {
        if self.endpoint_url.is_some() {
            return Ok(());
        }

        let response = self
            .client
            .get(&self.url)
            .headers(sse_get_headers(&self.headers)?)
            .send()
            .await
            .map_err(|error| McpClientError::Transport(format!("MCP SSE GET failed: {error}")))?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(McpClientError::AuthRequired(
                "MCP SSE server requires authentication".to_string(),
            ));
        }
        if !response.status().is_success() {
            return Err(McpClientError::Transport(format!(
                "MCP SSE GET returned status {}",
                response.status()
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !content_type
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
        {
            return Err(McpClientError::Malformed(
                "MCP SSE endpoint did not return text/event-stream".to_string(),
            ));
        }

        self.stream = Some(Box::pin(response.bytes_stream()));
        loop {
            let event = self.read_sse_event().await?;
            if event.event.as_deref() == Some("endpoint") {
                let endpoint = event.data.ok_or_else(|| {
                    McpClientError::Malformed(
                        "MCP SSE endpoint event did not include data".to_string(),
                    )
                })?;
                self.endpoint_url = Some(resolve_endpoint_url(&self.url, &endpoint)?);
                return Ok(());
            }
        }
    }

    async fn post_message(&mut self, message: Value) -> Result<(), McpClientError> {
        let endpoint = self.endpoint_url.as_ref().ok_or_else(|| {
            McpClientError::Transport("MCP SSE message endpoint is not initialized".to_string())
        })?;
        let response = self
            .client
            .post(endpoint)
            .headers(sse_post_headers(&self.headers)?)
            .json(&message)
            .send()
            .await
            .map_err(|error| McpClientError::Transport(format!("MCP SSE POST failed: {error}")))?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(McpClientError::AuthRequired(
                "MCP SSE server requires authentication".to_string(),
            ));
        }
        if !response.status().is_success() {
            return Err(McpClientError::Transport(format!(
                "MCP SSE POST returned status {}",
                response.status()
            )));
        }
        Ok(())
    }

    async fn read_response(&mut self, id: u64, method: &str) -> Result<Value, McpClientError> {
        loop {
            let event = self.read_sse_event().await?;
            if !matches!(event.event.as_deref(), None | Some("") | Some("message")) {
                continue;
            }
            let Some(data) = event.data else {
                continue;
            };
            let response: Value = serde_json::from_str(&data).map_err(|error| {
                McpClientError::Malformed(format!("invalid MCP SSE JSON-RPC event: {error}"))
            })?;
            let Some(response_id) = response.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if response_id != id {
                continue;
            }
            return parse_json_rpc_response(response, method);
        }
    }

    async fn read_sse_event(&mut self) -> Result<SseEvent, McpClientError> {
        loop {
            if let Some(event) = next_event_from_buffer(&mut self.stream_buffer)? {
                return Ok(event);
            }
            let stream = self.stream.as_mut().ok_or_else(|| {
                McpClientError::Transport("MCP SSE stream is not connected".to_string())
            })?;
            let next_chunk = time::timeout(DEFAULT_TIMEOUT, stream.next())
                .await
                .map_err(|_| McpClientError::Transport("timed out reading MCP SSE".to_string()))?
                .ok_or_else(|| {
                    McpClientError::Transport("MCP SSE stream closed unexpectedly".to_string())
                })?
                .map_err(|error| {
                    McpClientError::Transport(format!("failed to read MCP SSE: {error}"))
                })?;
            let text = std::str::from_utf8(&next_chunk).map_err(|error| {
                McpClientError::Malformed(format!("MCP SSE stream was not UTF-8: {error}"))
            })?;
            self.stream_buffer.push_str(text);
        }
    }
}

impl McpTransport for SseMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        if let Some(token) =
            oauth_access_token(&self.url, self.oauth_client_credentials.as_ref()).await?
        {
            self.headers
                .insert("Authorization".to_string(), format!("Bearer {token}"));
        }
        self.ensure_connected().await?;
        let id = self.next_request_id();
        self.post_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": SSE_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "noema",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }))
        .await?;
        let _result = self.read_response(id, "initialize").await?;
        self.post_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }))
        .await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
        let mut tools = Vec::new();
        let mut cursor = None;
        loop {
            let id = self.next_request_id();
            let params = match &cursor {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            self.post_message(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/list",
                "params": params
            }))
            .await?;
            let result = self.read_response(id, "tools/list").await?;
            let page = parse_tools_list_result(result)?;
            tools.extend(page.tools);
            match page.next_cursor {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => return Ok(tools),
            }
        }
    }
}

struct HttpConfig {
    url: String,
    headers: BTreeMap<String, String>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
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
    })
}

async fn oauth_access_token(
    url: &str,
    credentials: Option<&McpOAuthClientCredentials>,
) -> Result<Option<String>, McpClientError> {
    let Some(credentials) = credentials else {
        return Ok(None);
    };
    let mut oauth_state = OAuthState::new(url, None).await.map_err(|error| {
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
    manager.get_access_token().await.map(Some).map_err(|error| {
        McpClientError::AuthRequired(format!("MCP OAuth token unavailable: {error}"))
    })
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

fn sse_get_headers(headers: &BTreeMap<String, String>) -> Result<HeaderMap, McpClientError> {
    let mut map = HeaderMap::new();
    map.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    insert_custom_headers(&mut map, headers)?;
    Ok(map)
}

fn sse_post_headers(headers: &BTreeMap<String, String>) -> Result<HeaderMap, McpClientError> {
    let mut map = HeaderMap::new();
    map.insert(ACCEPT, HeaderValue::from_static("application/json"));
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    map.insert(
        HeaderName::from_static(MCP_PROTOCOL_VERSION_HEADER),
        HeaderValue::from_static(SSE_PROTOCOL_VERSION),
    );
    insert_custom_headers(&mut map, headers)?;
    Ok(map)
}

fn insert_custom_headers(
    map: &mut HeaderMap,
    headers: &BTreeMap<String, String>,
) -> Result<(), McpClientError> {
    for (key, value) in headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            McpClientError::Transport(format!("invalid MCP HTTP header name {key}: {error}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|error| {
            McpClientError::Transport(format!("invalid MCP HTTP header value for {key}: {error}"))
        })?;
        map.insert(name, value);
    }
    Ok(())
}

fn resolve_endpoint_url(base_url: &str, endpoint: &str) -> Result<String, McpClientError> {
    let base = reqwest::Url::parse(base_url)
        .map_err(|error| McpClientError::Transport(format!("invalid MCP SSE base URL: {error}")))?;
    base.join(endpoint)
        .map(|url| url.to_string())
        .map_err(|error| {
            McpClientError::Transport(format!("invalid MCP SSE message endpoint: {error}"))
        })
}

fn next_event_from_buffer(buffer: &mut String) -> Result<Option<SseEvent>, McpClientError> {
    let Some(end) = buffer.find("\n\n") else {
        return Ok(None);
    };
    let raw = buffer[..end].to_string();
    buffer.replace_range(..end + 2, "");
    parse_sse_event(&raw).map(Some)
}

#[derive(Debug, PartialEq, Eq)]
struct SseEvent {
    event: Option<String>,
    data: Option<String>,
}

fn parse_sse_event(raw: &str) -> Result<SseEvent, McpClientError> {
    let mut event = None;
    let mut data_lines = Vec::new();
    for line in raw.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim_start().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim_start().to_string());
        }
    }
    Ok(SseEvent {
        event,
        data: (!data_lines.is_empty()).then(|| data_lines.join("\n")),
    })
}

fn parse_json_rpc_response(response: Value, method: &str) -> Result<Value, McpClientError> {
    if let Some(error) = response.get("error") {
        return Err(McpClientError::Transport(format!(
            "MCP {method} failed: {}",
            json_rpc_error_message(error)
        )));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| McpClientError::Malformed(format!("MCP {method} response missing result")))
}

fn json_rpc_error_message(error: &Value) -> String {
    let Some(object) = error.as_object() else {
        return error.to_string();
    };
    let code = object.get("code").and_then(Value::as_i64);
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown JSON-RPC error");
    match code {
        Some(code) => format!("{code}: {message}"),
        None => message.to_string(),
    }
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("MCP config field {field} must be a string"))
}

fn string_map_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<BTreeMap<String, String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|string| (key.clone(), string.to_string()))
                    .ok_or_else(|| format!("MCP config field {field} must contain string values"))
            })
            .collect(),
        Some(_) => Err(format!("MCP config field {field} must be an object")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_sse_endpoint_event() {
        let parsed =
            parse_sse_event("event: endpoint\ndata: /messages?session=abc\n\n").expect("parsed");

        assert_eq!(
            parsed,
            SseEvent {
                event: Some("endpoint".to_string()),
                data: Some("/messages?session=abc".to_string())
            }
        );
    }

    #[test]
    fn resolves_relative_sse_endpoint() {
        let resolved =
            resolve_endpoint_url("https://example.com/mcp/sse", "../messages").expect("resolved");

        assert_eq!(resolved, "https://example.com/messages");
    }

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
        };
        let secrets = McpSecretMaterial {
            env: BTreeMap::new(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer secret".to_string())]),
            oauth_client_credentials: None,
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
    fn sse_from_server_config_merges_safe_headers_and_secret_headers() {
        let server = McpServerRecord {
            mcp_server_id: "mcp:remote".to_string(),
            display_name: "Remote".to_string(),
            transport_kind: crate::McpTransportKind::Sse,
            safe_config: json!({
                "url": "https://example.com/sse",
                "headers": { "X-Team": "infra" }
            }),
            enabled: true,
            health_status: crate::McpServerHealthStatus::Unknown,
            auth_status: crate::McpServerAuthStatus::None,
            tool_count: 0,
        };
        let secrets = McpSecretMaterial {
            env: BTreeMap::new(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer secret".to_string())]),
            oauth_client_credentials: None,
        };

        let transport = SseMcpTransport::from_server_config(&server, &secrets).expect("transport");

        assert_eq!(transport.url, "https://example.com/sse");
        assert_eq!(
            transport.headers.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }
}
