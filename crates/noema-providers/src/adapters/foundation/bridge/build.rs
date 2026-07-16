use std::{path::Path, process::Output};

use tokio::process::Command;

use super::{FoundationBridgeBuildConfig, FoundationBridgeError};

pub(super) async fn materialize_bridge(
    bridge_path: &Path,
    build: &FoundationBridgeBuildConfig,
) -> Result<(), FoundationBridgeError> {
    let output = Command::new(&build.swift_executable)
        .arg("build")
        .current_dir(&build.package_path)
        .stdin(std::process::Stdio::null())
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

fn summarize_build_output(output: &Output) -> String {
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
