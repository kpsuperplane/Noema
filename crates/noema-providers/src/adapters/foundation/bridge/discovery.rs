use std::path::PathBuf;

use crate::FoundationLocalProviderConfig;

/// Bridge process configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::adapters::foundation) struct FoundationBridgeConfig {
    /// Bridge executable path.
    pub(in crate::adapters::foundation) bridge_path: PathBuf,
    /// Optional SwiftPM build used to materialize the bridge in source builds.
    pub(in crate::adapters::foundation) build: Option<FoundationBridgeBuildConfig>,
}

/// SwiftPM build configuration used to materialize the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::adapters::foundation) struct FoundationBridgeBuildConfig {
    /// Swift package directory.
    pub(in crate::adapters::foundation) package_path: PathBuf,
    /// Swift executable path or name.
    pub(in crate::adapters::foundation) swift_executable: PathBuf,
}

pub(in crate::adapters::foundation) fn bridge_config_for_provider(
    config: &FoundationLocalProviderConfig,
) -> FoundationBridgeConfig {
    let configured_path = config.bridge_path.clone();
    let bridge_path = configured_path.clone().unwrap_or_else(default_bridge_path);
    let build = if configured_path.is_none()
        && cfg!(target_os = "macos")
        && cfg!(debug_assertions)
        && bridge_path == default_development_bridge_path()
    {
        Some(FoundationBridgeBuildConfig {
            package_path: default_bridge_package_path(),
            swift_executable: PathBuf::from("swift"),
        })
    } else {
        None
    };
    FoundationBridgeConfig { bridge_path, build }
}

fn default_bridge_path() -> PathBuf {
    let sibling_path = std::env::current_exe().ok().and_then(|path| {
        path.parent()
            .map(|parent| parent.join("noema-foundation-bridge"))
    });
    if let Some(path) = &sibling_path
        && path.exists()
    {
        return path.clone();
    }

    let development_path = default_development_bridge_path();
    if development_path.exists() {
        return development_path;
    }

    if cfg!(debug_assertions) {
        development_path
    } else {
        sibling_path.unwrap_or(development_path)
    }
}

pub(in crate::adapters::foundation) fn default_bridge_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("apple-foundation-bridge")
}

pub(in crate::adapters::foundation) fn default_development_bridge_path() -> PathBuf {
    default_bridge_package_path()
        .join(".build")
        .join("debug")
        .join("noema-foundation-bridge")
}
