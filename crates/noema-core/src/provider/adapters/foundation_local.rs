use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    FoundationLocalProviderConfig,
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderContextMetadata, ProviderError,
    },
};

use super::foundation_bridge_process::{
    FoundationBridgeBuildConfig, FoundationBridgeConfig, FoundationBridgeProcess,
};

/// Provider identifier for Apple Foundation Models.
pub const FOUNDATION_LOCAL_PROVIDER: &str = "foundation_local";
/// Apple Foundation Models system context window.
pub const FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS: u32 = 4_096;
/// Default response reserve for Foundation Local prompts.
pub const FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 512;
/// Default compact summary target for Foundation Local.
pub const FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 512;

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

    async fn start_bridge(&self) -> Result<FoundationBridgeProcess, ProviderError> {
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

fn transient_conversation_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("conversation:foundation-local-{sequence}")
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

    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata {
            context_window_tokens: Some(FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS),
            default_output_reserve_tokens: Some(FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS),
            compact_summary_target_tokens: Some(FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS),
        }
    }

    async fn count_tokens(
        &self,
        instructions: Option<&str>,
        input: &str,
        _model: Option<&str>,
    ) -> Result<Option<u32>, ProviderError> {
        let mut bridge = self.start_bridge().await?;
        let tokens = bridge
            .count_tokens(instructions.map(str::to_string), input.to_string())
            .await
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!("Apple Foundation Models bridge token count failed: {error}"),
            })?;
        Ok(Some(tokens))
    }

    async fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let mut bridge = self.start_bridge().await?;
        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_profile.clone());
        let GenerateInput::Text(text) = request.input;
        let mut relay_delta = |delta| on_event(GenerateStreamEvent::AssistantTextDelta { delta });
        let output_text = bridge
            .generate(
                transient_conversation_id(),
                model.clone(),
                request.instructions,
                text,
                request.options.max_output_tokens,
                &mut relay_delta,
            )
            .await
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!("Apple Foundation Models bridge generation failed: {error}"),
            })?;
        Ok(GenerateResponse {
            output: vec![crate::GenerateOutputItem::AssistantText { text: output_text }],
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
    use crate::provider::GenerateStreamEvent;
    use crate::{
        FoundationLocalProviderConfig, GenerateOutputItem, GenerateRequest, ModelProvider,
        ProviderError,
    };

    #[tokio::test]
    async fn missing_configured_bridge_fails_without_path_configuration_error() {
        let bridge_path = std::env::temp_dir().join(format!(
            "missing-noema-foundation-bridge-{}",
            std::process::id()
        ));
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path),
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
            "provider should report bridge launch/materialization errors directly: {message}"
        );
    }

    #[test]
    fn foundation_local_advertises_context_window_metadata() {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: None,
        })
        .expect("provider");

        let metadata = provider.context_metadata(Some("default"));

        assert_eq!(metadata.context_window_tokens, Some(4_096));
        assert_eq!(metadata.default_output_reserve_tokens, Some(512));
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

    #[cfg(all(unix, target_os = "macos"))]
    #[tokio::test]
    async fn generate_uses_bridge_response_instead_of_echoing_input() {
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
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path),
        })
        .expect("provider");
        let mut events = Vec::new();

        let response = provider
            .generate_streaming(GenerateRequest::text("prompt text"), &mut |event| {
                events.push(event);
            })
            .await
            .expect("generate");

        assert_eq!(
            response.output,
            vec![GenerateOutputItem::AssistantText {
                text: "bridge answer".to_string(),
            }]
        );
        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: "bridge ".to_string(),
            }]
        );
    }

    #[cfg(all(unix, target_os = "macos"))]
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
}
