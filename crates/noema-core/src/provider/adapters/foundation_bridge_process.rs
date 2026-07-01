use std::path::PathBuf;
use thiserror::Error;

/// Bridge process configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationBridgeConfig {
    /// Bridge executable path.
    pub bridge_path: PathBuf,
}

/// Running bridge process handle.
#[derive(Debug)]
pub struct FoundationBridgeProcess;

/// Bridge lifecycle error.
#[derive(Debug, Error)]
pub enum FoundationBridgeError {
    /// Bridge binary is missing.
    #[error("bridge binary is missing")]
    BridgeMissing,
    /// Bridge launch failed.
    #[error("bridge launch failed: {0}")]
    BridgeLaunchFailed(String),
}

impl FoundationBridgeError {
    /// Stable safe error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BridgeMissing => "bridge_missing",
            Self::BridgeLaunchFailed(_) => "bridge_launch_failed",
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
            return Err(FoundationBridgeError::BridgeMissing);
        }
        Err(FoundationBridgeError::BridgeLaunchFailed(
            "bridge process transport is not enabled in this build".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn missing_bridge_reports_safe_error() {
        let config = FoundationBridgeConfig {
            bridge_path: PathBuf::from("/path/that/does/not/exist"),
        };

        let error = FoundationBridgeProcess::start(config)
            .await
            .expect_err("missing bridge should fail");

        assert_eq!(error.code(), "bridge_missing");
    }
}
