use std::{path::PathBuf, process::Stdio, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time,
};

use super::foundation_bridge_protocol::{
    BRIDGE_PROTOCOL_VERSION, BridgeReplayTurn, BridgeRequest, BridgeRequestPayload, BridgeResponse,
    BridgeResponsePayload,
};

const CONTROL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const TOKEN_COUNT_RESPONSE_TIMEOUT: Duration = GENERATE_RESPONSE_TIMEOUT;
const GENERATE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// Bridge process configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationBridgeConfig {
    /// Bridge executable path.
    pub bridge_path: PathBuf,
    /// Optional SwiftPM build used to materialize the bridge in source builds.
    pub build: Option<FoundationBridgeBuildConfig>,
}

/// SwiftPM build configuration used to materialize the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationBridgeBuildConfig {
    /// Swift package directory.
    pub package_path: PathBuf,
    /// Swift executable path or name.
    pub swift_executable: PathBuf,
}

/// Running bridge process handle.
#[derive(Debug)]
pub struct FoundationBridgeProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

/// Bridge lifecycle error.
#[derive(Debug, Error)]
pub enum FoundationBridgeError {
    /// Bridge binary is missing.
    #[error("bridge binary is missing")]
    BridgeMissing,
    /// Current platform cannot run Apple Foundation Models.
    #[error("unsupported platform")]
    UnsupportedPlatform,
    /// Bridge launch failed.
    #[error("bridge launch failed: {0}")]
    BridgeLaunchFailed(String),
    /// Bridge build failed.
    #[error("bridge build failed: {0}")]
    BridgeBuildFailed(String),
    /// Bridge protocol failed.
    #[error("bridge protocol failed: {0}")]
    BridgeProtocol(String),
    /// Foundation Models runtime is not available through the bridge.
    #[error("foundation models unavailable: {0}")]
    FoundationUnavailable(String),
}

impl FoundationBridgeError {
    /// Stable safe error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BridgeMissing => "bridge_missing",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::BridgeLaunchFailed(_) => "bridge_launch_failed",
            Self::BridgeBuildFailed(_) => "bridge_build_failed",
            Self::BridgeProtocol(_) => "bridge_protocol_error",
            Self::FoundationUnavailable(_) => "foundation_unavailable",
        }
    }
}

impl FoundationBridgeProcess {
    /// Start the bridge process.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the bridge cannot be launched.
    pub async fn start(config: FoundationBridgeConfig) -> Result<Self, FoundationBridgeError> {
        if !config.bridge_path.exists() {
            if let Some(build) = &config.build {
                materialize_bridge(&config.bridge_path, build).await?;
            }
            if !config.bridge_path.exists() {
                return Err(FoundationBridgeError::BridgeMissing);
            }
        }

        let mut child = Command::new(&config.bridge_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            FoundationBridgeError::BridgeLaunchFailed("bridge stdin is unavailable".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            FoundationBridgeError::BridgeLaunchFailed("bridge stdout is unavailable".to_string())
        })?;
        let mut process = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        };

        process.handshake().await?;
        process.health().await?;
        Ok(process)
    }

    /// Count prompt tokens through the bridge.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when token counting fails.
    pub async fn count_tokens(
        &mut self,
        instructions: Option<String>,
        input: String,
    ) -> Result<u32, FoundationBridgeError> {
        let response = self
            .send_request_with_timeout(
                BridgeRequest {
                    id: "count_tokens".to_string(),
                    payload: BridgeRequestPayload::CountTokens {
                        instructions,
                        input,
                    },
                },
                TOKEN_COUNT_RESPONSE_TIMEOUT,
            )
            .await?;
        match response.payload {
            BridgeResponsePayload::TokenCount { tokens } => Ok(tokens),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected token count response {payload:?}"
            ))),
        }
    }

    async fn handshake(&mut self) -> Result<(), FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "handshake".to_string(),
                payload: BridgeRequestPayload::Handshake {
                    protocol_version: BRIDGE_PROTOCOL_VERSION,
                },
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::HandshakeOk { protocol_version }
                if protocol_version == BRIDGE_PROTOCOL_VERSION =>
            {
                Ok(())
            }
            BridgeResponsePayload::HandshakeOk { protocol_version } => {
                Err(FoundationBridgeError::BridgeProtocol(format!(
                    "unsupported bridge protocol version {protocol_version}"
                )))
            }
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected handshake response {payload:?}"
            ))),
        }
    }

    async fn health(&mut self) -> Result<(), FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "health".to_string(),
                payload: BridgeRequestPayload::Health,
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::Health {
                available: true, ..
            } => Ok(()),
            BridgeResponsePayload::Health {
                available: false,
                unavailable_reason,
                ..
            } => Err(FoundationBridgeError::FoundationUnavailable(
                unavailable_reason
                    .unwrap_or_else(|| "Foundation Models runtime is unavailable.".to_string()),
            )),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected health response {payload:?}"
            ))),
        }
    }

    pub(crate) async fn create_session(
        &mut self,
        conversation_id: String,
        model_profile: String,
        instructions: Option<String>,
    ) -> Result<String, FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "create_session".to_string(),
                payload: BridgeRequestPayload::CreateSession {
                    conversation_id,
                    model_profile,
                    instructions,
                },
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::SessionCreated { session_id } => Ok(session_id),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected create_session response {payload:?}"
            ))),
        }
    }

    /// Replay prior transcript turns into an existing bridge session.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the bridge rejects replay or
    /// returns an unexpected response.
    pub async fn replay_turns(
        &mut self,
        session_id: String,
        turns: Vec<BridgeReplayTurn>,
    ) -> Result<(), FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "replay_turns".to_string(),
                payload: BridgeRequestPayload::ReplayTurns { session_id, turns },
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::ReplayComplete => Ok(()),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected replay_turns response {payload:?}"
            ))),
        }
    }

    /// Close a bridge session that is no longer valid for its conversation.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the bridge rejects the close or
    /// returns an unexpected response.
    pub async fn close_session(&mut self, session_id: String) -> Result<(), FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "close_session".to_string(),
                payload: BridgeRequestPayload::CloseSession { session_id },
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::ReplayComplete => Ok(()),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected close_session response {payload:?}"
            ))),
        }
    }

    /// Ask the bridge to cancel an in-flight request.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when cancellation is unsupported,
    /// rejected, or the bridge returns an unexpected response.
    pub async fn cancel_request(
        &mut self,
        request_id: String,
    ) -> Result<(), FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "cancel".to_string(),
                payload: BridgeRequestPayload::Cancel { request_id },
            })
            .await?;
        match response.payload {
            BridgeResponsePayload::CancelComplete => Ok(()),
            payload => Err(FoundationBridgeError::BridgeProtocol(format!(
                "unexpected cancel response {payload:?}"
            ))),
        }
    }

    pub(crate) async fn generate_in_session(
        &mut self,
        session_id: String,
        input: String,
        max_output_tokens: Option<u32>,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, FoundationBridgeError> {
        let request_id = "generate".to_string();
        let request = BridgeRequest {
            id: request_id.clone(),
            payload: BridgeRequestPayload::Generate {
                session_id,
                input,
                max_output_tokens,
            },
        };
        self.write_request(&request).await?;

        loop {
            let response = self
                .read_response_with_timeout(&request_id, GENERATE_RESPONSE_TIMEOUT)
                .await?;
            match response.payload {
                BridgeResponsePayload::AssistantTextDelta { delta } => on_delta(delta),
                BridgeResponsePayload::GenerateComplete { text } => return Ok(text),
                payload => {
                    return Err(FoundationBridgeError::BridgeProtocol(format!(
                        "unexpected generate response {payload:?}"
                    )));
                }
            }
        }
    }

    async fn send_request(
        &mut self,
        request: BridgeRequest,
    ) -> Result<BridgeResponse, FoundationBridgeError> {
        self.send_request_with_timeout(request, CONTROL_RESPONSE_TIMEOUT)
            .await
    }

    async fn send_request_with_timeout(
        &mut self,
        request: BridgeRequest,
        timeout: Duration,
    ) -> Result<BridgeResponse, FoundationBridgeError> {
        let request_id = request.id.clone();
        self.write_request(&request).await?;
        self.read_response_with_timeout(&request_id, timeout).await
    }

    async fn write_request(
        &mut self,
        request: &BridgeRequest,
    ) -> Result<(), FoundationBridgeError> {
        let line = serde_json::to_string(&request)
            .map_err(|source| FoundationBridgeError::BridgeProtocol(source.to_string()))?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?;
        Ok(())
    }

    async fn read_response_with_timeout(
        &mut self,
        request_id: &str,
        timeout: Duration,
    ) -> Result<BridgeResponse, FoundationBridgeError> {
        let deadline = time::Instant::now() + timeout;
        loop {
            let now = time::Instant::now();
            if now >= deadline {
                return Err(FoundationBridgeError::BridgeLaunchFailed(format!(
                    "timed out waiting for bridge response to {request_id}"
                )));
            }
            let remaining = deadline - now;
            let response_line = time::timeout(remaining, self.stdout.next_line())
                .await
                .map_err(|_| {
                    FoundationBridgeError::BridgeLaunchFailed(format!(
                        "timed out waiting for bridge response to {request_id}"
                    ))
                })?
                .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?
                .ok_or_else(|| {
                    FoundationBridgeError::BridgeLaunchFailed(format!(
                        "bridge exited before responding to {request_id}"
                    ))
                })?;
            let response: BridgeResponse = serde_json::from_str(&response_line)
                .map_err(|source| FoundationBridgeError::BridgeProtocol(source.to_string()))?;
            if response.id != request_id {
                continue;
            }
            if let BridgeResponsePayload::Error { code, message } = &response.payload {
                return Err(FoundationBridgeError::BridgeProtocol(format!(
                    "{code}: {message}"
                )));
            }
            return Ok(response);
        }
    }
}

impl Drop for FoundationBridgeProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

async fn materialize_bridge(
    bridge_path: &std::path::Path,
    build: &FoundationBridgeBuildConfig,
) -> Result<(), FoundationBridgeError> {
    let output = Command::new(&build.swift_executable)
        .arg("build")
        .current_dir(&build.package_path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|source| FoundationBridgeError::BridgeBuildFailed(source.to_string()))?;
    if !output.status.success() {
        return Err(FoundationBridgeError::BridgeBuildFailed(
            summarize_build_output(&output),
        ));
    }
    if !bridge_path.exists() {
        return Err(FoundationBridgeError::BridgeBuildFailed(format!(
            "swift build completed but did not create {}",
            bridge_path.display()
        )));
    }
    Ok(())
}

fn summarize_build_output(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    if detail.is_empty() {
        return format!("swift build exited with status {}", output.status);
    }
    let mut lines = detail.lines().take(12).collect::<Vec<_>>().join("\n");
    if detail.lines().count() > 12 {
        lines.push_str("\n...");
    }
    format!("swift build exited with status {}: {lines}", output.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::adapters::foundation_bridge_protocol::{BridgeReplayTurn, BridgeRole};
    use std::path::PathBuf;

    #[tokio::test]
    async fn missing_bridge_reports_safe_error() {
        let config = FoundationBridgeConfig {
            bridge_path: PathBuf::from("/path/that/does/not/exist"),
            build: None,
        };

        let error = FoundationBridgeProcess::start(config)
            .await
            .expect_err("missing bridge should fail");

        assert_eq!(error.code(), "bridge_missing");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_start_performs_handshake_and_health_check() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );

        let process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        drop(process);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_start_reports_foundation_unavailable_health() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":false,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":"Foundation Models runtime is unavailable."}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );

        let error = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect_err("unavailable bridge should fail health");

        assert_eq!(error.code(), "foundation_unavailable");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_generate_returns_session_output_and_deltas() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"bridge "}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");
        let mut deltas = Vec::new();

        let session_id = process
            .create_session(
                "conversation:test".to_string(),
                "default".to_string(),
                Some("be concise".to_string()),
            )
            .await
            .expect("session should be created");
        let text = process
            .generate_in_session(session_id, "hello".to_string(), None, &mut |delta| {
                deltas.push(delta);
            })
            .await
            .expect("generate should complete");

        assert_eq!(text, "bridge answer");
        assert_eq!(deltas, vec!["bridge ".to_string()]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_generate_waits_longer_than_control_timeout() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) sleep 6; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"slow bridge answer"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        let session_id = process
            .create_session("conversation:test".to_string(), "default".to_string(), None)
            .await
            .expect("session should be created");
        let text = process
            .generate_in_session(session_id, "hello".to_string(), None, &mut |_| {})
            .await
            .expect("generate should wait beyond control timeout");

        assert_eq!(text, "slow bridge answer");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_count_tokens_waits_longer_than_control_timeout() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"count_tokens"'*) sleep 6; printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        let tokens = process
            .count_tokens(Some("instructions".to_string()), "hello".to_string())
            .await
            .expect("count_tokens should wait beyond control timeout");

        assert_eq!(tokens, 42);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_ignores_stale_response_ids_before_matching_response() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}'; printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        let session_id = process
            .create_session(
                "conversation:test".to_string(),
                "default".to_string(),
                Some("be concise".to_string()),
            )
            .await
            .expect("create_session should skip stale responses");

        assert_eq!(session_id, "session-1");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_replay_turns_sends_replay_request() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"replay_turns"'*'"role":"user"'*'"text":"hello"'*) printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        process
            .replay_turns(
                "session-1".to_string(),
                vec![BridgeReplayTurn {
                    role: BridgeRole::User,
                    text: "hello".to_string(),
                }],
            )
            .await
            .expect("replay should complete");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_cancel_request_accepts_cancel_complete() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"cancel"'*'"request_id":"generate"'*) printf '%s\n' '{"id":"cancel","payload":{"type":"cancel_complete"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let mut process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: None,
        })
        .await
        .expect("bridge should start");

        process
            .cancel_request("generate".to_string())
            .await
            .expect("cancel should complete");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn missing_bridge_can_be_materialized_before_launch() {
        let package_dir = tempfile::tempdir().expect("package tempdir");
        let swift = fake_swift_builder(
            package_dir.path(),
            r#"#!/bin/sh
mkdir -p .build/debug
cat > .build/debug/noema-foundation-bridge <<'BRIDGE'
#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
BRIDGE
chmod +x .build/debug/noema-foundation-bridge
"#,
        );
        let bridge_path = package_dir
            .path()
            .join(".build/debug/noema-foundation-bridge");

        let process = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: Some(FoundationBridgeBuildConfig {
                package_path: package_dir.path().to_path_buf(),
                swift_executable: swift,
            }),
        })
        .await
        .expect("bridge should be materialized and launched");

        drop(process);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_materialization_reports_build_error() {
        let package_dir = tempfile::tempdir().expect("package tempdir");
        let swift = fake_swift_builder(
            package_dir.path(),
            r#"#!/bin/sh
printf '%s\n' 'missing BuildServerProtocol.framework' >&2
exit 42
"#,
        );
        let bridge_path = package_dir
            .path()
            .join(".build/debug/noema-foundation-bridge");

        let error = FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path,
            build: Some(FoundationBridgeBuildConfig {
                package_path: package_dir.path().to_path_buf(),
                swift_executable: swift,
            }),
        })
        .await
        .expect_err("build failure should be reported");

        assert_eq!(error.code(), "bridge_build_failed");
        assert!(error.to_string().contains("BuildServerProtocol.framework"));
    }

    #[cfg(unix)]
    fn bridge_script(contents: &str) -> (tempfile::TempDir, PathBuf) {
        use std::{fs, os::unix::fs::PermissionsExt};

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bridge");
        fs::write(&path, contents).expect("script write");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        (dir, path)
    }

    #[cfg(unix)]
    fn fake_swift_builder(package_dir: &std::path::Path, contents: &str) -> PathBuf {
        use std::{fs, os::unix::fs::PermissionsExt};

        let path = package_dir.join("swift");
        fs::write(&path, contents).expect("fake swift write");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }
}
