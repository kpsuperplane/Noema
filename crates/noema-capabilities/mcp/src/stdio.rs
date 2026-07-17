//! Stdio MCP session preparation.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fmt,
};

#[cfg(unix)]
use process_wrap::tokio::CommandWrapper;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use rmcp::ServiceExt;
use serde_json::{Map, Value};
use tokio::process::Command;

use crate::{
    McpDiagnosticHandle, McpSecretMaterial, McpServerRecord, McpTransportKind,
    client::{
        McpClientError, McpClientFuture, McpClientResult, McpPreparedSession, McpRequestContext,
        McpSessionFactory, McpSessionPreparation, RmcpPreparedSession, initialize_error,
        run_with_context,
    },
};

const TRANSPORT_KIND: &str = "stdio";

mod transport;

/// Factory for initialized stdio MCP sessions.
#[derive(Clone)]
pub struct StdioMcpSessionFactory {
    diagnostics: Option<McpDiagnosticHandle>,
}

impl StdioMcpSessionFactory {
    /// Construct the stdio factory with optional developer diagnostics.
    #[must_use]
    pub const fn new(diagnostics: Option<McpDiagnosticHandle>) -> Self {
        Self { diagnostics }
    }
}

impl fmt::Debug for StdioMcpSessionFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StdioMcpSessionFactory")
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
            let config = stdio_config_from_server(server, secrets)?;
            #[cfg(unix)]
            let process_group_capture = ProcessGroupCapture::new();
            let command = wrapped_stdio_command(
                stdio_command(&config, std::env::vars_os()),
                #[cfg(unix)]
                process_group_capture.clone(),
            );
            let transport = transport::spawn(command).map_err(|error| {
                McpClientError::Unavailable(format!("failed to start MCP stdio command: {error}"))
            })?;
            #[cfg(unix)]
            let process_group = process_group_capture.guard()?;
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
            #[cfg(unix)]
            let session = StdioPreparedSession::new(session, process_group);
            Ok(McpSessionPreparation::new(Box::new(session), None))
        })
    }
}

struct StdioConfig {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
}

fn wrapped_stdio_command(
    command: Command,
    #[cfg(unix)] process_group_capture: ProcessGroupCapture,
) -> CommandWrap {
    let mut command = CommandWrap::from(command);
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    {
        command.wrap(process_wrap::tokio::ProcessGroup::leader());
        command.wrap(process_group_capture);
    }
    #[cfg(windows)]
    {
        command.wrap(process_wrap::tokio::JobObject);
    }
    command
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct ProcessGroupCapture {
    pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

#[cfg(unix)]
impl ProcessGroupCapture {
    fn new() -> Self {
        Self {
            pid: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    fn guard(&self) -> McpClientResult<ProcessGroupGuard> {
        let pgid = self.pid.load(std::sync::atomic::Ordering::Acquire);
        if pgid == 0 || i32::try_from(pgid).is_err() {
            return Err(McpClientError::Unavailable(
                "failed to capture MCP stdio process group".to_string(),
            ));
        }
        Ok(ProcessGroupGuard { pgid, armed: true })
    }
}

#[cfg(unix)]
impl CommandWrapper for ProcessGroupCapture {
    fn post_spawn(
        &mut self,
        _command: &mut Command,
        child: &mut tokio::process::Child,
        _core: &CommandWrap,
    ) -> std::io::Result<()> {
        let pid = child
            .id()
            .ok_or_else(|| std::io::Error::other("spawned MCP stdio child has no process ID"))?;
        self.pid.store(pid, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

#[cfg(unix)]
struct ProcessGroupGuard {
    pgid: u32,
    armed: bool,
}

#[cfg(unix)]
impl ProcessGroupGuard {
    fn terminate(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        let Ok(pgid) = i32::try_from(self.pgid) else {
            return;
        };
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pgid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(unix)]
struct StdioPreparedSession {
    inner: Option<RmcpPreparedSession>,
    process_group: ProcessGroupGuard,
}

#[cfg(unix)]
impl StdioPreparedSession {
    fn new(inner: RmcpPreparedSession, process_group: ProcessGroupGuard) -> Self {
        Self {
            inner: Some(inner),
            process_group,
        }
    }
}

#[cfg(unix)]
impl McpPreparedSession for StdioPreparedSession {
    fn discover_tools<'a>(
        &'a mut self,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, Vec<crate::McpDiscoveredTool>> {
        self.inner
            .as_mut()
            .expect("stdio session remains present until close")
            .discover_tools(context)
    }

    fn call_tool<'a>(
        &'a mut self,
        tool_name: &'a str,
        arguments: Value,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, crate::client::McpToolCallOutput> {
        self.inner
            .as_mut()
            .expect("stdio session remains present until close")
            .call_tool(tool_name, arguments, context)
    }

    fn close(mut self: Box<Self>) -> McpClientFuture<'static, ()> {
        let inner = self
            .inner
            .take()
            .expect("stdio session remains present until close");
        Box::pin(async move {
            let result = Box::new(inner).close().await;
            self.process_group.terminate();
            result
        })
    }
}

fn stdio_command(
    config: &StdioConfig,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Command {
    let mut command = Command::new(&config.command);
    command.env_clear();
    for (key, value) in inherited {
        if inherited_env_allowed(&key) {
            command.env(key, value);
        }
    }
    command
        .args(&config.args)
        .envs(&config.env)
        .kill_on_drop(true);
    if let Some(cwd) = &config.cwd {
        command.current_dir(cwd);
    }
    command
}

fn inherited_env_allowed(key: &OsStr) -> bool {
    const PORTABLE: &[&str] = &["PATH", "HOME", "TMPDIR", "TEMP", "TMP"];
    if PORTABLE.iter().any(|allowed| key == OsStr::new(allowed)) {
        return true;
    }
    #[cfg(windows)]
    {
        const WINDOWS: &[&str] = &["PATHEXT", "SYSTEMROOT", "COMSPEC", "USERPROFILE"];
        if WINDOWS.iter().any(|allowed| key == OsStr::new(allowed)) {
            return true;
        }
    }
    false
}

fn stdio_config_from_server(
    server: &McpServerRecord,
    secrets: &McpSecretMaterial,
) -> McpClientResult<StdioConfig> {
    if server.transport_kind != McpTransportKind::Stdio {
        return Err(McpClientError::Malformed(
            "MCP server is not configured for stdio".to_string(),
        ));
    }
    let object = server.safe_config.as_object().ok_or_else(|| {
        McpClientError::Malformed("MCP stdio config must be an object".to_string())
    })?;
    let command = string_field(object, "command")?;
    if command.trim().is_empty() || command.chars().any(char::is_control) {
        return Err(McpClientError::Malformed(
            "MCP stdio command is invalid".to_string(),
        ));
    }
    let args = string_array_field(object, "args")?;
    let cwd = optional_string_field(object, "cwd")?;
    let mut env = string_map_field(object, "env")?;
    env.extend(secrets.env.clone());
    Ok(StdioConfig {
        command,
        args,
        cwd,
        env,
    })
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> McpClientResult<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            McpClientError::Malformed(format!("MCP config field {field} must be a string"))
        })
}

fn optional_string_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> McpClientResult<Option<String>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(McpClientError::Malformed(format!(
            "MCP config field {field} must be a string"
        ))),
    }
}

fn string_array_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> McpClientResult<Vec<String>> {
    match object.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(ToString::to_string).ok_or_else(|| {
                    McpClientError::Malformed(format!(
                        "MCP config field {field} must contain strings"
                    ))
                })
            })
            .collect(),
        Some(_) => Err(McpClientError::Malformed(format!(
            "MCP config field {field} must be an array"
        ))),
    }
}

fn string_map_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> McpClientResult<BTreeMap<String, String>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|value| (key.clone(), value.to_string()))
                    .ok_or_else(|| {
                        McpClientError::Malformed(format!(
                            "MCP config field {field} must contain string values"
                        ))
                    })
            })
            .collect(),
        Some(_) => Err(McpClientError::Malformed(format!(
            "MCP config field {field} must be an object"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        process::{Command as StdCommand, Stdio as StdStdio},
        time::Duration,
    };

    use serde_json::json;
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{McpServerAuthStatus, McpServerHealthStatus};

    fn server(safe_config: Value) -> McpServerRecord {
        McpServerRecord {
            mcp_server_id: "mcp:stdio".to_string(),
            display_name: "Local".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config,
            enabled: true,
            health_status: McpServerHealthStatus::Unknown,
            auth_status: McpServerAuthStatus::None,
            tool_count: 0,
            authority_generation: "generation".to_string(),
        }
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
    async fn command_inherits_only_minimal_allowlist_plus_declared_environment() {
        let config = StdioConfig {
            command: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf '%s|%s|%s' \"${PRIVATE_SENTINEL-unset}\" \"$DECLARED\" \"$PATH\""
                    .to_string(),
            ],
            cwd: None,
            env: BTreeMap::from([("DECLARED".to_string(), "present".to_string())]),
        };
        let inherited = [
            (
                OsString::from("PRIVATE_SENTINEL"),
                OsString::from("must-not-leak"),
            ),
            (OsString::from("PATH"), OsString::from("/test/bin")),
        ];

        let output = stdio_command(&config, inherited)
            .output()
            .await
            .expect("run command");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("utf8"),
            "unset|present|/test/bin"
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
        let preparation = StdioMcpSessionFactory::new(None)
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

        let error = StdioMcpSessionFactory::new(None)
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
