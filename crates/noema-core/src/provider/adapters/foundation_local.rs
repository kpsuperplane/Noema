use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    FoundationLocalProviderConfig,
    provider::{
        GenerateInput, GenerateMessageRole, GenerateRequest, GenerateResponse, GenerateStreamEvent,
        ModelProvider, ProviderContextMetadata, ProviderError, required_output_items_from_text,
    },
};

use super::foundation_bridge_process::{
    FoundationBridgeBuildConfig, FoundationBridgeConfig, FoundationBridgeError,
    FoundationBridgeProcess,
};
use super::foundation_bridge_protocol::{BridgeReplayTurn, BridgeRole};
use super::noema_response_stream::NoemaAssistantTextDeltaExtractor;
use super::responses::ResponsesDiagnosticContext;
use tokio::sync::Mutex;

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
    bridge_runtime: Arc<Mutex<Option<FoundationBridgeRuntime>>>,
}

#[derive(Debug)]
struct FoundationBridgeRuntime {
    process: FoundationBridgeProcess,
    sessions: HashMap<FoundationSessionKey, String>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FoundationSessionKey {
    conversation_id: String,
    model_profile: String,
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
        Ok(Self {
            config,
            bridge_runtime: Arc::new(Mutex::new(None)),
        })
    }

    async fn start_bridge(&self) -> Result<FoundationBridgeProcess, ProviderError> {
        let config = self.bridge_config();
        let diagnostic_path = config.bridge_path.clone();

        self.start_bridge_process(config).await.map_err(|error| {
            ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!(
                    "Apple Foundation Models bridge unavailable: {} ({}) at {}",
                    error.code(),
                    error,
                    diagnostic_path.display()
                ),
            }
        })
    }

    async fn start_bridge_process(
        &self,
        config: FoundationBridgeConfig,
    ) -> Result<FoundationBridgeProcess, FoundationBridgeError> {
        if !cfg!(target_os = "macos") {
            return Err(FoundationBridgeError::UnsupportedPlatform);
        }

        FoundationBridgeProcess::start(config).await
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

    async fn ensure_bridge_runtime(
        &self,
        runtime: &mut Option<FoundationBridgeRuntime>,
    ) -> Result<(), ProviderError> {
        if runtime.is_none() {
            *runtime = Some(FoundationBridgeRuntime {
                process: self.start_bridge().await?,
                sessions: HashMap::new(),
            });
        }
        Ok(())
    }

    async fn session_for_request(
        &self,
        runtime: &mut FoundationBridgeRuntime,
        key: FoundationSessionKey,
        instructions: Option<String>,
        replay_turns: Vec<BridgeReplayTurn>,
    ) -> Result<String, ProviderError> {
        if let Some(session_id) = runtime.sessions.get(&key) {
            return Ok(session_id.clone());
        }
        let session_id = runtime
            .process
            .create_session(
                key.conversation_id.clone(),
                key.model_profile.clone(),
                instructions,
            )
            .await
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!("Apple Foundation Models bridge session failed: {error}"),
            })?;
        if !replay_turns.is_empty() {
            runtime
                .process
                .replay_turns(session_id.clone(), replay_turns)
                .await
                .map_err(|error| ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!("Apple Foundation Models bridge replay failed: {error}"),
                })?;
        }
        runtime.sessions.insert(key, session_id.clone());
        Ok(session_id)
    }

    /// Check whether the local Foundation Models bridge can be launched and is healthy.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the platform, bridge, or
    /// Foundation Models runtime is unavailable.
    pub async fn check_availability(&self) -> Result<(), FoundationBridgeError> {
        let bridge = self.start_bridge_process(self.bridge_config()).await?;
        drop(bridge);
        Ok(())
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
        let mut guard = self.bridge_runtime.lock().await;
        self.ensure_bridge_runtime(&mut guard).await?;
        let runtime = guard.as_mut().expect("bridge runtime initialized");
        let tokens = runtime
            .process
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
        let conversation_id = request
            .conversation_id
            .clone()
            .unwrap_or_else(transient_conversation_id);
        let model = request
            .model
            .clone()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_profile.clone());
        let require_noema_response = request.options.require_noema_response;
        let FoundationPrompt {
            replay_turns,
            generate_input,
        } = foundation_prompt_parts(&request.input);
        let mut noema_delta_extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut relay_delta = |delta: String| {
            if require_noema_response {
                noema_delta_extractor.push_delta(&delta, on_event);
            } else {
                on_event(GenerateStreamEvent::AssistantTextDelta { delta });
            }
        };
        let key = FoundationSessionKey {
            conversation_id,
            model_profile: model.clone(),
        };
        let mut guard = self.bridge_runtime.lock().await;
        self.ensure_bridge_runtime(&mut guard).await?;
        let runtime = guard.as_mut().expect("bridge runtime initialized");
        let session_id = self
            .session_for_request(runtime, key, request.instructions.clone(), replay_turns)
            .await?;
        let output_text = runtime
            .process
            .generate_in_session(
                session_id,
                generate_input,
                request.options.max_output_tokens,
                &mut relay_delta,
            )
            .await
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!("Apple Foundation Models bridge generation failed: {error}"),
            })?;
        let output = if require_noema_response {
            match required_output_items_from_text(output_text.clone()) {
                Ok(output) => output,
                Err(error) => {
                    ResponsesDiagnosticContext::new(
                        self.config.system_errors.clone(),
                        FOUNDATION_LOCAL_PROVIDER,
                        model.clone(),
                        request.conversation_id.clone(),
                    )
                    .log_malformed(
                        error.to_string(),
                        serde_json::json!({ "provider_text": output_text }),
                    );
                    return Err(error);
                }
            }
        } else {
            vec![crate::GenerateOutputItem::AssistantText {
                phase: None,
                text: output_text,
            }]
        };
        Ok(GenerateResponse {
            output,
            provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
            model,
            response_id: None,
            usage: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FoundationPrompt {
    replay_turns: Vec<BridgeReplayTurn>,
    generate_input: String,
}

fn foundation_prompt_parts(input: &GenerateInput) -> FoundationPrompt {
    match input {
        GenerateInput::Text(text) => FoundationPrompt {
            replay_turns: Vec::new(),
            generate_input: text.clone(),
        },
        GenerateInput::Messages(messages) => {
            let last_user_index = messages
                .iter()
                .rposition(|message| message.role == GenerateMessageRole::User);
            let Some(last_user_index) = last_user_index else {
                return FoundationPrompt {
                    replay_turns: bridge_replay_turns(messages),
                    generate_input: String::new(),
                };
            };
            FoundationPrompt {
                replay_turns: bridge_replay_turns(&messages[..last_user_index]),
                generate_input: messages[last_user_index].content.clone(),
            }
        }
    }
}

fn bridge_replay_turns(messages: &[crate::GenerateMessage]) -> Vec<BridgeReplayTurn> {
    messages
        .iter()
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| BridgeReplayTurn {
            role: match message.role {
                GenerateMessageRole::User => BridgeRole::User,
                GenerateMessageRole::Assistant => BridgeRole::Assistant,
            },
            text: message.content.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenerateInput;
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
            system_errors: None,
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
            system_errors: None,
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
            system_errors: None,
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
            system_errors: None,
        })
        .expect("provider");

        let config = provider.bridge_config();

        assert_eq!(config.bridge_path, bridge_path);
        assert!(config.build.is_none());
    }

    #[test]
    fn message_prompt_replays_prior_turns_and_generates_from_latest_user_message() {
        let prompt = foundation_prompt_parts(&GenerateInput::Messages(vec![
            crate::GenerateMessage {
                role: GenerateMessageRole::User,
                content: "first question".to_string(),
            },
            crate::GenerateMessage {
                role: GenerateMessageRole::Assistant,
                content: "first answer".to_string(),
            },
            crate::GenerateMessage {
                role: GenerateMessageRole::User,
                content: "second question".to_string(),
            },
        ]));

        assert_eq!(
            prompt.replay_turns,
            vec![
                BridgeReplayTurn {
                    role: BridgeRole::User,
                    text: "first question".to_string(),
                },
                BridgeReplayTurn {
                    role: BridgeRole::Assistant,
                    text: "first answer".to_string(),
                },
            ]
        );
        assert_eq!(prompt.generate_input, "second question");
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
            system_errors: None,
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
                phase: None,
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
    #[tokio::test]
    async fn generate_required_noema_response_parses_bridge_envelope() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"{\"type\":\"noema_response\",\"output\":[{\"kind\":\"assistant_text\",\"text\":\"bridge answer\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path),
            system_errors: None,
        })
        .expect("provider");

        let response = provider
            .generate(GenerateRequest {
                conversation_id: None,
                model: None,
                input: GenerateInput::Text("prompt text".to_string()),
                instructions: None,
                options: crate::GenerateOptions {
                    require_noema_response: true,
                    ..crate::GenerateOptions::default()
                },
            })
            .await
            .expect("generate");

        assert_eq!(
            response.output,
            vec![
                GenerateOutputItem::AssistantText {
                    phase: None,
                    text: "bridge answer".to_string(),
                },
                GenerateOutputItem::MemoryProposals { proposals: vec![] }
            ]
        );
    }

    #[cfg(all(unix, target_os = "macos"))]
    #[tokio::test]
    async fn generate_required_noema_response_streams_only_assistant_text_from_envelope() {
        let (_dir, bridge_path) = bridge_script(
            r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":1}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"{\"type\":\"noema_response\",\"output\":[{\"kind\":\"assistant_text\",\"text\":\"bridge answer\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"{\"type\":\"noema_response\",\"output\":[{\"kind\":\"assistant_text\",\"text\":\"bridge answer\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}"}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
"#,
        );
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path),
            system_errors: None,
        })
        .expect("provider");
        let mut events = Vec::new();

        let response = provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: None,
                    model: None,
                    input: GenerateInput::Text("prompt text".to_string()),
                    instructions: None,
                    options: crate::GenerateOptions {
                        require_noema_response: true,
                        ..crate::GenerateOptions::default()
                    },
                },
                &mut |event| events.push(event),
            )
            .await
            .expect("generate");

        assert_eq!(
            response.output,
            vec![
                GenerateOutputItem::AssistantText {
                    phase: None,
                    text: "bridge answer".to_string(),
                },
                GenerateOutputItem::MemoryProposals { proposals: vec![] }
            ]
        );
        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                delta: "bridge answer".to_string(),
            }]
        );
    }

    #[cfg(all(unix, target_os = "macos"))]
    #[tokio::test]
    async fn generate_reuses_bridge_session_for_same_conversation() {
        let log = tempfile::NamedTempFile::new().expect("log");
        let log_path = log.path().to_string_lossy().to_string();
        let (_dir, bridge_path) = bridge_script(&format!(
            r#"#!/bin/sh
LOG_PATH="{}"
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":1}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{{"id":"health","payload":{{"type":"health","available":true,"profiles":[{{"id":"default","label":"Default"}}],"unavailable_reason":null}}}}' ;;
    *'"id":"create_session"'*) printf '%s\n' "create_session" >> "$LOG_PATH"; printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
            log_path
        ));
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: Some(bridge_path),
            system_errors: None,
        })
        .expect("provider");

        for input in ["first", "second"] {
            provider
                .generate(GenerateRequest {
                    conversation_id: Some("conversation:stable".to_string()),
                    model: Some("default".to_string()),
                    input: GenerateInput::Text(input.to_string()),
                    instructions: Some("be concise".to_string()),
                    options: crate::GenerateOptions::default(),
                })
                .await
                .expect("generate");
        }

        let create_session_count = std::fs::read_to_string(log.path())
            .expect("log read")
            .lines()
            .filter(|line| *line == "create_session")
            .count();
        assert_eq!(create_session_count, 1);
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
