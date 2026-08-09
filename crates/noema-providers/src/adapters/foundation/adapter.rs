use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::Mutex;

use crate::{
    FoundationLocalProviderConfig, GenerateInput, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateStreamEvent, GenerateToolCall, ModelProvider,
    ProviderContextMetadata, ProviderError, ProviderResponseContinuation, ProviderSchemaRequest,
    ProviderSchemaRequestCapabilities, ProviderTool, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport,
};

use super::{
    bridge::{
        BridgeReplayToolResult, BridgeReplayTurn, BridgeRole, BridgeToolDefinition,
        BridgeToolResult, FoundationBridgeProcess, FoundationGeneration,
    },
    lowering::{bridge_replay_response, foundation_prompt_parts},
};

/// Provider identifier for Apple Foundation Models.
pub const FOUNDATION_LOCAL_PROVIDER: &str = "foundation_local";
/// Apple Foundation Models system context window.
const FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS: u32 = 4_096;
/// Default response reserve for Foundation Local prompts.
const FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 512;
/// Default compact summary target for Foundation Local.
const FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 512;

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
    pending_generation: Option<FoundationPendingGeneration>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FoundationSessionKey {
    conversation_id: String,
    model_profile: String,
    static_instructions: Option<String>,
    tool_catalog_fingerprint: String,
}

#[derive(Debug)]
struct FoundationSession {
    id: String,
    replayed_history: Vec<BridgeReplayTurn>,
}

#[derive(Clone, Debug)]
struct FoundationPendingGeneration {
    key: FoundationSessionKey,
    session_id: String,
    tools: Vec<ProviderTool>,
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
                pending_generation: None,
            });
        }
        Ok(())
    }

    async fn session_for_request(
        &self,
        runtime: &mut FoundationBridgeRuntime,
        key: FoundationSessionKey,
        replay_turns: Vec<BridgeReplayTurn>,
        bridge_tools: Vec<BridgeToolDefinition>,
    ) -> Result<String, ProviderError> {
        let stale_keys = runtime
            .sessions
            .keys()
            .filter(|existing| {
                existing.conversation_id == key.conversation_id
                    && (existing.model_profile != key.model_profile
                        || existing.static_instructions != key.static_instructions
                        || existing.tool_catalog_fingerprint != key.tool_catalog_fingerprint)
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
                bridge_tools,
                key.tool_catalog_fingerprint.clone(),
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

    async fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
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
            tool_transport: ProviderToolTransport::Native,
            tool_choice: false,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::FoundationLocal,
            request_strict_schema_when_possible: true,
            native_tool_results: true,
            ..ProviderToolCapabilities::default()
        }
    }

    fn schema_request_capabilities(
        &self,
        _model: Option<&str>,
    ) -> ProviderSchemaRequestCapabilities {
        ProviderSchemaRequestCapabilities {
            native_tool_arguments: ProviderSchemaRequest::RequestStrictWhenPossible,
        }
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::ActiveSession
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
        let is_tool_continuation = matches!(&request.input, GenerateInput::NativeToolResults(_));
        let mut relay_delta = |delta: String| {
            on_event(GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta,
            });
        };
        let mut guard = self.bridge_runtime.lock().await;
        self.ensure_bridge_runtime(&mut guard).await?;
        let runtime = guard.as_mut().expect("bridge runtime initialized");
        let (origin_key, session_id, response_tools, response_model, generated_input) =
            if is_tool_continuation {
                let pending = runtime.pending_generation.clone().ok_or_else(|| {
                    ProviderError::InvalidRequest {
                        message:
                            "Foundation tool results arrived without their originating session"
                                .to_string(),
                    }
                })?;
                if pending.key.conversation_id != conversation_id {
                    return Err(ProviderError::InvalidRequest {
                        message: "Foundation tool results targeted a different conversation"
                            .to_string(),
                    });
                }
                if !runtime.sessions.contains_key(&pending.key) {
                    runtime.pending_generation = None;
                    return Err(ProviderError::InvalidRequest {
                        message: "Foundation tool results arrived for an expired session"
                            .to_string(),
                    });
                }
                let key = pending.key;
                let response_model = key.model_profile.clone();
                (key, pending.session_id, pending.tools, response_model, None)
            } else {
                if runtime.pending_generation.is_some() {
                    return Err(ProviderError::InvalidRequest {
                        message: "Foundation generation has pending native tool calls".to_string(),
                    });
                }
                let prompt = foundation_prompt_parts(&request.input);
                let bridge_tools = bridge_tool_definitions(&request.tools)?;
                let key = FoundationSessionKey {
                    conversation_id,
                    model_profile: model.clone(),
                    static_instructions: request.instructions.clone(),
                    tool_catalog_fingerprint: tool_catalog_fingerprint(&bridge_tools),
                };
                let session_id = self
                    .session_for_request(runtime, key.clone(), prompt.replay_turns, bridge_tools)
                    .await?;
                (
                    key,
                    session_id,
                    request.tools.clone(),
                    model,
                    Some(prompt.generate_input),
                )
            };
        let generation = match &request.input {
            GenerateInput::NativeToolResults(results) => {
                runtime
                    .process
                    .continue_generation(
                        &session_id,
                        results
                            .iter()
                            .map(|result| BridgeToolResult {
                                call_id: result.call_id.clone(),
                                output: result.output_json_string(),
                                is_error: !result.success,
                            })
                            .collect(),
                        &mut relay_delta,
                    )
                    .await
            }
            _ => {
                runtime
                    .process
                    .generate_in_session(
                        session_id.clone(),
                        generated_input
                            .clone()
                            .expect("fresh generation has provider input"),
                        request.options.max_output_tokens,
                        &mut relay_delta,
                    )
                    .await
            }
        };
        let generation = match generation {
            Ok(generation) => generation,
            Err(error) => {
                runtime.pending_generation = None;
                // A failed generation can leave Swift suspended inside a tool
                // continuation. Drop the whole bridge so the next request
                // starts from a clean process instead of reusing a poisoned
                // pending-generation slot.
                *guard = None;
                return Err(ProviderError::ProviderUnavailable {
                    provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                    message: format!("Apple Foundation Models bridge generation failed: {error}"),
                });
            }
        };
        let response = match foundation_response(generation, &response_tools, response_model) {
            Ok(response) => response,
            Err(error) => {
                runtime.pending_generation = None;
                *guard = None;
                return Err(error);
            }
        };
        if let Some(session) = runtime.sessions.get_mut(&origin_key) {
            match &request.input {
                GenerateInput::NativeToolResults(results) => {
                    session
                        .replayed_history
                        .extend(results.iter().map(|result| {
                            BridgeReplayTurn {
                                role: BridgeRole::Assistant,
                                text: String::new(),
                                tool_call: None,
                                tool_result: Some(BridgeReplayToolResult {
                                    call_id: result.call_id.clone(),
                                    tool_name: result
                                        .provider_name
                                        .clone()
                                        .unwrap_or_else(|| result.name.clone()),
                                    output: result.output_json_string(),
                                }),
                            }
                        }));
                }
                _ => session.replayed_history.push(BridgeReplayTurn {
                    role: BridgeRole::User,
                    text: generated_input.expect("fresh generation has provider input"),
                    tool_call: None,
                    tool_result: None,
                }),
            }
            session
                .replayed_history
                .extend(bridge_replay_response(&response));
        }
        if response.tool_calls.is_empty() {
            runtime.pending_generation = None;
        } else {
            runtime.pending_generation = Some(FoundationPendingGeneration {
                key: origin_key,
                session_id,
                tools: response_tools,
            });
        }
        Ok(response)
    }
}

fn foundation_response(
    generation: FoundationGeneration,
    tools: &[ProviderTool],
    model: String,
) -> Result<GenerateResponse, ProviderError> {
    let tool_calls = generation
        .tool_calls
        .into_iter()
        .map(|call| {
            let tool = tools
                .iter()
                .find(|tool| tool.exposed_name() == call.tool_name)
                .ok_or_else(|| ProviderError::MalformedResponse {
                    message: format!(
                        "Foundation Models called unknown native tool {:?}",
                        call.tool_name
                    ),
                })?;
            let payload: serde_json::Value =
                serde_json::from_str(&call.arguments).map_err(|error| {
                    ProviderError::MalformedResponse {
                        message: format!("Foundation tool arguments were invalid JSON: {error}"),
                    }
                })?;
            if !payload.is_object() {
                return Err(ProviderError::MalformedResponse {
                    message: "Foundation tool arguments must be a JSON object".to_string(),
                });
            }
            Ok(GenerateToolCall {
                id: None,
                provider_call_id: Some(call.call_id),
                provider_name: Some(call.tool_name),
                name: tool.canonical_spec().name.as_str().to_string(),
                payload,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    let responses = (!generation.text.trim().is_empty())
        .then_some(GenerateResponseItem::Text {
            phase: None,
            text: generation.text,
        })
        .into_iter()
        .collect();
    Ok(GenerateResponse {
        responses,
        tool_calls,
        reasoning_items: Vec::new(),
        hosted_web_searches: Vec::new(),
        citations: Vec::new(),
        provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
        model,
        response_id: None,
        usage: None,
    })
}

fn bridge_tool_definitions(
    tools: &[ProviderTool],
) -> Result<Vec<BridgeToolDefinition>, ProviderError> {
    tools
        .iter()
        .map(|tool| {
            serde_json::to_string(&tool.input_schema)
                .map(|parameters| BridgeToolDefinition {
                    name: tool.exposed_name().to_string(),
                    description: tool.description.clone(),
                    parameters,
                })
                .map_err(|error| ProviderError::InvalidRequest {
                    message: format!("failed to encode Foundation native tool schema: {error}"),
                })
        })
        .collect()
}

fn tool_catalog_fingerprint(tools: &[BridgeToolDefinition]) -> String {
    let encoded = serde_json::to_string(tools).unwrap_or_default();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in encoded.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capabilities::ToolSpec;

    #[test]
    fn foundation_response_rejects_non_object_native_tool_arguments() {
        let tool = ProviderTool::canonical(
            ToolSpec::new(
                "search_memory",
                "Search memory.",
                serde_json::json!({ "type": "object" }),
            )
            .expect("tool"),
        );
        let error = foundation_response(
            FoundationGeneration {
                text: String::new(),
                tool_calls: vec![super::super::bridge::BridgeToolCall {
                    call_id: "call-1".to_string(),
                    tool_name: "search_memory".to_string(),
                    arguments: "[]".to_string(),
                }],
            },
            &[tool],
            "default".to_string(),
        )
        .expect_err("array arguments must be rejected");

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "Foundation tool arguments must be a JSON object"
        ));
    }
}
