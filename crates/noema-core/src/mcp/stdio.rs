//! Stdio MCP metadata transport.

use std::{collections::BTreeMap, process::Stdio};

use rmcp::{ServiceExt, transport::TokioChildProcess};
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{
    McpServerRecord,
    mcp::{
        client::{
            DiscoveredMcpTool, McpClientError, McpDiagnosticContext, McpTransport,
            call_tool_params, call_tool_result_value, discovered_tool_from_rmcp,
            optional_string_field, string_array_field, string_field, string_map_field,
        },
        secrets::McpSecretMaterial,
    },
};
use noema_home::SystemErrorLogger;

/// Metadata-only MCP transport over stdio.
pub struct StdioMcpTransport {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    discovered_tools: Option<Vec<DiscoveredMcpTool>>,
    diagnostics: Option<SystemErrorLogger>,
    diagnostic_mcp_server_id: Option<String>,
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
            discovered_tools: None,
            diagnostics: None,
            diagnostic_mcp_server_id: None,
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
            Some("stdio".to_string()),
            method,
        )
    }
}

impl McpTransport for StdioMcpTransport {
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let mut command = Command::new(&self.command);
        command.args(&self.args).envs(&self.env);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        let (transport, _stderr) = TokioChildProcess::builder(command)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                McpClientError::Transport(format!("failed to start MCP stdio command: {error}"))
            })?;
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
            McpClientError::Transport("MCP stdio transport is not initialized".to_string())
        })
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, McpClientError> {
        let mut command = Command::new(&self.command);
        command.args(&self.args).envs(&self.env);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        let (transport, _stderr) = TokioChildProcess::builder(command)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                McpClientError::Transport(format!("failed to start MCP stdio command: {error}"))
            })?;
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

fn rmcp_initialize_error(error: rmcp::service::ClientInitializeError) -> McpClientError {
    let message = error.to_string();
    if message.contains("AuthRequired") || message.contains("Auth required") {
        McpClientError::AuthRequired("MCP stdio server requires authentication".to_string())
    } else {
        McpClientError::Transport(format!("MCP stdio initialize failed: {message}"))
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
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"read_doc","description":"Read a document","inputSchema":{"type":"object"},"outputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}}]}}'
      ;;
    *initialize*)
      printf '%s\n' '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"fake","version":"1"}}}'
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
            oauth_client_credentials: None,
            oauth_credentials: None,
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
