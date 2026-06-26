//! Warm Codex app-server runtime.

use crate::{
    provider::{
        GenerateResponse, ProviderError, output_items_from_text, required_output_items_from_text,
    },
    providers::codex::CodexProviderConfig,
};
use serde_json::{Value, json};
use std::{io::ErrorKind, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time,
};

/// Conversation state owned by the Codex app-server runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAppServerConversation {
    /// Codex thread id.
    pub thread_id: String,
    /// Model selected for this conversation.
    pub model: Option<String>,
}

/// Runtime that keeps a Codex `app-server` subprocess alive.
#[derive(Debug)]
pub struct CodexAppServerRuntime {
    config: CodexProviderConfig,
    process: Option<CodexAppServerProcess>,
}

impl CodexAppServerRuntime {
    /// Create a runtime from validated Codex configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when required configuration
    /// values are empty or timeout values are zero.
    pub fn new(config: CodexProviderConfig) -> Result<Self, ProviderError> {
        validate_config(&config)?;
        Ok(Self {
            config,
            process: None,
        })
    }

    /// Start a conversation in the Codex app server.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the app-server process cannot be started
    /// or its `thread/start` response is invalid.
    pub async fn start_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<CodexAppServerConversation, ProviderError> {
        let model = model.or_else(|| self.config.default_model.clone());
        let process = self.ensure_process().await?;
        let thread_id = process.start_thread(model.clone(), cwd).await?;

        Ok(CodexAppServerConversation { thread_id, model })
    }

    /// Send one turn to an existing Codex app-server conversation.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when input is empty, the process is
    /// unavailable, the protocol fails, or the turn response is malformed.
    pub async fn turn(
        &mut self,
        conversation: &CodexAppServerConversation,
        input: String,
    ) -> Result<GenerateResponse, ProviderError> {
        self.turn_with_options(conversation, input, None, false)
            .await
    }

    /// Send one turn that must return a Noema structured response envelope.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when input is empty, the app-server protocol
    /// fails, or the provider does not return the required structured output.
    pub async fn turn_structured(
        &mut self,
        conversation: &CodexAppServerConversation,
        input: String,
        instructions: String,
    ) -> Result<GenerateResponse, ProviderError> {
        self.turn_with_options(conversation, input, Some(instructions), true)
            .await
    }

    async fn turn_with_options(
        &mut self,
        conversation: &CodexAppServerConversation,
        input: String,
        instructions: Option<String>,
        require_noema_response: bool,
    ) -> Result<GenerateResponse, ProviderError> {
        if input.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let provider_input = compose_turn_input(instructions.as_deref(), &input);
        let process = self.ensure_process().await?;
        match process
            .turn(
                &conversation.thread_id,
                conversation.model.as_deref(),
                provider_input,
                require_noema_response,
            )
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                self.retire().await;
                Err(error)
            }
        }
    }

    /// Shut down the app-server subprocess if one is running.
    pub async fn shutdown(&mut self) {
        self.retire().await;
    }

    async fn ensure_process(&mut self) -> Result<&mut CodexAppServerProcess, ProviderError> {
        let should_spawn = match self.process.as_mut() {
            Some(process) => !process.is_alive(),
            None => true,
        };

        if should_spawn {
            self.retire().await;
            let process = CodexAppServerProcess::spawn(self.config.clone()).await?;
            self.process = Some(process);
        }

        self.process
            .as_mut()
            .ok_or_else(|| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: "codex app-server process is not available".to_string(),
            })
    }

    async fn retire(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.close().await;
        }
    }
}

fn validate_config(config: &CodexProviderConfig) -> Result<(), ProviderError> {
    if config.command.trim().is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "codex command cannot be empty".to_string(),
        });
    }

    if config.startup_timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "codex startup timeout must be greater than zero seconds".to_string(),
        });
    }

    if config.turn_timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "codex turn timeout must be greater than zero seconds".to_string(),
        });
    }

    Ok(())
}

fn compose_turn_input(instructions: Option<&str>, input: &str) -> String {
    match instructions.filter(|instructions| !instructions.trim().is_empty()) {
        Some(instructions) => format!(
            "System instructions:\n{}\n\nUser message:\n{}",
            instructions.trim(),
            input
        ),
        None => input.to_string(),
    }
}

#[derive(Debug)]
struct CodexAppServerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    startup_timeout_seconds: u64,
    turn_timeout_seconds: u64,
}

impl CodexAppServerProcess {
    async fn spawn(config: CodexProviderConfig) -> Result<Self, ProviderError> {
        let mut command = Command::new(config.command.trim());
        command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(codex_home) = config.codex_home.as_deref() {
            command.env("CODEX_HOME", codex_home);
        }

        command.env("RUST_LOG", "warn");
        command.kill_on_drop(true);

        let mut child = command.spawn().map_err(|source| match source.kind() {
            ErrorKind::NotFound => ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: "codex command not found".to_string(),
            },
            _ => ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: source.to_string(),
            },
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: "failed to open codex app-server stdin".to_string(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: "failed to open codex app-server stdout".to_string(),
            })?;

        let mut process = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
            startup_timeout_seconds: config.startup_timeout_seconds,
            turn_timeout_seconds: config.turn_timeout_seconds,
        };
        process.initialize().await?;
        Ok(process)
    }

    async fn initialize(&mut self) -> Result<(), ProviderError> {
        let timeout_seconds = self.startup_timeout_seconds;
        let params = json!({
            "clientInfo": {
                "name": "noema",
                "title": "Noema",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        self.request("initialize", params, timeout_seconds).await?;
        self.notify("initialized", json!({})).await
    }

    async fn start_thread(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<String, ProviderError> {
        let mut params = serde_json::Map::new();
        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            params.insert("model".to_string(), Value::String(model));
        }
        if let Some(cwd) = cwd.filter(|cwd| !cwd.trim().is_empty()) {
            params.insert("cwd".to_string(), Value::String(cwd));
        }

        let result = self
            .request(
                "thread/start",
                Value::Object(params),
                self.startup_timeout_seconds,
            )
            .await?;
        extract_thread_id(&result)
    }

    async fn turn(
        &mut self,
        thread_id: &str,
        model: Option<&str>,
        input: String,
        require_noema_response: bool,
    ) -> Result<GenerateResponse, ProviderError> {
        let request_id = self.next_request_id();
        self.send(json!({
            "id": request_id,
            "method": "turn/start",
            "params": {
                "threadId": thread_id,
                "input": [{"type": "text", "text": input}]
            }
        }))
        .await?;

        let mut saw_response = false;
        let mut saw_completion = false;
        let mut delta_text = String::new();
        let mut final_text = None;

        while !(saw_response && saw_completion) {
            let message = self
                .read_message("turn/start", self.turn_timeout_seconds)
                .await?;

            if response_id(&message) == Some(request_id) {
                response_result(&message)?;
                saw_response = true;
                continue;
            }

            if self.handle_server_request(&message).await? {
                continue;
            }

            let Some(method) = message.get("method").and_then(Value::as_str) else {
                continue;
            };

            match method {
                "item/agentMessage/delta" | "item/agentMessage/outputDelta" => {
                    if let Some(delta) = extract_delta_text(&message) {
                        delta_text.push_str(&delta);
                    }
                }
                "item/completed" => {
                    if let Some(text) = extract_completed_agent_text(&message) {
                        final_text = Some(text);
                    }
                }
                "turn/completed" => {
                    validate_turn_completion(&message)?;
                    saw_completion = true;
                }
                _ => {}
            }
        }

        let text = final_text.unwrap_or(delta_text);
        if text.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "codex app-server turn completed without assistant text".to_string(),
            });
        }

        let output = if require_noema_response {
            required_output_items_from_text(text)?
        } else {
            output_items_from_text(text)?
        };

        Ok(GenerateResponse {
            output,
            provider: "codex".to_string(),
            model: model.unwrap_or("codex-default").to_string(),
            response_id: None,
            usage: None,
        })
    }

    async fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout_seconds: u64,
    ) -> Result<Value, ProviderError> {
        let request_id = self.next_request_id();
        self.send(json!({
            "id": request_id,
            "method": method,
            "params": params
        }))
        .await?;

        loop {
            let message = self.read_message(method, timeout_seconds).await?;
            if response_id(&message) == Some(request_id) {
                return response_result(&message);
            }

            self.handle_server_request(&message).await?;
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), ProviderError> {
        self.send(json!({
            "method": method,
            "params": params
        }))
        .await
    }

    async fn send(&mut self, message: Value) -> Result<(), ProviderError> {
        let mut bytes =
            serde_json::to_vec(&message).map_err(|source| ProviderError::ProtocolError {
                provider: "codex".to_string(),
                message: format!("failed to encode JSON-RPC message: {source}"),
            })?;
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|source| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: format!("failed to write to codex app-server: {source}"),
            })
    }

    async fn read_message(
        &mut self,
        operation: &str,
        timeout_seconds: u64,
    ) -> Result<Value, ProviderError> {
        let line = time::timeout(
            Duration::from_secs(timeout_seconds),
            self.stdout.next_line(),
        )
        .await
        .map_err(|_| ProviderError::Timeout {
            provider: "codex".to_string(),
            operation: operation.to_string(),
            seconds: timeout_seconds,
        })?
        .map_err(|source| ProviderError::ProviderUnavailable {
            provider: "codex".to_string(),
            message: format!("failed to read from codex app-server: {source}"),
        })?
        .ok_or_else(|| ProviderError::ProviderUnavailable {
            provider: "codex".to_string(),
            message: "codex app-server exited".to_string(),
        })?;

        serde_json::from_str(&line).map_err(|source| ProviderError::MalformedResponse {
            message: format!("failed to parse codex app-server JSON: {source}"),
        })
    }

    async fn handle_server_request(&mut self, message: &Value) -> Result<bool, ProviderError> {
        let Some(id) = message.get("id").cloned() else {
            return Ok(false);
        };
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(false);
        };

        let response = if method.ends_with("/requestApproval") {
            json!({"id": id, "result": {"decision": "decline"}})
        } else if method == "mcpServer/elicitation/request" {
            json!({
                "id": id,
                "result": {"action": "decline", "content": null, "_meta": null}
            })
        } else {
            json!({
                "id": id,
                "error": {"code": -32601, "message": format!("unsupported method: {method}")}
            })
        };
        self.send(response).await?;
        Ok(true)
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_id;
        self.next_id += 1;
        request_id
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    async fn close(&mut self) {
        let _ = self.stdin.shutdown().await;
        let _ = self.child.start_kill();
        let _ = time::timeout(Duration::from_secs(2), self.child.wait()).await;
    }
}

fn response_id(message: &Value) -> Option<u64> {
    message.get("id").and_then(Value::as_u64)
}

fn response_result(message: &Value) -> Result<Value, ProviderError> {
    if let Some(error) = message.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("codex app-server returned an error")
            .to_string();
        return Err(ProviderError::ProtocolError {
            provider: "codex".to_string(),
            message,
        });
    }

    Ok(message.get("result").cloned().unwrap_or_else(|| json!({})))
}

fn extract_thread_id(result: &Value) -> Result<String, ProviderError> {
    let thread = result.get("thread").unwrap_or(result);
    for key in ["id", "sessionId", "threadId"] {
        if let Some(thread_id) = thread.get(key).and_then(Value::as_str) {
            return Ok(thread_id.to_string());
        }
    }

    if let Some(thread_id) = result.get("threadId").and_then(Value::as_str) {
        return Ok(thread_id.to_string());
    }

    Err(ProviderError::MalformedResponse {
        message: "codex thread/start response did not include a thread id".to_string(),
    })
}

fn extract_delta_text(message: &Value) -> Option<String> {
    let params = message.get("params")?;
    for key in ["delta", "text"] {
        let value = params.get(key)?;
        if let Some(text) = value.as_str() {
            return Some(text.to_string());
        }
        for nested_key in ["text", "content"] {
            if let Some(text) = value.get(nested_key).and_then(Value::as_str) {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn extract_completed_agent_text(message: &Value) -> Option<String> {
    let item = message.get("params")?.get("item")?;
    if item.get("type").and_then(Value::as_str) != Some("agentMessage") {
        return None;
    }
    item.get("text")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn validate_turn_completion(message: &Value) -> Result<(), ProviderError> {
    let Some(status) = message
        .get("params")
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("status"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };

    if matches!(status, "completed" | "interrupted") {
        return Ok(());
    }

    let detail = message
        .get("params")
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("error"))
        .map(format_json_value)
        .filter(|detail| !detail.trim().is_empty());
    let message = match detail {
        Some(detail) => format!("codex turn completed with status {status}: {detail}"),
        None => format!("codex turn completed with status {status}"),
    };

    Err(ProviderError::ProtocolError {
        provider: "codex".to_string(),
        message,
    })
}

fn format_json_value(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }

    for key in ["message", "error", "detail"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            return text.to_string();
        }
    }

    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_thread_id_from_known_shapes() {
        assert_eq!(
            extract_thread_id(&json!({"thread": {"id": "thread-a"}})).expect("id"),
            "thread-a"
        );
        assert_eq!(
            extract_thread_id(&json!({"threadId": "thread-b"})).expect("id"),
            "thread-b"
        );
        assert_eq!(
            extract_thread_id(&json!({"thread": {"sessionId": "thread-c"}})).expect("id"),
            "thread-c"
        );
    }

    #[test]
    fn extracts_completed_agent_message_text() {
        let message = json!({
            "method": "item/completed",
            "params": {
                "item": {"type": "agentMessage", "text": "hello"}
            }
        });

        assert_eq!(
            extract_completed_agent_text(&message).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn validates_failed_turn_status() {
        let message = json!({
            "method": "turn/completed",
            "params": {"turn": {"status": "failed"}}
        });

        assert!(matches!(
            validate_turn_completion(&message),
            Err(ProviderError::ProtocolError { .. })
        ));
    }
}
