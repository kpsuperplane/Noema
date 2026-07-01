use std::path::PathBuf;

use crate::{
    FoundationLocalProviderConfig,
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderError,
    },
};

use super::foundation_bridge_process::{
    FoundationBridgeBuildConfig, FoundationBridgeConfig, FoundationBridgeProcess,
};

/// Provider identifier for Apple Foundation Models.
pub const FOUNDATION_LOCAL_PROVIDER: &str = "foundation_local";

/// Apple Foundation Models provider facade.
#[derive(Debug, Clone)]
pub struct FoundationLocalProvider {
    config: FoundationLocalProviderConfig,
}

impl FoundationLocalProvider {
    /// Build a Foundation Local provider.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn new(config: FoundationLocalProviderConfig) -> Result<Self, ProviderError> {
        if config.default_profile.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "foundation local default profile cannot be empty".to_string(),
            });
        }
        Ok(Self { config })
    }

    async fn ensure_available(&self) -> Result<(), ProviderError> {
        if !cfg!(target_os = "macos") {
            return Err(ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: "Apple Foundation Models are only available on macOS.".to_string(),
            });
        }

        let config = self.bridge_config();
        let diagnostic_path = config.bridge_path.clone();

        FoundationBridgeProcess::start(config)
            .await
            .map(|_| ())
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!(
                    "Apple Foundation Models bridge unavailable: {} ({}) at {}",
                    error.code(),
                    error,
                    diagnostic_path.display()
                ),
            })
    }

    fn bridge_config(&self) -> FoundationBridgeConfig {
        let configured_path = self.config.bridge_path.clone();
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

fn default_bridge_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("apple-foundation-bridge")
}

fn default_development_bridge_path() -> PathBuf {
    default_bridge_package_path()
        .join(".build")
        .join("debug")
        .join("noema-foundation-bridge")
}

impl ModelProvider for FoundationLocalProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let mut ignore_event = |_| {};
        self.generate_streaming(request, &mut ignore_event).await
    }

    async fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        self.ensure_available().await?;
        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_profile.clone());
        let GenerateInput::Text(text) = request.input;
        Ok(GenerateResponse {
            output: vec![crate::GenerateOutputItem::AssistantText { text }],
            provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
            model,
            response_id: None,
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoundationLocalProviderConfig, GenerateRequest, ModelProvider, ProviderError};

    #[tokio::test]
    async fn unavailable_stub_fails_cleanly() {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: None,
        })
        .expect("provider");

        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .expect_err("stub should be unavailable");

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
        let ProviderError::ProviderUnavailable { message, .. } = error else {
            unreachable!("matched provider unavailable above");
        };
        assert!(
            !message.contains("bridge path is not configured"),
            "provider should attempt a default daemon-owned bridge path: {message}"
        );
    }

    #[test]
    fn default_macos_debug_bridge_config_materializes_source_tree_bridge() {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: None,
        })
        .expect("provider");

        let config = provider.bridge_config();

        if cfg!(target_os = "macos") && cfg!(debug_assertions) {
            assert_eq!(config.bridge_path, default_development_bridge_path());
            let build = config.build.expect("debug macOS source build");
            assert_eq!(build.package_path, default_bridge_package_path());
            assert_eq!(build.swift_executable, PathBuf::from("swift"));
        } else {
            assert!(config.build.is_none());
        }
    }

    #[test]
    fn configured_bridge_path_is_not_auto_materialized() {
        let bridge_path = PathBuf::from("/tmp/noema-foundation-bridge");
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path.clone()),
        })
        .expect("provider");

        let config = provider.bridge_config();

        assert_eq!(config.bridge_path, bridge_path);
        assert!(config.build.is_none());
    }
}
