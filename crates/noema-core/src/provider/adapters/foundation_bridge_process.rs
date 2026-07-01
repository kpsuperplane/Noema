use std::{path::PathBuf, process::Stdio, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time,
};

use super::foundation_bridge_protocol::{
    BRIDGE_PROTOCOL_VERSION, BridgeRequest, BridgeRequestPayload, BridgeResponse,
    BridgeResponsePayload,
};

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

    async fn send_request(
        &mut self,
        request: BridgeRequest,
    ) -> Result<BridgeResponse, FoundationBridgeError> {
        let request_id = request.id.clone();
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

        let response_line = time::timeout(Duration::from_secs(5), self.stdout.next_line())
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
            return Err(FoundationBridgeError::BridgeProtocol(format!(
                "bridge response id {} did not match request id {request_id}",
                response.id
            )));
        }
        if let BridgeResponsePayload::Error { code, message } = &response.payload {
            return Err(FoundationBridgeError::BridgeProtocol(format!(
                "{code}: {message}"
            )));
        }
        Ok(response)
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
