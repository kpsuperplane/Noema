//! One-use loopback bridge used by the run-scoped ACP terminal MCP helper.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::oneshot,
};
use tokio_util::sync::CancellationToken;

const MAX_REQUEST_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Serialize)]
struct TokenClaims<'a> {
    run_id: &'a str,
    task_generation: u64,
    contract_id: &'a str,
    lease_token: &'a str,
    expires_at: u64,
}

#[derive(Debug, Deserialize)]
struct BridgeRequest {
    token: String,
    tool: String,
    arguments: serde_json::Value,
}

/// Terminal tool call delivered by the scoped MCP helper.
#[derive(Debug)]
pub(crate) struct AcpTerminalCall {
    pub(crate) tool: String,
    pub(crate) arguments: serde_json::Value,
}

/// Live, cancellation-bound loopback endpoint for exactly one terminal call.
pub(crate) struct AcpTerminalBridge {
    pub(crate) address: String,
    pub(crate) token: String,
    receiver: oneshot::Receiver<Result<AcpTerminalCall, String>>,
}

impl AcpTerminalBridge {
    pub(crate) async fn start(
        fence: &noema_store::WorkRunFence,
        cancellation: CancellationToken,
    ) -> Result<Self, String> {
        Self::start_with_expiry(fence, cancellation, unix_now().saturating_add(30 * 60)).await
    }

    async fn start_with_expiry(
        fence: &noema_store::WorkRunFence,
        cancellation: CancellationToken,
        expires_at: u64,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| format!("cannot bind ACP terminal bridge: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("cannot inspect ACP terminal bridge: {error}"))?
            .to_string();
        let contract_id = fence
            .contract_id
            .as_ref()
            .map_or_else(String::new, ToString::to_string);
        let claims = TokenClaims {
            run_id: &fence.run_id,
            task_generation: fence.task_generation,
            contract_id: &contract_id,
            lease_token: &fence.lease_token,
            expires_at,
        };
        let token = hex_digest(&serde_json::to_vec(&claims).map_err(|error| error.to_string())?);
        let expected = token.clone();
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let result = tokio::select! {
                _ = cancellation.cancelled() => Err("ACP terminal bridge was cancelled".to_string()),
                accepted = listener.accept() => match accepted {
                    Ok((stream, peer)) if peer.ip().is_loopback() => read_request(stream, &expected, expires_at).await,
                    Ok(_) => Err("ACP terminal bridge rejected a non-loopback peer".to_string()),
                    Err(error) => Err(format!("ACP terminal bridge accept failed: {error}")),
                }
            };
            let _ = sender.send(result);
        });
        Ok(Self {
            address,
            token,
            receiver,
        })
    }

    pub(crate) async fn receive(self) -> Result<AcpTerminalCall, String> {
        self.receiver
            .await
            .map_err(|_| "ACP terminal bridge closed without a result".to_string())?
    }
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "tests exercise the private bridge parser and token helpers below"
)]
mod tests {
    use super::*;
    use noema_tasks::TaskContractId;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    fn fence(run_id: &str, lease: &str, generation: u64) -> noema_store::WorkRunFence {
        noema_store::WorkRunFence {
            run_id: run_id.to_string(),
            lease_token: lease.to_string(),
            task_generation: generation,
            contract_id: Some(TaskContractId::new("contract:test").unwrap()),
        }
    }

    async fn send(address: &str, token: &str, tool: &str) -> serde_json::Value {
        let stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        writer
            .write_all(
                format!(
                    "{}\n",
                    serde_json::json!({"token": token, "tool": tool, "arguments": {}})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        BufReader::new(reader)
            .read_line(&mut response)
            .await
            .unwrap();
        serde_json::from_str(&response).unwrap()
    }

    #[tokio::test]
    async fn terminal_tokens_are_scoped_expiring_and_one_use() {
        let cancellation = CancellationToken::new();
        let first = AcpTerminalBridge::start(
            &fence("run:first", "lease:first", 1),
            cancellation.child_token(),
        )
        .await
        .unwrap();
        let other = AcpTerminalBridge::start(
            &fence("run:other", "lease:other", 2),
            cancellation.child_token(),
        )
        .await
        .unwrap();
        assert_ne!(first.token, other.token);
        cancellation.cancel();

        let valid = AcpTerminalBridge::start(
            &fence("run:valid", "lease:valid", 1),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            send(&valid.address, &valid.token, "task.finish_execution").await["ok"],
            true
        );
        let valid_address = valid.address.clone();
        assert_eq!(valid.receive().await.unwrap().tool, "task.finish_execution");
        assert!(
            tokio::net::TcpStream::connect(&valid_address)
                .await
                .is_err()
        );

        let expired = AcpTerminalBridge::start_with_expiry(
            &fence("run:expired", "lease:expired", 1),
            CancellationToken::new(),
            unix_now().saturating_sub(1),
        )
        .await
        .unwrap();
        assert_eq!(
            send(&expired.address, &expired.token, "task.finish_execution").await["error"],
            "invalid_or_expired_token"
        );
        assert!(expired.receive().await.is_err());

        let unscoped = AcpTerminalBridge::start(
            &fence("run:unscoped", "lease:unscoped", 1),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            send(&unscoped.address, &unscoped.token, "filesystem.delete").await["error"],
            "tool_not_scoped"
        );
        assert!(unscoped.receive().await.is_err());
    }
}

async fn read_request(
    stream: tokio::net::TcpStream,
    expected_token: &str,
    expires_at: u64,
) -> Result<AcpTerminalCall, String> {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    BufReader::new(reader)
        .take(MAX_REQUEST_BYTES)
        .read_line(&mut line)
        .await
        .map_err(|error| format!("ACP terminal bridge read failed: {error}"))?;
    let request: BridgeRequest = serde_json::from_str(&line)
        .map_err(|error| format!("ACP terminal bridge received malformed JSON: {error}"))?;
    if unix_now() > expires_at || request.token != expected_token {
        let _ = writer
            .write_all(b"{\"ok\":false,\"error\":\"invalid_or_expired_token\"}\n")
            .await;
        return Err("ACP terminal bridge rejected an invalid or expired token".to_string());
    }
    if !matches!(
        request.tool.as_str(),
        "task.finish_execution" | "task.continue_execution" | "task.report_blocked"
    ) {
        let _ = writer
            .write_all(b"{\"ok\":false,\"error\":\"tool_not_scoped\"}\n")
            .await;
        return Err("ACP terminal bridge rejected an unscoped tool".to_string());
    }
    writer
        .write_all(b"{\"ok\":true}\n")
        .await
        .map_err(|error| format!("ACP terminal bridge response failed: {error}"))?;
    Ok(AcpTerminalCall {
        tool: request.tool,
        arguments: request.arguments,
    })
}

pub(crate) fn helper_executable() -> Result<PathBuf, String> {
    let current = std::env::current_exe()
        .map_err(|error| format!("cannot resolve Noema executable: {error}"))?;
    let file_name = if cfg!(windows) {
        "noema-acp-task-mcp.exe"
    } else {
        "noema-acp-task-mcp"
    };
    Ok(current.with_file_name(file_name))
}

fn hex_digest(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}
