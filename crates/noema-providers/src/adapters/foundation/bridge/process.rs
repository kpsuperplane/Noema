use std::{collections::HashSet, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time,
};

use super::{
    FoundationBridgeConfig, FoundationBridgeError,
    build::materialize_bridge,
    protocol::{
        BRIDGE_PROTOCOL_VERSION, BridgeReplayTurn, BridgeRequest, BridgeRequestPayload,
        BridgeResponse, BridgeResponsePayload, BridgeToolCall, BridgeToolResult,
    },
};

const CONTROL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const TOKEN_COUNT_RESPONSE_TIMEOUT: Duration = GENERATE_RESPONSE_TIMEOUT;
const GENERATE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// Native generation output surfaced by the Foundation bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::adapters::foundation) struct FoundationGeneration {
    /// Complete assistant text returned by Foundation Models.
    pub text: String,
    /// Native calls requested during this generation.
    pub tool_calls: Vec<BridgeToolCall>,
}

/// Running bridge process handle.
#[derive(Debug)]
pub(in crate::adapters::foundation) struct FoundationBridgeProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    pending_generation: Option<PendingFoundationGeneration>,
}

#[derive(Debug)]
struct PendingFoundationGeneration {
    request_id: String,
    session_id: String,
    tool_call_ids: HashSet<String>,
}

impl FoundationBridgeProcess {
    pub(in crate::adapters::foundation) async fn start(
        config: FoundationBridgeConfig,
    ) -> Result<Self, FoundationBridgeError> {
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
            pending_generation: None,
        };

        process.handshake().await?;
        process.health().await?;
        Ok(process)
    }

    pub(in crate::adapters::foundation) async fn count_tokens(
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

    pub(in crate::adapters::foundation) async fn create_session(
        &mut self,
        conversation_id: String,
        model_profile: String,
        instructions: Option<String>,
        tools: Vec<super::protocol::BridgeToolDefinition>,
        tool_catalog_fingerprint: String,
    ) -> Result<String, FoundationBridgeError> {
        let response = self
            .send_request(BridgeRequest {
                id: "create_session".to_string(),
                payload: BridgeRequestPayload::CreateSession {
                    conversation_id,
                    model_profile,
                    instructions,
                    tools,
                    tool_catalog_fingerprint,
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

    pub(in crate::adapters::foundation) async fn replay_turns(
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

    pub(in crate::adapters::foundation) async fn close_session(
        &mut self,
        session_id: String,
    ) -> Result<(), FoundationBridgeError> {
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

    #[cfg(test)]
    pub(in crate::adapters::foundation) async fn cancel_request(
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

    pub(in crate::adapters::foundation) async fn generate_in_session(
        &mut self,
        session_id: String,
        input: String,
        max_output_tokens: Option<u32>,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<FoundationGeneration, FoundationBridgeError> {
        if self.pending_generation.is_some() {
            return Err(FoundationBridgeError::BridgeProtocol(
                "a Foundation generation is already waiting for tool results".to_string(),
            ));
        }
        let request_id = "generate".to_string();
        let request = BridgeRequest {
            id: request_id.clone(),
            payload: BridgeRequestPayload::Generate {
                session_id: session_id.clone(),
                input,
                max_output_tokens,
            },
        };
        self.write_request(&request).await?;
        self.pending_generation = Some(PendingFoundationGeneration {
            request_id,
            session_id,
            tool_call_ids: HashSet::new(),
        });
        self.read_generation_until_boundary(on_delta).await
    }

    pub(in crate::adapters::foundation) async fn continue_generation(
        &mut self,
        session_id: &str,
        results: Vec<BridgeToolResult>,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<FoundationGeneration, FoundationBridgeError> {
        let Some(pending) = &self.pending_generation else {
            return Err(FoundationBridgeError::BridgeProtocol(
                "Foundation tool results arrived without a pending generation".to_string(),
            ));
        };
        if pending.session_id != session_id {
            return Err(FoundationBridgeError::BridgeProtocol(
                "Foundation tool results targeted the wrong session".to_string(),
            ));
        }
        if pending.tool_call_ids.is_empty() {
            return Err(FoundationBridgeError::BridgeProtocol(
                "Foundation tool results arrived without a pending native tool call".to_string(),
            ));
        }
        let expected_call_ids = pending.tool_call_ids.clone();
        if results.is_empty() {
            return Err(FoundationBridgeError::BridgeProtocol(
                "no Foundation tool results supplied for pending native tool calls".to_string(),
            ));
        }
        let mut provided_call_ids = HashSet::with_capacity(results.len());
        for result in &results {
            if !expected_call_ids.contains(&result.call_id) {
                return Err(FoundationBridgeError::BridgeProtocol(format!(
                    "unknown Foundation tool result call id {:?}",
                    result.call_id
                )));
            }
            if !provided_call_ids.insert(result.call_id.clone()) {
                return Err(FoundationBridgeError::BridgeProtocol(format!(
                    "duplicate Foundation tool result call id {:?}",
                    result.call_id
                )));
            }
        }
        if provided_call_ids.len() != expected_call_ids.len() {
            let missing_call_id = expected_call_ids
                .difference(&provided_call_ids)
                .next()
                .expect("different call-id counts imply a missing id");
            return Err(FoundationBridgeError::BridgeProtocol(format!(
                "missing Foundation tool result for call id {:?}",
                missing_call_id
            )));
        }
        for result in results {
            self.write_request(&BridgeRequest {
                id: format!("tool_result:{}", result.call_id),
                payload: BridgeRequestPayload::ToolResult {
                    session_id: session_id.to_string(),
                    call_id: result.call_id,
                    output: result.output,
                    is_error: result.is_error,
                },
            })
            .await?;
        }
        self.read_generation_until_boundary(on_delta).await
    }

    async fn read_generation_until_boundary(
        &mut self,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<FoundationGeneration, FoundationBridgeError> {
        let request_id = self
            .pending_generation
            .as_ref()
            .expect("pending generation checked")
            .request_id
            .clone();
        let mut text = String::new();
        loop {
            let response = self.read_any_response(GENERATE_RESPONSE_TIMEOUT).await?;
            if response.id != request_id {
                if matches!(response.payload, BridgeResponsePayload::ToolResultAccepted) {
                    continue;
                }
                return Err(FoundationBridgeError::BridgeProtocol(format!(
                    "unexpected interleaved bridge response {:?}",
                    response.payload
                )));
            }
            match response.payload {
                BridgeResponsePayload::AssistantTextDelta { delta } => {
                    text.push_str(&delta);
                    on_delta(delta);
                }
                BridgeResponsePayload::ToolCall {
                    call_id,
                    tool_name,
                    arguments,
                } => {
                    let pending = self
                        .pending_generation
                        .as_mut()
                        .expect("pending generation checked");
                    pending.tool_call_ids.clear();
                    pending.tool_call_ids.insert(call_id.clone());
                    return Ok(FoundationGeneration {
                        text,
                        tool_calls: vec![BridgeToolCall {
                            call_id,
                            tool_name,
                            arguments,
                        }],
                    });
                }
                BridgeResponsePayload::GenerateComplete { text } => {
                    self.pending_generation = None;
                    return Ok(FoundationGeneration {
                        text,
                        tool_calls: Vec::new(),
                    });
                }
                BridgeResponsePayload::Error { code, message } => {
                    self.pending_generation = None;
                    return Err(FoundationBridgeError::BridgeProtocol(format!(
                        "{code}: {message}"
                    )));
                }
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

    async fn read_any_response(
        &mut self,
        timeout: Duration,
    ) -> Result<BridgeResponse, FoundationBridgeError> {
        let response_line = time::timeout(timeout, self.stdout.next_line())
            .await
            .map_err(|_| {
                FoundationBridgeError::BridgeLaunchFailed(
                    "timed out waiting for bridge response".to_string(),
                )
            })?
            .map_err(|source| FoundationBridgeError::BridgeLaunchFailed(source.to_string()))?
            .ok_or_else(|| {
                FoundationBridgeError::BridgeLaunchFailed(
                    "bridge exited before completing generation".to_string(),
                )
            })?;
        serde_json::from_str(&response_line)
            .map_err(|source| FoundationBridgeError::BridgeProtocol(source.to_string()))
    }
}

impl Drop for FoundationBridgeProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
