use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::Mutex;

use crate::{
    FoundationLocalProviderConfig, GenerateRequest, GenerateResponse, GenerateResponseStatus,
    GenerateStreamEvent, ModelProvider, ParsedNoemaResponse, ProviderContextMetadata,
    ProviderError, ProviderToolCapabilities, ProviderToolTransport, output_items_from_text,
    required_noema_response_from_text,
    response_support::{NoemaAssistantTextDeltaExtractor, StructuredResponseDiagnosticContext},
};

use super::{
    bridge::{BridgeReplayTurn, BridgeRole, FoundationBridgeProcess},
    lowering::{bridge_replay_parsed_response, foundation_prompt_parts},
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
    pub(super) config: FoundationLocalProviderConfig,
    bridge_runtime: Arc<Mutex<Option<FoundationBridgeRuntime>>>,
}

#[derive(Debug)]
struct FoundationBridgeRuntime {
    process: FoundationBridgeProcess,
    sessions: HashMap<FoundationSessionKey, FoundationSession>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FoundationSessionKey {
    conversation_id: String,
    model_profile: String,
    static_instructions: Option<String>,
}

#[derive(Debug)]
struct FoundationSession {
    id: String,
    replayed_history: Vec<BridgeReplayTurn>,
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
        replay_turns: Vec<BridgeReplayTurn>,
    ) -> Result<String, ProviderError> {
        let stale_keys = runtime
            .sessions
            .keys()
            .filter(|existing| {
                existing.conversation_id == key.conversation_id
                    && (existing.model_profile != key.model_profile
                        || existing.static_instructions != key.static_instructions)
            })
            .cloned()
            .collect::<Vec<_>>();
        for stale_key in stale_keys {
            let stale_session = runtime
                .sessions
                .remove(&stale_key)
                .expect("stale foundation session remains cached");
            runtime
                .process
                .close_session(stale_session.id)
                .await
                .map_err(|error| ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!(
                        "Apple Foundation Models bridge session invalidation failed: {error}"
                    ),
                })?;
        }

        if let Some(session) = runtime.sessions.get(&key)
            && let Some(new_turns) = replay_turns.strip_prefix(session.replayed_history.as_slice())
        {
            let session_id = session.id.clone();
            if !new_turns.is_empty() {
                runtime
                    .process
                    .replay_turns(session_id.clone(), new_turns.to_vec())
                    .await
                    .map_err(|error| ProviderError::ProviderUnavailable {
                        provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                        message: format!(
                            "Apple Foundation Models bridge context update failed: {error}"
                        ),
                    })?;
                runtime
                    .sessions
                    .get_mut(&key)
                    .expect("foundation session remains cached")
                    .replayed_history = replay_turns;
            }
            return Ok(session_id);
        }
        if let Some(stale_session) = runtime.sessions.remove(&key) {
            runtime
                .process
                .close_session(stale_session.id)
                .await
                .map_err(|error| ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!(
                        "Apple Foundation Models bridge session reset failed: {error}"
                    ),
                })?;
        }
        let session_id = runtime
            .process
            .create_session(
                key.conversation_id.clone(),
                key.model_profile.clone(),
                key.static_instructions.clone(),
            )
            .await
            .map_err(|error| ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!("Apple Foundation Models bridge session failed: {error}"),
            })?;
        if !replay_turns.is_empty() {
            runtime
                .process
                .replay_turns(session_id.clone(), replay_turns.clone())
                .await
                .map_err(|error| ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!("Apple Foundation Models bridge replay failed: {error}"),
                })?;
        }
        runtime.sessions.insert(
            key,
            FoundationSession {
                id: session_id.clone(),
                replayed_history: replay_turns,
            },
        );
        Ok(session_id)
    }
}

fn transient_conversation_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("conversation:foundation-local-{sequence}")
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

    fn default_tool_classification_model(&self) -> Option<String> {
        Some(self.config.default_profile.clone())
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
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
        let prompt = foundation_prompt_parts(&request.input);
        let mut noema_delta_extractor = NoemaAssistantTextDeltaExtractor::default();
        let mut relay_delta = |delta: String| {
            if require_noema_response {
                noema_delta_extractor.push_delta(&delta, on_event);
            } else {
                on_event(GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta,
                });
            }
        };
        let key = FoundationSessionKey {
            conversation_id,
            model_profile: model.clone(),
            static_instructions: request.instructions.clone(),
        };
        let mut guard = self.bridge_runtime.lock().await;
        self.ensure_bridge_runtime(&mut guard).await?;
        let runtime = guard.as_mut().expect("bridge runtime initialized");
        let session_id = self
            .session_for_request(runtime, key.clone(), prompt.replay_turns)
            .await?;
        let generated_input = prompt.generate_input.clone();
        let output_text = match runtime
            .process
            .generate_in_session(
                session_id.clone(),
                prompt.generate_input,
                request.options.max_output_tokens,
                &mut relay_delta,
            )
            .await
        {
            Ok(output_text) => output_text,
            Err(error) => {
                runtime.sessions.remove(&key);
                let _ = runtime.process.close_session(session_id).await;
                return Err(ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!("Apple Foundation Models bridge generation failed: {error}"),
                });
            }
        };
        let parsed = if require_noema_response {
            required_noema_response_from_text(output_text.clone())
        } else {
            output_items_from_text(output_text.clone()).map(|responses| ParsedNoemaResponse {
                responses,
                tool_calls: Vec::new(),
                response_status: GenerateResponseStatus::Final,
            })
        };
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                runtime.sessions.remove(&key);
                let _ = runtime.process.close_session(session_id).await;
                if require_noema_response {
                    StructuredResponseDiagnosticContext::new(
                        self.config.system_errors.clone(),
                        FOUNDATION_LOCAL_PROVIDER,
                        model.clone(),
                        request.conversation_id.clone(),
                    )
                    .log_malformed(
                        error.to_string(),
                        serde_json::json!({ "provider_text": output_text }),
                    );
                }
                return Err(error);
            }
        };
        if let Some(session) = runtime.sessions.get_mut(&key) {
            session.replayed_history.push(BridgeReplayTurn {
                role: BridgeRole::User,
                text: generated_input,
            });
            session
                .replayed_history
                .extend(bridge_replay_parsed_response(&parsed));
        }
        Ok(GenerateResponse::from_parsed(
            parsed,
            FOUNDATION_LOCAL_PROVIDER,
            model,
            None,
            None,
        ))
    }
}
