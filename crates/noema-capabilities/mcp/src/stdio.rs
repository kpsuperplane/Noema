//! Stdio MCP session preparation.

use std::fmt;

use process_wrap::tokio::{CommandWrap, KillOnDrop};
use rmcp::ServiceExt;
use tokio::process::Command;

use crate::{
    McpDiagnosticHandle, McpSecretMaterial, McpServerRecord, McpStdioSetupConfig, McpTransportKind,
    client::{
        McpClientError, McpClientFuture, McpClientResult, McpRequestContext, McpSessionFactory,
        McpSessionPreparation, RmcpPreparedSession, initialize_error, run_with_context,
    },
};

const TRANSPORT_KIND: &str = "stdio";

mod transport;

/// Factory for initialized stdio MCP sessions.
#[derive(Clone)]
pub struct StdioMcpSessionFactory {
    enabled: bool,
    diagnostics: Option<McpDiagnosticHandle>,
}

impl StdioMcpSessionFactory {
    /// Construct the stdio factory with startup authority and optional diagnostics.
    #[must_use]
    pub const fn new(enabled: bool, diagnostics: Option<McpDiagnosticHandle>) -> Self {
        Self {
            enabled,
            diagnostics,
        }
    }
}

impl fmt::Debug for StdioMcpSessionFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StdioMcpSessionFactory")
            .field("enabled", &self.enabled)
            .field("diagnostics_configured", &self.diagnostics.is_some())
            .finish()
    }
}

impl McpSessionFactory for StdioMcpSessionFactory {
    fn prepare<'a>(
        &'a self,
        server: &'a McpServerRecord,
        secrets: &'a McpSecretMaterial,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpSessionPreparation> {
        Box::pin(async move {
            context.check("initialize")?;
            if !self.enabled {
                return Err(McpClientError::Unavailable(
                    "MCP stdio transport is disabled by startup configuration".to_string(),
                ));
            }
            let config = stdio_config_from_server(server, secrets)?;
            let command = wrapped_stdio_command(stdio_command(&config));
            let transport = transport::spawn(command).map_err(|error| {
                McpClientError::Unavailable(format!("failed to start MCP stdio command: {error}"))
            })?;
            let service_cancellation = context.cancellation_token().child_token();
            let initialize = ().serve_with_ct(transport, service_cancellation.clone());
            let service = match run_with_context(context, "initialize", async {
                initialize
                    .await
                    .map_err(|error| initialize_error("initialize", error))
            })
            .await
            {
                Ok(service) => service,
                Err(error) => {
                    service_cancellation.cancel();
                    return Err(error);
                }
            };
            let session = RmcpPreparedSession::new(
                service,
                self.diagnostics.clone(),
                server.mcp_server_id.clone(),
                TRANSPORT_KIND,
            );
            Ok(McpSessionPreparation::new(Box::new(session), None))
        })
    }
}

fn wrapped_stdio_command(command: Command) -> CommandWrap {
    let mut command = CommandWrap::from(command);
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    {
        command.wrap(process_wrap::tokio::JobObject);
    }
    command
}

fn stdio_command(config: &McpStdioSetupConfig) -> Command {
    let mut command = Command::new(&config.command);
    command.env_clear();
    command
        .args(&config.args)
        .envs(&config.env)
        .kill_on_drop(true);
    if let Some(cwd) = &config.cwd {
        command.current_dir(cwd);
    }
    command
}

fn stdio_config_from_server(
    server: &McpServerRecord,
    secrets: &McpSecretMaterial,
) -> McpClientResult<McpStdioSetupConfig> {
    if server.transport_kind != McpTransportKind::Stdio {
        return Err(McpClientError::Malformed(
            "MCP server is not configured for stdio".to_string(),
        ));
    }
    let mut config: McpStdioSetupConfig = serde_json::from_value(server.safe_config.clone())
        .map_err(|_| McpClientError::Malformed("MCP stdio config is invalid".to_string()))?;
    if config.command.trim().is_empty() || config.command.chars().any(char::is_control) {
        return Err(McpClientError::Malformed(
            "MCP stdio command is invalid".to_string(),
        ));
    }
    config.env.extend(secrets.env.clone());
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        process::{Command as StdCommand, Stdio as StdStdio},
        time::Duration,
    };

    use serde_json::{Value, json};
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::test_fixture::server_record;

    fn server(safe_config: Value) -> McpServerRecord {
        server_record("mcp:stdio", McpTransportKind::Stdio, safe_config)
    }

    #[test]
    fn config_merges_safe_and_secret_environment_with_secret_precedence() {
        let server = server(json!({
            "command": "server",
            "args": ["--stdio"],
            "cwd": "/tmp",
            "env": { "VISIBLE": "yes", "TOKEN": "placeholder" }
        }));
        let secrets = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]),
            ..McpSecretMaterial::default()
        };

        let config = stdio_config_from_server(&server, &secrets).expect("config");

        assert_eq!(config.command, "server");
        assert_eq!(config.args, vec!["--stdio"]);
        assert_eq!(config.cwd.as_deref(), Some("/tmp"));
        assert_eq!(config.env["VISIBLE"], "yes");
        assert_eq!(config.env["TOKEN"], "secret");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn command_passes_only_declared_environment() {
        let config = McpStdioSetupConfig {
            command: "/usr/bin/env".to_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::from([("DECLARED".to_string(), "present".to_string())]),
        };

        let output = stdio_command(&config).output().await.expect("run command");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("utf8"),
            "DECLARED=present\n"
        );
    }

    #[tokio::test]
    async fn disabled_factory_rejects_before_reading_stdio_configuration() {
        let server = server(Value::Null);
        let context =
            McpRequestContext::with_timeout(Duration::from_secs(1), CancellationToken::new());

        let error = StdioMcpSessionFactory::new(false, None)
            .prepare(&server, &McpSecretMaterial::default(), &context)
            .await
            .expect_err("disabled stdio");

        assert_eq!(
            error,
            McpClientError::Unavailable(
                "MCP stdio transport is disabled by startup configuration".to_string()
            )
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prepared_stdio_session_discovers_tools_and_closes_child() {
        let directory = tempdir().expect("tempdir");
        let pid_file = directory.path().join("pid");
        let descendant_pid_file = directory.path().join("descendant-pid");
        let script = r#"
printf '%s' "$$" > "$PID_FILE"
sleep 30 &
printf '%s' "$!" > "$DESCENDANT_PID_FILE"
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
        let server = server(json!({
            "command": "/bin/sh",
            "args": ["-c", script],
            "env": {
                "PID_FILE": pid_file.to_string_lossy(),
                "DESCENDANT_PID_FILE": descendant_pid_file.to_string_lossy()
            }
        }));
        let context =
            McpRequestContext::with_timeout(Duration::from_secs(3), CancellationToken::new());
        let preparation = StdioMcpSessionFactory::new(true, None)
            .prepare(&server, &McpSecretMaterial::default(), &context)
            .await
            .expect("prepare");
        let (mut session, refreshed) = preparation.into_parts();

        let tools = session.discover_tools(&context).await.expect("tools");
        let pid = std::fs::read_to_string(&pid_file).expect("pid");
        let descendant_pid = wait_for_pid(&descendant_pid_file).await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_doc");
        assert!(refreshed.is_none());

        session.close().await.expect("close");
        wait_for_process_exit(pid.trim()).await;
        wait_for_process_exit(descendant_pid.trim()).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn initialization_timeout_still_terminates_stdio_child() {
        let directory = tempdir().expect("tempdir");
        let pid_file = directory.path().join("pid");
        let script = r#"
printf '%s' "$$" > "$PID_FILE"
trap '' TERM
while IFS= read -r _; do :; done
"#;
        let server = server(json!({
            "command": "/bin/sh",
            "args": ["-c", script],
            "env": { "PID_FILE": pid_file.to_string_lossy() }
        }));
        let context =
            McpRequestContext::with_timeout(Duration::from_millis(100), CancellationToken::new());

        let error = StdioMcpSessionFactory::new(true, None)
            .prepare(&server, &McpSecretMaterial::default(), &context)
            .await
            .expect_err("timeout");
        assert_eq!(
            error,
            McpClientError::Timeout {
                operation: "initialize"
            }
        );

        let pid = wait_for_pid(&pid_file).await;
        for _ in 0..40 {
            if !process_is_running(&pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("stdio child {pid} survived cancelled initialization");
    }

    #[cfg(unix)]
    async fn wait_for_pid(path: &std::path::Path) -> String {
        for _ in 0..40 {
            if let Ok(pid) = std::fs::read_to_string(path) {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("stdio child did not publish its pid");
    }

    #[cfg(unix)]
    async fn wait_for_process_exit(pid: &str) {
        for _ in 0..40 {
            if !process_is_running(pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("stdio process {pid} survived session termination");
    }

    #[cfg(unix)]
    fn process_is_running(pid: &str) -> bool {
        StdCommand::new("/bin/kill")
            .args(["-0", pid])
            .stdout(StdStdio::null())
            .stderr(StdStdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}
