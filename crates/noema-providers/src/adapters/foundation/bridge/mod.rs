mod build;
mod discovery;
mod process;
mod protocol;

use thiserror::Error;

pub(super) use discovery::{
    FoundationBridgeBuildConfig, FoundationBridgeConfig, bridge_config_for_provider,
};
#[cfg(test)]
pub(in crate::adapters::foundation) use discovery::{
    default_bridge_package_path, default_development_bridge_path,
};
pub(super) use process::{FoundationBridgeProcess, FoundationGeneration};
#[cfg(test)]
pub(in crate::adapters::foundation) use protocol::BridgeToolCall;
pub(super) use protocol::{
    BridgeReplayToolCall, BridgeReplayToolResult, BridgeReplayTurn, BridgeRole,
    BridgeToolDefinition, BridgeToolResult,
};

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

#[cfg(test)]
mod tests;
