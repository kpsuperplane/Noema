//! HTTP/SSE MCP metadata transport.

use std::{collections::BTreeMap, time::Duration};

use reqwest::{
    StatusCode,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Map, Value, json};

use crate::{
    McpServerRecord,
    mcp::{
        client::{DiscoveredMcpTool, McpClientError, McpTransport, parse_tools_list_result},
        secrets::McpSecretMaterial,
    },
};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";
const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";

/// Metadata-only MCP transport over HTTP responses or SSE event streams.
pub struct HttpSseMcpTransport {
    url: String,
    headers: BTreeMap<String, String>,
    client: reqwest::Client,
    session_id: Option<String>,
    next_id: u64,
}

impl HttpSseMcpTransport {
    /// Create an HTTP/SSE MCP metadata transport.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be created.
    pub fn new(url: String, headers: BTreeMap<String, String>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|error| format!("failed to build MCP HTTP client: {error}"))?;
        Ok(Self {
            url,
            headers,
            client,
            session_id: None,
            next_id: 1,
        })
    }

    /// Build an HTTP/SSE transport from persisted safe config and disk-backed secrets.
    ///
    /// # Errors
    ///
    /// Returns an error when the persisted config does not have the expected
    /// HTTP/SSE shape.
    pub fn from_server_config(
        server: &McpServerRecord,
        secrets: &McpSecretMaterial,
    ) -> Result<Self, String> {
        let object = server
            .safe_config
            .as_object()
            .ok_or_else(|| "MCP http_sse config must be an object".to_string())?;
        let url = string_field(object, "url")?;
        let mut headers = string_map_field(object, "headers")?;
        headers.extend(secrets.headers.clone());
        Self::new(url, headers)
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn send_request(&mut self, message: Value) -> Result<Option<Value>, McpClientError> {
        let mut headers = request_headers(&self.headers)?;
        if let Some(session_id) = &self.session_id {
            headers.insert(
                HeaderName::from_static(MCP_SESSION_ID_HEADER),
                HeaderValue::from_str(session_id).map_err(|error| {
                    McpClientError::Transport(format!("invalid MCP session id header: {error}"))
                })?,
            );
        }

        let response = self
            .client
            .post(&self.url)
            .headers(headers)
            .json(&message)
            .send()
            .await
            .map_err(|error| {
                McpClientError::Transport(format!("MCP HTTP request failed: {error}"))
            })?;

        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(McpClientError::AuthRequired(
                "MCP HTTP server requires authentication".to_string(),
            ));
        }
        if response.status() == StatusCode::ACCEPTED {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(McpClientError::Transport(format!(
                "MCP HTTP request returned status {}",
                response.status()
            )));
        }

        if self.session_id.is_none() {
            self.session_id = response
                .headers()
                .get(MCP_SESSION_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let text = response.text().await.map_err(|error| {
            McpClientError::Transport(format!("failed to read MCP HTTP response: {error}"))
        })?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        parse_http_response_body(content_type.as_deref(), &text)
    }

    async fn send_request_for_result(
        &mut self,
        id: u64,
        method: &str,
        message: Value,
    ) -> Result<Value, McpClientError> {
        let response = self.send_request(message).await?.ok_or_else(|| {
            McpClientError::Malformed(format!("MCP HTTP {method} response missing body"))
        })?;
        parse_json_rpc_response(response, id, method)
    }
}

impl McpTransport for HttpSseMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let id = self.next_request_id();
        self.send_request_for_result(
            id,
            "initialize",
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "noema",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
        )
        .await?;
        let _accepted = self
            .send_request(json!({
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
            let result = self
                .send_request_for_result(
                    id,
                    "tools/list",
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": "tools/list",
                        "params": params
                    }),
                )
                .await?;
            let page = parse_tools_list_result(result)?;
            tools.extend(page.tools);
            match page.next_cursor {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => return Ok(tools),
            }
        }
    }
}

fn request_headers(headers: &BTreeMap<String, String>) -> Result<HeaderMap, McpClientError> {
    let mut map = HeaderMap::new();
    map.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    map.insert(
        HeaderName::from_static(MCP_PROTOCOL_VERSION_HEADER),
        HeaderValue::from_static(MCP_PROTOCOL_VERSION),
    );
    for (key, value) in headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            McpClientError::Transport(format!("invalid MCP HTTP header name {key}: {error}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|error| {
            McpClientError::Transport(format!("invalid MCP HTTP header value for {key}: {error}"))
        })?;
        map.insert(name, value);
    }
    Ok(map)
}

fn parse_http_response_body(
    content_type: Option<&str>,
    text: &str,
) -> Result<Option<Value>, McpClientError> {
    if content_type
        .unwrap_or_default()
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
    {
        return parse_event_stream_response(text).map(Some);
    }
    serde_json::from_str(text)
        .map(Some)
        .map_err(|error| McpClientError::Malformed(format!("invalid MCP HTTP JSON: {error}")))
}

fn parse_event_stream_response(text: &str) -> Result<Value, McpClientError> {
    for event in text.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if data.trim().is_empty() {
            continue;
        }
        return serde_json::from_str(&data).map_err(|error| {
            McpClientError::Malformed(format!("invalid MCP SSE JSON data: {error}"))
        });
    }
    Err(McpClientError::Malformed(
        "MCP SSE response did not include data".to_string(),
    ))
}

fn parse_json_rpc_response(
    response: Value,
    id: u64,
    method: &str,
) -> Result<Value, McpClientError> {
    if response.get("id").and_then(Value::as_u64) != Some(id) {
        return Err(McpClientError::Malformed(format!(
            "MCP {method} response id did not match request"
        )));
    }
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
    fn parses_sse_json_rpc_response() {
        let parsed = parse_http_response_body(
            Some("text/event-stream; charset=utf-8"),
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n",
        )
        .expect("parsed")
        .expect("body");

        assert_eq!(parsed["result"], json!({ "ok": true }));
    }

    #[test]
    fn from_server_config_merges_safe_headers_and_secret_headers() {
        let server = McpServerRecord {
            mcp_server_id: "mcp:remote".to_string(),
            display_name: "Remote".to_string(),
            transport_kind: crate::McpTransportKind::HttpSse,
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
        };

        let transport =
            HttpSseMcpTransport::from_server_config(&server, &secrets).expect("transport");

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
}
