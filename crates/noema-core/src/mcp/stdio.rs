//! Stdio MCP metadata transport.

use std::{collections::BTreeMap, process::Stdio, time::Duration};

use serde_json::{Map, Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time,
};

use crate::{
    McpServerRecord,
    mcp::{
        client::{DiscoveredMcpTool, McpClientError, McpTransport, parse_tools_list_result},
        secrets::McpSecretMaterial,
    },
};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Metadata-only MCP transport over stdio.
pub struct StdioMcpTransport {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    timeout: Duration,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_id: u64,
}

impl StdioMcpTransport {
    /// Create a stdio MCP metadata transport.
    #[must_use]
    pub fn new(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            command,
            args,
            cwd,
            env,
            timeout: DEFAULT_TIMEOUT,
            child: None,
            stdin: None,
            stdout: None,
            next_id: 1,
        }
    }

    /// Build a stdio transport from persisted safe config and disk-backed secrets.
    ///
    /// # Errors
    ///
    /// Returns an error when the persisted config does not have the expected
    /// stdio shape.
    pub fn from_server_config(
        server: &McpServerRecord,
        secrets: &McpSecretMaterial,
    ) -> Result<Self, String> {
        let object = server
            .safe_config
            .as_object()
            .ok_or_else(|| "MCP stdio config must be an object".to_string())?;
        let command = string_field(object, "command")?;
        let args = string_array_field(object, "args")?;
        let cwd = optional_string_field(object, "cwd")?;
        let mut env = string_map_field(object, "env")?;
        env.extend(secrets.env.clone());
        Ok(Self::new(command, args, cwd, env))
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn ensure_started(&mut self) -> Result<(), McpClientError> {
        if self.child.is_some() {
            return Ok(());
        }

        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .envs(&self.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }

        let mut child = command.spawn().map_err(|error| {
            McpClientError::Transport(format!("failed to start MCP stdio command: {error}"))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            McpClientError::Transport("MCP stdio command did not expose stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            McpClientError::Transport("MCP stdio command did not expose stdout".to_string())
        })?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.child = Some(child);
        Ok(())
    }

    async fn send_message(&mut self, message: Value) -> Result<(), McpClientError> {
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            McpClientError::Transport("MCP stdio command is not started".to_string())
        })?;
        let mut bytes = serde_json::to_vec(&message).map_err(|error| {
            McpClientError::Malformed(format!("failed to encode MCP request: {error}"))
        })?;
        bytes.push(b'\n');
        time::timeout(self.timeout, stdin.write_all(&bytes))
            .await
            .map_err(|_| McpClientError::Transport("timed out writing MCP request".to_string()))?
            .map_err(|error| {
                McpClientError::Transport(format!("failed to write MCP request: {error}"))
            })?;
        time::timeout(self.timeout, stdin.flush())
            .await
            .map_err(|_| McpClientError::Transport("timed out flushing MCP request".to_string()))?
            .map_err(|error| {
                McpClientError::Transport(format!("failed to flush MCP request: {error}"))
            })?;
        Ok(())
    }

    async fn read_response(&mut self, id: u64, method: &str) -> Result<Value, McpClientError> {
        loop {
            let stdout = self.stdout.as_mut().ok_or_else(|| {
                McpClientError::Transport("MCP stdio command is not started".to_string())
            })?;
            let mut line = String::new();
            let bytes_read = time::timeout(self.timeout, stdout.read_line(&mut line))
                .await
                .map_err(|_| {
                    McpClientError::Transport(format!("timed out waiting for MCP {method}"))
                })?
                .map_err(|error| {
                    McpClientError::Transport(format!("failed to read MCP response: {error}"))
                })?;
            if bytes_read == 0 {
                return Err(McpClientError::Transport(format!(
                    "MCP server closed stdout while waiting for {method}"
                )));
            }

            let response: Value = serde_json::from_str(&line).map_err(|error| {
                McpClientError::Malformed(format!("invalid MCP JSON-RPC response: {error}"))
            })?;
            let Some(response_id) = response.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if response_id != id {
                continue;
            }

            if let Some(error) = response.get("error") {
                return Err(McpClientError::Transport(format!(
                    "MCP {method} failed: {}",
                    json_rpc_error_message(error)
                )));
            }

            return response.get("result").cloned().ok_or_else(|| {
                McpClientError::Malformed(format!("MCP {method} response missing result"))
            });
        }
    }
}

impl McpTransport for StdioMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        self.ensure_started()?;
        let id = self.next_request_id();
        self.send_message(json!({
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
        }))
        .await?;
        let _result = self.read_response(id, "initialize").await?;
        self.send_message(json!({
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
            self.send_message(json!({
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

fn optional_string_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("MCP config field {field} must be a string")),
    }
}

fn string_array_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, String> {
    match object.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| format!("MCP config field {field} must contain strings"))
            })
            .collect(),
        Some(_) => Err(format!("MCP config field {field} must be an array")),
    }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn stdio_transport_initializes_and_lists_tools() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *tools/list*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"read_doc","description":"Read a document","inputSchema":{"type":"object"},"outputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}}]}}'
      ;;
    *initialize*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"fake","version":"1"}}}'
      ;;
  esac
done
"#;
        let mut transport = StdioMcpTransport::new(
            "/bin/sh".to_string(),
            vec!["-c".to_string(), script.to_string()],
            None,
            BTreeMap::new(),
        );

        transport.initialize().await.expect("initialize");
        let tools = transport.list_tools().await.expect("tools");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_doc");
        assert_eq!(tools[0].description.as_deref(), Some("Read a document"));
        assert_eq!(tools[0].input_schema, json!({ "type": "object" }));
        assert_eq!(tools[0].output_schema, Some(json!({ "type": "object" })));
        assert_eq!(tools[0].annotations, json!({ "readOnlyHint": true }));
    }

    #[test]
    fn from_server_config_merges_safe_env_and_secret_env() {
        let server = McpServerRecord {
            mcp_server_id: "mcp:test".to_string(),
            display_name: "Test".to_string(),
            transport_kind: crate::McpTransportKind::Stdio,
            safe_config: json!({
                "command": "server",
                "args": ["--stdio"],
                "cwd": "/tmp",
                "env": { "VISIBLE": "yes", "TOKEN": "placeholder" }
            }),
            enabled: true,
            health_status: crate::McpServerHealthStatus::Unknown,
            auth_status: crate::McpServerAuthStatus::None,
            tool_count: 0,
        };
        let secrets = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]),
            headers: BTreeMap::new(),
        };

        let transport =
            StdioMcpTransport::from_server_config(&server, &secrets).expect("transport");

        assert_eq!(transport.command, "server");
        assert_eq!(transport.args, vec!["--stdio"]);
        assert_eq!(transport.cwd.as_deref(), Some("/tmp"));
        assert_eq!(
            transport.env.get("VISIBLE").map(String::as_str),
            Some("yes")
        );
        assert_eq!(
            transport.env.get("TOKEN").map(String::as_str),
            Some("secret")
        );
    }
}
