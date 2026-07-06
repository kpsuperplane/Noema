use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus, ReplayMode, SYSTEM_ERROR_RUNTIME_INVARIANT,
    SystemErrorEvent,
    memory::extraction::{ExtractorMemoryProposal, ValidatedMemoryProposal},
    provider::{
        GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
        GenerateStreamEvent, GenerateToolCall, PromptCacheRetention, ProviderError,
        ProviderToolCapabilities,
    },
};
use serde_json::json;
use tokio::sync::mpsc;

use super::{
    actor::CodexRuntimeActor,
    local_tools::{
        LocalToolResult, agent_identity_after_local_tools, local_tool_result_action_item,
        local_tool_result_continuation_input,
    },
    model_tools::{ModelTools, build_model_tools},
    progress::{
        ContinuationProgressTracker, DeterministicProgressStop, MAX_PROVIDER_TOOL_CONTINUATIONS,
    },
    progress_audit::{
        ProgressAuditDecision, ProgressAuditError, build_no_tools_finalization_prompt,
    },
    tool_lifecycle::local_tool_calls,
    transcript_persistence::{
        assistant_stream_id, handle_provider_stream_event, send_conversation_item,
    },
    turn_timing::TurnTiming,
};
use crate::daemon::{
    agent_name_tool::is_update_own_name_tool,
    agent_onboarding::AgentPromptIdentity,
    memory::pipeline::{AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_content},
    prompts::{
        PromptToolExposure, build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt, build_model_available_tools_prompt,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
    },
};

fn prompt_cache_retention_for(
    tool_capabilities: ProviderToolCapabilities,
) -> Option<PromptCacheRetention> {
    tool_capabilities
        .prompt_cache_retention
        .then_some(PromptCacheRetention::TwentyFourHours)
}

pub(super) fn mcp_health_status_label(status: crate::McpServerHealthStatus) -> &'static str {
    match status {
        crate::McpServerHealthStatus::Unknown => "unknown",
        crate::McpServerHealthStatus::Healthy => "healthy",
        crate::McpServerHealthStatus::Unavailable => "unavailable",
    }
}

pub(super) fn mcp_auth_status_label(status: crate::McpServerAuthStatus) -> &'static str {
    match status {
        crate::McpServerAuthStatus::None => "none",
        crate::McpServerAuthStatus::NeedsAuth => "needs_auth",
        crate::McpServerAuthStatus::Authenticated => "authenticated",
        crate::McpServerAuthStatus::Unavailable => "unavailable",
    }
}

impl CodexRuntimeActor {
    fn log_runtime_invariant(
        &self,
        message: impl Into<String>,
        context: serde_json::Value,
        raw: serde_json::Value,
    ) {
        let message = message.into();
        self.system_errors.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
                .with_context(context)
                .with_error_chain([message])
                .with_raw(raw),
        );
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn start_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection = super::conversation_state::provider_selection_for_conversation(
            &self.store,
            &self.default_provider_kind,
        )
        .await?;
        let new_conversation = crate::NewConversation::local_chat_for_provider(
            &selection.provider_kind,
            selection.model.clone(),
            cwd.clone(),
        );
        let durable_conversation = self.store.create_conversation(new_conversation).await?;
        let conversation_id = durable_conversation.conversation_id;
        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    pub(in crate::daemon) async fn start_primary_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection = super::conversation_state::provider_selection_for_conversation(
            &self.store,
            &self.default_provider_kind,
        )
        .await?;
        let durable_conversation = self
            .store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                &selection.provider_kind,
                selection.model.clone(),
                cwd.clone(),
            )
            .await?;
        let conversation_id = durable_conversation.conversation_id;

        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        self.ensure_initial_name_onboarding_message(&conversation_id)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    async fn ensure_initial_name_onboarding_message(
        &mut self,
        conversation_id: &str,
    ) -> Result<(), DaemonError> {
        let agent_identity = self
            .agent_identity_for_conversation(conversation_id)
            .await?;
        if agent_identity.display_name.is_some() {
            return Ok(());
        }
        let replay = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        if !replay.is_empty() {
            return Ok(());
        }

        let conversation = self
            .conversations
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.to_string(),
                trigger_item_id: None,
                metadata: json!({
                    "turn_index": turn_index,
                    "source": "agent_onboarding",
                }),
            })
            .await?;
        let instructions = build_initial_name_onboarding_system_prompt(
            conversation_id,
            turn_index,
            conversation.cwd.as_deref(),
            &agent_identity,
        );
        let provider = self.provider_for_kind(&conversation.provider_kind)?;
        let tool_capabilities = provider.tool_capabilities(conversation.model.as_deref());
        let response = match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.to_string()),
                    model: conversation.model.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        ..GenerateOptions::default()
                    },
                    tools: Vec::new(),
                    tool_choice: Default::default(),
                    parallel_tool_calls: false,
                },
                &mut |_| {},
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.store.fail_conversation_turn(&turn.turn_id).await?;
                self.conversations.remove(conversation_id);
                return Err(error.into());
            }
        };
        let raw_provider_response = json!({
            "provider": &response.provider,
            "model": &response.model,
            "response_id": &response.response_id,
            "usage": response.usage.as_ref().map(|usage| json!({
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "total_tokens": usage.total_tokens,
                "cached_input_tokens": usage.cached_input_tokens,
            })),
            "responses": &response.responses,
            "tool_calls": &response.tool_calls,
            "memory_proposals": &response.memory_proposals,
            "response_status": response.response_status,
        });
        let persisted_count = self
            .persist_agent_initiated_provider_response(
                conversation_id,
                &turn.turn_id,
                turn_index,
                response,
            )
            .await?;
        if persisted_count == 0 {
            self.store.fail_conversation_turn(&turn.turn_id).await?;
            self.conversations.remove(conversation_id);
            self.log_runtime_invariant(
                "initial onboarding response did not include assistant text",
                json!({
                    "conversation_id": conversation_id,
                    "turn_id": turn.turn_id,
                    "turn_index": turn_index,
                    "provider_kind": conversation.provider_kind,
                    "model": conversation.model,
                }),
                json!({
                    "persisted_count": persisted_count,
                    "provider_response": raw_provider_response,
                }),
            );
            return Err(DaemonError::Protocol(
                "initial onboarding response did not include assistant text".to_string(),
            ));
        }
        self.store.complete_conversation_turn(&turn.turn_id).await?;
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }
        Ok(())
    }

    pub(in crate::daemon) async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let pre_turn_started_at = std::time::Instant::now();
        let conversation = self
            .hydrate_active_conversation(&conversation_id, None)
            .await?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({ "turn_index": turn_index }),
            })
            .await?;
        let timing = TurnTiming::new(
            conversation_id.clone(),
            turn.turn_id.clone(),
            turn_index,
            client_message_id,
        );
        timing.mark(
            "runtime_turn_started",
            json!({
                "hydrate_and_create_turn_ms": pre_turn_started_at.elapsed().as_millis(),
                "input_chars": input.chars().count(),
                "provider_kind": conversation.provider_kind,
                "model": conversation.model,
            }),
        );
        let provider = self.provider_for_kind(&conversation.provider_kind)?;
        let tool_capabilities = provider.tool_capabilities(conversation.model.as_deref());
        let agent_identity = self
            .agent_identity_for_conversation(&conversation_id)
            .await?;
        let tools_started_at = std::time::Instant::now();
        let model_tools = self.model_tools(true, tool_capabilities).await?;
        let continuation_model_tools = self.model_tools(false, tool_capabilities).await?;
        let rendered_tools = render_available_tools(&model_tools);
        let rendered_continuation_tools = render_available_tools(&continuation_model_tools);
        timing.mark(
            "runtime_model_tools_ready",
            json!({
                "duration_ms": tools_started_at.elapsed().as_millis(),
                "native_tool_count": model_tools.native.len(),
                "legacy_builtin_tool_count": model_tools.legacy_builtin_envelope_tools.len(),
                "continuation_native_tool_count": continuation_model_tools.native.len(),
            }),
        );
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::InputReceived,
            &item_tx,
        )
        .await?;
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::Thinking,
            &item_tx,
        )
        .await?;
        timing.mark("runtime_status_thinking", json!({}));
        let prompt_started_at = std::time::Instant::now();
        let mut planned_context =
            super::prompt_context::plan_prompt_context(super::prompt_context::PromptPlanRequest {
                store: &self.store,
                provider: provider.as_ref(),
                conversation_id: &conversation_id,
                provider_kind: &conversation.provider_kind,
                model_profile: conversation.model.as_deref(),
                turn_index,
                cwd: conversation.cwd.as_deref(),
                agent_identity: &agent_identity,
                rendered_tools: &rendered_tools,
                native_tools_available: !model_tools.native.is_empty(),
                legacy_builtin_envelope_tools: &model_tools.legacy_builtin_envelope_tools,
                current_input: &input,
            })
            .await?;
        timing.mark(
            "runtime_prompt_context_planned",
            json!({
                "duration_ms": prompt_started_at.elapsed().as_millis(),
                "fits": planned_context.fits,
                "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                "budget_input_tokens": planned_context.budget.available_input_tokens(),
                "budget_output_reserve_tokens": planned_context.budget.output_reserve_tokens(),
            }),
        );
        let user_metadata = json!({ "turn_index": turn_index });
        let user_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(input.clone()),
                payload_json: json!({}),
                metadata: user_metadata.clone(),
            })
            .await?;
        let user_item_id = user_item.item_id.clone();
        send_conversation_item(
            &item_tx,
            user_item,
            user_metadata,
            TurnTranscriptItem::UserText {
                text: input.clone(),
            },
        );
        timing.mark("runtime_user_item_persisted", json!({}));
        let explicit_memory_outcome =
            if let Some(explicit_content) = explicit_memory_content(&input) {
                timing.mark("runtime_explicit_memory_started", json!({}));
                let memory_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                self.persist_explicit_memory_claim(&memory_context, &explicit_content, &item_tx)
                    .await?
            } else {
                ExplicitMemoryOutcome::None
            };
        if explicit_memory_outcome.was_attempted() {
            timing.mark(
                "runtime_explicit_memory_finished",
                json!({
                    "outcome": format!("{explicit_memory_outcome:?}"),
                }),
            );
        }
        if super::context_compaction::should_compact_foreground(&planned_context) {
            let compaction_started_at = std::time::Instant::now();
            timing.mark("runtime_foreground_compaction_started", json!({}));
            let compaction_result = super::context_compaction::compact_context_with_retry(
                super::context_compaction::CompactionRequest {
                    store: &self.store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &conversation.provider_kind,
                    model_profile: conversation.model.as_deref(),
                    budget: planned_context.budget,
                    mode: super::context_compaction::CompactionMode::Foreground,
                },
            )
            .await;
            if let Err(error) = compaction_result {
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                self.record_turn_failure_notice(
                    &error_context,
                    format!("Context compaction failed before this turn could run: {error}"),
                    true,
                    &item_tx,
                )
                .await?;
                self.conversations.remove(&conversation_id);
                return Err(error);
            }
            timing.mark(
                "runtime_foreground_compaction_finished",
                json!({
                    "duration_ms": compaction_started_at.elapsed().as_millis(),
                }),
            );
            let prompt_replan_started_at = std::time::Instant::now();
            planned_context = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &conversation.provider_kind,
                    model_profile: conversation.model.as_deref(),
                    turn_index,
                    cwd: conversation.cwd.as_deref(),
                    agent_identity: &agent_identity,
                    rendered_tools: &rendered_tools,
                    native_tools_available: !model_tools.native.is_empty(),
                    legacy_builtin_envelope_tools: &model_tools.legacy_builtin_envelope_tools,
                    current_input: &input,
                },
            )
            .await?;
            timing.mark(
                "runtime_prompt_context_replanned_after_compaction",
                json!({
                    "duration_ms": prompt_replan_started_at.elapsed().as_millis(),
                    "fits": planned_context.fits,
                    "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                }),
            );
            if !planned_context.fits {
                let smaller_compaction_started_at = std::time::Instant::now();
                timing.mark("runtime_smaller_compaction_started", json!({}));
                let smaller_compaction = super::context_compaction::compact_active_summary_smaller(
                    super::context_compaction::CompactionRequest {
                        store: &self.store,
                        provider: provider.as_ref(),
                        conversation_id: &conversation_id,
                        provider_kind: &conversation.provider_kind,
                        model_profile: conversation.model.as_deref(),
                        budget: planned_context.budget,
                        mode: super::context_compaction::CompactionMode::Foreground,
                    },
                )
                .await;
                if let Err(error) = smaller_compaction {
                    let error_context = ConversationMemoryContext {
                        turn_index,
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id.clone(),
                        user_item_id: user_item_id.clone(),
                        assistant_item_id: None,
                        assistant_items: Vec::new(),
                        user_content: input.clone(),
                        cwd: conversation.cwd.clone(),
                    };
                    self.record_turn_failure_notice(
                        &error_context,
                        format!("Context compaction failed before this turn could run: {error}"),
                        true,
                        &item_tx,
                    )
                    .await?;
                    self.conversations.remove(&conversation_id);
                    return Err(error);
                }
                timing.mark(
                    "runtime_smaller_compaction_finished",
                    json!({
                        "duration_ms": smaller_compaction_started_at.elapsed().as_millis(),
                    }),
                );
                let prompt_replan_started_at = std::time::Instant::now();
                planned_context = super::prompt_context::plan_prompt_context(
                    super::prompt_context::PromptPlanRequest {
                        store: &self.store,
                        provider: provider.as_ref(),
                        conversation_id: &conversation_id,
                        provider_kind: &conversation.provider_kind,
                        model_profile: conversation.model.as_deref(),
                        turn_index,
                        cwd: conversation.cwd.as_deref(),
                        agent_identity: &agent_identity,
                        rendered_tools: &rendered_tools,
                        native_tools_available: !model_tools.native.is_empty(),
                        legacy_builtin_envelope_tools: &model_tools.legacy_builtin_envelope_tools,
                        current_input: &input,
                    },
                )
                .await?;
                timing.mark(
                    "runtime_prompt_context_replanned_after_smaller_compaction",
                    json!({
                        "duration_ms": prompt_replan_started_at.elapsed().as_millis(),
                        "fits": planned_context.fits,
                        "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                    }),
                );
            }
            if !planned_context.fits {
                let error = ProviderError::InvalidRequest {
                    message: "context could not be compacted enough for the selected model"
                        .to_string(),
                };
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                self.record_turn_failure_notice(&error_context, error.to_string(), true, &item_tx)
                    .await?;
                self.conversations.remove(&conversation_id);
                return Err(error.into());
            }
        }
        let agent_identity_for_background = agent_identity.clone();

        let initial_stream_id = assistant_stream_id(&turn.turn_id, "initial");
        let mut initial_stream_seen = false;
        let mut initial_assistant_delta_seen = false;
        let initial_event_context = ConversationMemoryContext {
            turn_index,
            conversation_id: conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            user_item_id: user_item_id.clone(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: input.clone(),
            cwd: conversation.cwd.clone(),
        };
        let mut on_initial_event = |event| {
            if !initial_stream_seen {
                timing.mark(
                    "provider_initial_first_stream_event",
                    provider_stream_event_fields(&event),
                );
                initial_stream_seen = true;
            }
            if matches!(&event, GenerateStreamEvent::AssistantTextDelta { .. })
                && !initial_assistant_delta_seen
            {
                timing.mark(
                    "provider_initial_first_assistant_delta",
                    provider_stream_event_fields(&event),
                );
                initial_assistant_delta_seen = true;
            }
            if matches!(&event, GenerateStreamEvent::ToolCallStarted { .. }) {
                timing.mark(
                    "provider_initial_tool_call_started_streamed",
                    provider_stream_event_fields(&event),
                );
            }
            if matches!(&event, GenerateStreamEvent::MemoryProposalsStarted) {
                timing.mark(
                    "provider_initial_memory_proposals_started_streamed",
                    json!({}),
                );
            }
            handle_provider_stream_event(
                event,
                &item_tx,
                &initial_event_context,
                &initial_stream_id,
                0,
            );
        };

        timing.mark(
            "provider_initial_request_started",
            json!({
                "native_tool_count": model_tools.native.len(),
                "parallel_tool_calls": !model_tools.native.is_empty()
                    && tool_capabilities.parallel_tool_calls,
            }),
        );
        let initial_provider_started_at = std::time::Instant::now();
        match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.clone()),
                    model: conversation.model.clone(),
                    input: planned_context.input,
                    instructions: Some(planned_context.instructions),
                    options: GenerateOptions {
                        max_output_tokens: planned_context.budget.output_reserve_tokens(),
                        require_noema_response: true,
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        ..GenerateOptions::default()
                    },
                    tools: model_tools.native.clone(),
                    tool_choice: Default::default(),
                    parallel_tool_calls: !model_tools.native.is_empty()
                        && tool_capabilities.parallel_tool_calls,
                },
                &mut on_initial_event,
            )
            .await
        {
            Ok(response) => {
                timing.mark(
                    "provider_initial_response_completed",
                    json!({
                        "duration_ms": initial_provider_started_at.elapsed().as_millis(),
                        "response_count": response.responses.len(),
                        "tool_call_count": response.tool_calls.len(),
                        "memory_proposal_count": response.memory_proposals.len(),
                        "response_status": format!("{:?}", response.response_status),
                        "input_tokens": response.usage.as_ref().map(|usage| usage.input_tokens),
                        "output_tokens": response.usage.as_ref().map(|usage| usage.output_tokens),
                        "total_tokens": response.usage.as_ref().map(|usage| usage.total_tokens),
                        "cached_input_tokens": response
                            .usage
                            .as_ref()
                            .and_then(|usage| usage.cached_input_tokens),
                    }),
                );
                let result = self
                    .persist_successful_provider_turn(
                        SuccessfulProviderTurn {
                            conversation_id: conversation_id.clone(),
                            turn_id: turn.turn_id.clone(),
                            turn_index,
                            user_item_id: user_item_id.clone(),
                            user_input: input.clone(),
                            cwd: conversation.cwd.clone(),
                            provider_kind: conversation.provider_kind.clone(),
                            model: conversation.model.clone(),
                            initial_stream_id: initial_stream_id.clone(),
                            response,
                            explicit_memory_outcome,
                            agent_identity,
                            tool_capabilities,
                            continuation_model_tools,
                            rendered_tools: rendered_tools.clone(),
                            rendered_continuation_tools,
                        },
                        &item_tx,
                        &timing,
                    )
                    .await;
                if let Err(error) = result {
                    let failure_context = ConversationMemoryContext {
                        turn_index,
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id,
                        user_item_id,
                        assistant_item_id: None,
                        assistant_items: Vec::new(),
                        user_content: input,
                        cwd: conversation.cwd.clone(),
                    };
                    self.record_turn_failure(&failure_context, error.to_string(), &item_tx)
                        .await?;
                    self.conversations.remove(&conversation_id);
                    return Err(error);
                }
                self.schedule_background_context_compaction(BackgroundContextCompactionSchedule {
                    conversation_id: conversation_id.clone(),
                    provider_kind: conversation.provider_kind.clone(),
                    model_profile: conversation.model.clone(),
                    next_turn_index: turn_index.saturating_add(1),
                    cwd: conversation.cwd.clone(),
                    agent_identity: agent_identity_for_background,
                    rendered_tools: rendered_tools.clone(),
                });

                timing.mark("runtime_turn_ok", json!({}));
                Ok(())
            }
            Err(error) => {
                timing.mark(
                    "provider_initial_response_failed",
                    json!({
                        "duration_ms": initial_provider_started_at.elapsed().as_millis(),
                        "error": error.to_string(),
                    }),
                );
                let partial_output = match &error {
                    ProviderError::PartialResponse {
                        provider, output, ..
                    } => Some((provider.clone(), output.clone())),
                    ProviderError::MissingCredentials { .. }
                    | ProviderError::InvalidRequest { .. }
                    | ProviderError::HttpFailure { .. }
                    | ProviderError::ApiError { .. }
                    | ProviderError::RateLimit { .. }
                    | ProviderError::AuthenticationFailure { .. }
                    | ProviderError::MalformedResponse { .. }
                    | ProviderError::ProtocolError { .. }
                    | ProviderError::Timeout { .. }
                    | ProviderError::ProviderUnavailable { .. } => None,
                };
                let error_message = error.to_string();
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                if let Some((provider, output)) = partial_output {
                    let action_turn = ProviderActionTurn {
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id,
                        turn_index,
                        user_item_id,
                        provider,
                        stream_id: None,
                    };
                    self.persist_partial_provider_action_outputs(&action_turn, output, &item_tx)
                        .await?;
                }
                self.record_turn_failure(&error_context, error_message, &item_tx)
                    .await?;
                self.conversations.remove(&conversation_id);
                timing.mark("runtime_turn_failed", json!({}));
                Err(error.into())
            }
        }
    }

    async fn persist_successful_provider_turn(
        &mut self,
        turn: SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), DaemonError> {
        let persist_started_at = std::time::Instant::now();
        timing.mark("runtime_persist_successful_turn_started", json!({}));
        let initial_memory_proposals = turn.response.memory_proposals.clone();
        let initial_response_count = turn.response.responses.len();
        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: turn.response.provider.clone(),
            stream_id: Some(turn.initial_stream_id.clone()),
        };
        let mut initial_assistant_response = ProviderAssistantResponse::default();
        let initial_tool_calls = local_tool_calls(&turn.response.tool_calls);
        let initial_phase_has_tools = !initial_tool_calls.is_empty();
        for (index, response_item) in turn.response.responses.iter().cloned().enumerate() {
            self.persist_provider_response_item(
                &action_turn,
                index,
                response_item,
                initial_phase_has_tools,
                &mut initial_assistant_response,
                item_tx,
            )
            .await?;
            timing.mark(
                "runtime_assistant_response_item_persisted",
                json!({
                    "phase": "initial",
                    "response_index": index,
                }),
            );
        }

        let mut provider_memory_batches = Vec::new();
        if !initial_memory_proposals.is_empty() {
            provider_memory_batches.push(ProviderMemoryProposalBatch {
                context: ConversationMemoryContext {
                    turn_index: turn.turn_index,
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    assistant_item_id: initial_assistant_response.item_id.clone(),
                    assistant_items: initial_assistant_response.items.clone(),
                    user_content: turn.user_input.clone(),
                    cwd: turn.cwd.clone(),
                },
                proposals: initial_memory_proposals,
            });
        }

        let mut next_output_index = initial_response_count + initial_tool_calls.len();
        let local_action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: "noema_local".to_string(),
            stream_id: None,
        };
        let mut local_tool_results = Vec::new();
        for call in &initial_tool_calls {
            timing.mark(
                "runtime_tool_call_started",
                json!({
                    "phase": "initial",
                    "tool_name": call.name,
                    "output_index": call.output_index,
                }),
            );
            self.persist_provider_tool_call_started(
                &local_action_turn,
                initial_response_count + call.output_index,
                call,
                item_tx,
            )
            .await?;
            let tool_started_at = std::time::Instant::now();
            timing.mark(
                "runtime_tool_execution_started",
                json!({
                    "phase": "initial",
                    "tool_name": call.name,
                    "output_index": call.output_index,
                }),
            );
            let result = self
                .execute_local_tool(&turn, &turn.agent_identity, call)
                .await;
            timing.mark(
                "runtime_tool_execution_completed",
                json!({
                    "phase": "initial",
                    "tool_name": result.name(),
                    "duration_ms": tool_started_at.elapsed().as_millis(),
                    "success": result.success(),
                    "requires_provider_continuation": result.requires_provider_continuation(),
                }),
            );
            self.persist_provider_action_item(
                &local_action_turn,
                next_output_index,
                local_tool_result_action_item(&result),
                item_tx,
            )
            .await?;
            timing.mark(
                "runtime_tool_result_persisted",
                json!({
                    "phase": "initial",
                    "tool_name": result.name(),
                    "success": result.success(),
                }),
            );
            next_output_index += 1;
            local_tool_results.push(result);
        }
        let mut all_local_tool_results = local_tool_results.clone();
        let mut progress_tracker = ContinuationProgressTracker::new(&turn.user_input);
        progress_tracker.observe_results(&local_tool_results);

        let mut continuation_tool_results = local_tool_results
            .iter()
            .filter(|result| result.requires_provider_continuation())
            .cloned()
            .collect::<Vec<_>>();
        for continuation_step in 0..MAX_PROVIDER_TOOL_CONTINUATIONS {
            if continuation_tool_results.is_empty() {
                break;
            }
            let continuation_step_number = continuation_step + 1;
            progress_tracker.mark_continuation_step(continuation_step_number);
            if let Some(stop) = progress_tracker.deterministic_stop() {
                let reason = match stop {
                    DeterministicProgressStop::RepeatedArguments => "repeated tool arguments",
                    DeterministicProgressStop::FailureStreak => "repeated tool failures",
                };
                self.finalize_after_progress_stop(
                    &turn,
                    &all_local_tool_results,
                    next_output_index,
                    reason,
                    item_tx,
                    timing,
                )
                .await?;
                continuation_tool_results.clear();
                break;
            }
            if ContinuationProgressTracker::should_audit(continuation_step_number) {
                let audit_turn = ProviderActionTurn {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    turn_index: turn.turn_index,
                    user_item_id: turn.user_item_id.clone(),
                    provider: "noema_local".to_string(),
                    stream_id: None,
                };
                self.persist_progress_audit_started(&audit_turn, next_output_index, item_tx)
                    .await?;
                next_output_index += 1;
                let digest = progress_tracker.digest(continuation_step_number);
                match self.run_progress_audit(&digest).await {
                    Ok(outcome) => {
                        let label = match outcome.decision {
                            ProgressAuditDecision::Continue => "Still making progress",
                            ProgressAuditDecision::Finalize => "Ready to wrap up",
                            ProgressAuditDecision::AskHuman => "Needs your input",
                            ProgressAuditDecision::Checkpoint => "Paused with checkpoint",
                        };
                        self.persist_progress_audit_completed(
                            &audit_turn,
                            next_output_index,
                            label,
                            &outcome.user_summary,
                            item_tx,
                        )
                        .await?;
                        next_output_index += 1;
                        progress_tracker.update_current_goal(outcome.next_goal.clone());
                        progress_tracker.reset_window();
                        match outcome.decision {
                            ProgressAuditDecision::Continue => {}
                            ProgressAuditDecision::Finalize => {
                                self.finalize_after_progress_stop(
                                    &turn,
                                    &all_local_tool_results,
                                    next_output_index,
                                    "progress audit requested final answer",
                                    item_tx,
                                    timing,
                                )
                                .await?;
                                continuation_tool_results.clear();
                                break;
                            }
                            ProgressAuditDecision::AskHuman | ProgressAuditDecision::Checkpoint => {
                                self.persist_progress_pause_message(
                                    &turn,
                                    next_output_index,
                                    &outcome.user_summary,
                                    item_tx,
                                )
                                .await?;
                                continuation_tool_results.clear();
                                break;
                            }
                        }
                    }
                    Err(ProgressAuditError::Unavailable(message)) => {
                        self.persist_progress_audit_completed(
                            &audit_turn,
                            next_output_index,
                            "Progress check unavailable",
                            &message,
                            item_tx,
                        )
                        .await?;
                        next_output_index += 1;
                        progress_tracker.reset_window();
                    }
                    Err(ProgressAuditError::ExecutionFailed(message)) => {
                        self.persist_progress_audit_completed(
                            &audit_turn,
                            next_output_index,
                            "Progress check unavailable",
                            "The progress check failed, so I am pausing safely.",
                            item_tx,
                        )
                        .await?;
                        next_output_index += 1;
                        self.finalize_after_progress_stop(
                            &turn,
                            &all_local_tool_results,
                            next_output_index,
                            &message,
                            item_tx,
                            timing,
                        )
                        .await?;
                        continuation_tool_results.clear();
                        break;
                    }
                }
                if continuation_tool_results.is_empty() {
                    break;
                }
            }
            let continuation_agent_identity =
                agent_identity_after_local_tools(&turn.agent_identity, &all_local_tool_results);
            let continuation_result_refs = continuation_tool_results.iter().collect::<Vec<_>>();
            let continuation_input = if turn.tool_capabilities.native_tool_results {
                continuation_result_refs
                    .iter()
                    .map(|result| result.native_tool_result_input())
                    .collect::<Option<Vec<_>>>()
                    .map_or_else(
                        || {
                            GenerateInput::Text(
                                local_tool_result_continuation_input(&continuation_result_refs)
                                    .to_string(),
                            )
                        },
                        GenerateInput::NativeToolResults,
                    )
            } else {
                GenerateInput::Text(
                    local_tool_result_continuation_input(&continuation_result_refs).to_string(),
                )
            };
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                &turn.conversation_id,
                turn.turn_index,
                turn.cwd.as_deref(),
                &turn.user_input,
                &continuation_agent_identity,
                &turn.rendered_continuation_tools,
                PromptToolExposure {
                    native_tools_available: !turn.continuation_model_tools.native.is_empty(),
                    legacy_builtin_envelope_tools: &turn
                        .continuation_model_tools
                        .legacy_builtin_envelope_tools,
                },
            );
            let continuation_stream_suffix = if continuation_step == 0 {
                "continuation".to_string()
            } else {
                format!("continuation-{continuation_step}")
            };
            let continuation_stream_id =
                assistant_stream_id(&turn.turn_id, &continuation_stream_suffix);
            let continuation_output_base = next_output_index;
            let mut continuation_stream_seen = false;
            let mut continuation_assistant_delta_seen = false;
            let continuation_event_context = ConversationMemoryContext {
                turn_index: turn.turn_index,
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: turn.user_item_id.clone(),
                assistant_item_id: None,
                assistant_items: Vec::new(),
                user_content: turn.user_input.clone(),
                cwd: turn.cwd.clone(),
            };
            let mut on_continuation_event = |event| {
                if matches!(
                    &event,
                    GenerateStreamEvent::ToolCallStarted { name, .. }
                        if is_update_own_name_tool(name)
                ) {
                    return;
                }
                if !continuation_stream_seen {
                    timing.mark(
                        "provider_continuation_first_stream_event",
                        continuation_provider_stream_event_fields(continuation_step, &event),
                    );
                    continuation_stream_seen = true;
                }
                if matches!(&event, GenerateStreamEvent::AssistantTextDelta { .. })
                    && !continuation_assistant_delta_seen
                {
                    timing.mark(
                        "provider_continuation_first_assistant_delta",
                        continuation_provider_stream_event_fields(continuation_step, &event),
                    );
                    continuation_assistant_delta_seen = true;
                }
                if matches!(&event, GenerateStreamEvent::ToolCallStarted { .. }) {
                    timing.mark(
                        "provider_continuation_tool_call_started_streamed",
                        continuation_provider_stream_event_fields(continuation_step, &event),
                    );
                }
                if matches!(&event, GenerateStreamEvent::MemoryProposalsStarted) {
                    timing.mark(
                        "provider_continuation_memory_proposals_started_streamed",
                        json!({ "continuation_step": continuation_step }),
                    );
                }
                handle_provider_stream_event(
                    event,
                    item_tx,
                    &continuation_event_context,
                    &continuation_stream_id,
                    continuation_output_base,
                );
            };
            let provider = self.provider_for_kind(&turn.provider_kind)?;
            timing.mark(
                "provider_continuation_request_started",
                json!({
                    "continuation_step": continuation_step,
                    "tool_result_count": continuation_result_refs.len(),
                    "native_tool_count": turn.continuation_model_tools.native.len(),
                }),
            );
            let continuation_provider_started_at = std::time::Instant::now();
            let continuation_response = provider
                .generate_streaming(
                    GenerateRequest {
                        conversation_id: Some(turn.conversation_id.clone()),
                        model: turn.model.clone(),
                        input: continuation_input,
                        instructions: Some(continuation_instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
                            prompt_cache_retention: prompt_cache_retention_for(
                                turn.tool_capabilities,
                            ),
                            ..GenerateOptions::default()
                        },
                        tools: turn.continuation_model_tools.native.clone(),
                        tool_choice: Default::default(),
                        parallel_tool_calls: !turn.continuation_model_tools.native.is_empty()
                            && turn.tool_capabilities.parallel_tool_calls,
                    },
                    &mut on_continuation_event,
                )
                .await?;
            timing.mark(
                "provider_continuation_response_completed",
                json!({
                    "continuation_step": continuation_step,
                    "duration_ms": continuation_provider_started_at.elapsed().as_millis(),
                    "response_count": continuation_response.responses.len(),
                    "tool_call_count": continuation_response.tool_calls.len(),
                    "memory_proposal_count": continuation_response.memory_proposals.len(),
                    "response_status": format!("{:?}", continuation_response.response_status),
                    "input_tokens": continuation_response
                        .usage
                        .as_ref()
                        .map(|usage| usage.input_tokens),
                    "output_tokens": continuation_response
                        .usage
                        .as_ref()
                        .map(|usage| usage.output_tokens),
                    "total_tokens": continuation_response
                        .usage
                        .as_ref()
                        .map(|usage| usage.total_tokens),
                    "cached_input_tokens": continuation_response
                        .usage
                        .as_ref()
                        .and_then(|usage| usage.cached_input_tokens),
                }),
            );
            let continuation_memory_proposals = continuation_response.memory_proposals.clone();
            let mut continuation_assistant_response = ProviderAssistantResponse::default();
            let continuation_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: continuation_response.provider.clone(),
                stream_id: Some(continuation_stream_id.clone()),
            };
            let continuation_tool_call_items = continuation_response
                .tool_calls
                .iter()
                .filter(|call| !is_disallowed_continuation_tool_call(call))
                .cloned()
                .collect::<Vec<_>>();
            let continuation_response_count = continuation_response.responses.len();
            let continuation_tool_calls =
                if continuation_response.response_status == GenerateResponseStatus::NeedsTools {
                    local_tool_calls(&continuation_tool_call_items)
                } else {
                    Vec::new()
                };
            let continuation_phase_has_tools = !continuation_tool_calls.is_empty();
            for (offset, response_item) in
                continuation_response.responses.iter().cloned().enumerate()
            {
                self.persist_provider_response_item(
                    &continuation_action_turn,
                    continuation_output_base + offset,
                    response_item,
                    continuation_phase_has_tools,
                    &mut continuation_assistant_response,
                    item_tx,
                )
                .await?;
                timing.mark(
                    "runtime_assistant_response_item_persisted",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "response_index": continuation_output_base + offset,
                    }),
                );
            }
            if !continuation_memory_proposals.is_empty() {
                provider_memory_batches.push(ProviderMemoryProposalBatch {
                    context: ConversationMemoryContext {
                        turn_index: turn.turn_index,
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: turn.turn_id.clone(),
                        user_item_id: turn.user_item_id.clone(),
                        assistant_item_id: continuation_assistant_response.item_id.clone(),
                        assistant_items: continuation_assistant_response.items.clone(),
                        user_content: turn.user_input.clone(),
                        cwd: turn.cwd.clone(),
                    },
                    proposals: continuation_memory_proposals,
                });
            }
            next_output_index += continuation_response_count + continuation_tool_calls.len();

            let continuation_turn = SuccessfulProviderTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                user_input: turn.user_input.clone(),
                cwd: turn.cwd.clone(),
                provider_kind: turn.provider_kind.clone(),
                model: turn.model.clone(),
                initial_stream_id: continuation_stream_id.clone(),
                response: GenerateResponse {
                    responses: continuation_response.responses.clone(),
                    tool_calls: continuation_tool_call_items,
                    memory_proposals: continuation_response.memory_proposals.clone(),
                    response_status: continuation_response.response_status,
                    provider: continuation_response.provider.clone(),
                    model: continuation_response.model.clone(),
                    response_id: continuation_response.response_id.clone(),
                    usage: continuation_response.usage.clone(),
                },
                explicit_memory_outcome: ExplicitMemoryOutcome::None,
                agent_identity: continuation_agent_identity,
                tool_capabilities: turn.tool_capabilities,
                continuation_model_tools: turn.continuation_model_tools.clone(),
                rendered_tools: turn.rendered_tools.clone(),
                rendered_continuation_tools: turn.rendered_continuation_tools.clone(),
            };
            let mut local_tool_results = Vec::new();
            let local_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: "noema_local".to_string(),
                stream_id: None,
            };
            for call in &continuation_tool_calls {
                timing.mark(
                    "runtime_tool_call_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                self.persist_provider_tool_call_started(
                    &local_action_turn,
                    continuation_output_base + continuation_response_count + call.output_index,
                    call,
                    item_tx,
                )
                .await?;
                let tool_started_at = std::time::Instant::now();
                timing.mark(
                    "runtime_tool_execution_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                let result = self
                    .execute_local_tool(&continuation_turn, &continuation_turn.agent_identity, call)
                    .await;
                timing.mark(
                    "runtime_tool_execution_completed",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": result.name(),
                        "duration_ms": tool_started_at.elapsed().as_millis(),
                        "success": result.success(),
                        "requires_provider_continuation": result.requires_provider_continuation(),
                    }),
                );
                self.persist_provider_action_item(
                    &local_action_turn,
                    next_output_index,
                    local_tool_result_action_item(&result),
                    item_tx,
                )
                .await?;
                timing.mark(
                    "runtime_tool_result_persisted",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": result.name(),
                        "success": result.success(),
                    }),
                );
                next_output_index += 1;
                local_tool_results.push(result);
            }
            continuation_tool_results = local_tool_results
                .iter()
                .filter(|result| result.requires_provider_continuation())
                .cloned()
                .collect::<Vec<_>>();
            progress_tracker.observe_results(&local_tool_results);
            all_local_tool_results.extend(local_tool_results.clone());
        }
        if !continuation_tool_results.is_empty() {
            self.finalize_after_progress_stop(
                &turn,
                &all_local_tool_results,
                next_output_index,
                "maximum provider tool continuations reached",
                item_tx,
                timing,
            )
            .await?;
        }

        if !turn.explicit_memory_outcome.was_attempted() && !provider_memory_batches.is_empty() {
            let memory_started_at = std::time::Instant::now();
            timing.mark(
                "runtime_provider_memory_persistence_started",
                json!({
                    "batch_count": provider_memory_batches.len(),
                }),
            );
            let memory_provider = self.provider_for_kind(&turn.provider_kind)?;
            self.persist_provider_memory_proposals(
                provider_memory_batches,
                memory_provider,
                item_tx,
            )
            .await?;
            timing.mark(
                "runtime_provider_memory_persistence_completed",
                json!({
                    "duration_ms": memory_started_at.elapsed().as_millis(),
                }),
            );
        }

        self.store.complete_conversation_turn(&turn.turn_id).await?;
        timing.mark(
            "runtime_turn_persistence_completed",
            json!({
                "duration_ms": persist_started_at.elapsed().as_millis(),
            }),
        );
        self.update_conversation_agent_status(
            &turn.conversation_id,
            PersistedAgentStatus::Idle,
            item_tx,
        )
        .await?;
        timing.mark("runtime_status_idle", json!({}));

        if let Some(conversation) = self.conversations.get_mut(&turn.conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }

        Ok(())
    }

    pub(in crate::daemon) async fn update_conversation_agent_status(
        &mut self,
        conversation_id: &str,
        status: PersistedAgentStatus,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.store
            .update_conversation_agent_status(conversation_id, status)
            .await?;
        let _ = item_tx.send(TurnStreamEvent::AgentStatusChanged {
            conversation_id: conversation_id.to_string(),
            status: AgentStatus::from(status),
        });
        Ok(())
    }

    async fn finalize_after_progress_stop(
        &mut self,
        turn: &SuccessfulProviderTurn,
        results: &[LocalToolResult],
        index: usize,
        reason: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), DaemonError> {
        let result_refs = results.iter().collect::<Vec<_>>();
        let provider = self.provider_for_kind(&turn.provider_kind)?;
        let mut ignore_event = |_| {};
        timing.mark(
            "provider_progress_finalization_request_started",
            json!({
                "reason": reason,
                "tool_result_count": result_refs.len(),
            }),
        );
        let started_at = std::time::Instant::now();
        let response = provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(turn.conversation_id.clone()),
                    model: turn.model.clone(),
                    input: GenerateInput::Text(
                        local_tool_result_continuation_input(&result_refs).to_string(),
                    ),
                    instructions: Some(build_no_tools_finalization_prompt(reason)),
                    options: GenerateOptions {
                        require_noema_response: true,
                        prompt_cache_retention: prompt_cache_retention_for(turn.tool_capabilities),
                        ..GenerateOptions::default()
                    },
                    tools: Vec::new(),
                    tool_choice: Default::default(),
                    parallel_tool_calls: false,
                },
                &mut ignore_event,
            )
            .await?;
        timing.mark(
            "provider_progress_finalization_response_completed",
            json!({
                "duration_ms": started_at.elapsed().as_millis(),
                "response_count": response.responses.len(),
                "ignored_tool_call_count": response.tool_calls.len(),
                "memory_proposal_count": response.memory_proposals.len(),
            }),
        );

        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: response.provider.clone(),
            stream_id: None,
        };
        let mut assistant_response = ProviderAssistantResponse::default();
        for (offset, response_item) in response.responses.into_iter().enumerate() {
            self.persist_provider_response_item(
                &action_turn,
                index + offset,
                response_item,
                false,
                &mut assistant_response,
                item_tx,
            )
            .await?;
        }
        Ok(())
    }

    async fn persist_progress_pause_message(
        &mut self,
        turn: &SuccessfulProviderTurn,
        index: usize,
        summary: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let metadata = json!({
            "turn_index": turn.turn_index,
            "response_index": index,
            "phase": "final_answer",
            "source": "progress_audit_pause",
        });
        let assistant_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: turn.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: Some(turn.user_item_id.clone()),
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
                content_text: Some(summary.to_string()),
                payload_json: json!({}),
                metadata: metadata.clone(),
            })
            .await?;
        send_conversation_item(
            item_tx,
            assistant_item,
            metadata,
            TurnTranscriptItem::AssistantText {
                text: summary.to_string(),
            },
        );
        Ok(())
    }

    async fn agent_identity_for_conversation(
        &self,
        _conversation_id: &str,
    ) -> Result<AgentPromptIdentity, DaemonError> {
        let agent_id = "agent:primary".to_string();
        let agent = self
            .store
            .get_agent(&agent_id)
            .await?
            .ok_or_else(|| DaemonError::Protocol(format!("unknown agent id: {agent_id}")))?;
        Ok(AgentPromptIdentity {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
        })
    }

    async fn model_tools(
        &self,
        include_agent_name_tool: bool,
        capabilities: ProviderToolCapabilities,
    ) -> Result<ModelTools, DaemonError> {
        build_model_tools(&self.store, include_agent_name_tool, capabilities)
            .await
            .map_err(|error| DaemonError::Protocol(error.to_string()))
    }

    fn schedule_background_context_compaction(
        &self,
        schedule: BackgroundContextCompactionSchedule,
    ) {
        let store = self.store.clone();
        let Ok(provider) = self.provider_for_kind(&schedule.provider_kind) else {
            return;
        };
        tokio::spawn(async move {
            let BackgroundContextCompactionSchedule {
                conversation_id,
                provider_kind,
                model_profile,
                next_turn_index,
                cwd,
                agent_identity,
                rendered_tools,
            } = schedule;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            match store.next_conversation_turn_index(&conversation_id).await {
                Ok(current_next_turn_index) if current_next_turn_index == next_turn_index => {}
                Ok(_) | Err(_) => return,
            }
            let plan = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
                    turn_index: next_turn_index,
                    cwd: cwd.as_deref(),
                    agent_identity: &agent_identity,
                    rendered_tools: &rendered_tools,
                    native_tools_available: false,
                    legacy_builtin_envelope_tools: &[],
                    current_input: "",
                },
            )
            .await;
            let Ok(plan) = plan else {
                return;
            };
            if !super::context_compaction::should_compact_background(&plan) {
                return;
            }
            let result = super::context_compaction::compact_context(
                super::context_compaction::CompactionRequest {
                    store: &store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
                    budget: plan.budget,
                    mode: super::context_compaction::CompactionMode::Background,
                },
            )
            .await;
            if let Err(error) = result {
                let _ = super::context_compaction::record_failed_background_compaction(
                    &store,
                    &conversation_id,
                    &provider_kind,
                    model_profile.as_deref(),
                    &error,
                )
                .await;
            }
        });
    }
}

fn render_available_tools(model_tools: &ModelTools) -> String {
    let mut rows =
        Vec::with_capacity(model_tools.prompt_rows.len() + model_tools.unavailable_rows.len());
    rows.extend(model_tools.prompt_rows.iter().cloned());
    rows.extend(model_tools.unavailable_rows.iter().cloned());
    build_model_available_tools_prompt(&rows)
}

fn provider_stream_event_fields(event: &GenerateStreamEvent) -> serde_json::Value {
    match event {
        GenerateStreamEvent::AssistantTextDelta {
            response_index,
            delta,
        } => json!({
            "stream_event": "assistant_text_delta",
            "response_index": response_index,
            "delta_chars": delta.chars().count(),
        }),
        GenerateStreamEvent::MemoryProposalsStarted => json!({
            "stream_event": "memory_proposals_started",
        }),
        GenerateStreamEvent::ToolCallStarted { output_index, name } => json!({
            "stream_event": "tool_call_started",
            "output_index": output_index,
            "tool_name": name,
        }),
    }
}

fn continuation_provider_stream_event_fields(
    continuation_step: usize,
    event: &GenerateStreamEvent,
) -> serde_json::Value {
    let mut fields = provider_stream_event_fields(event);
    fields["continuation_step"] = json!(continuation_step);
    fields
}

#[derive(Debug)]
struct BackgroundContextCompactionSchedule {
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    next_turn_index: u64,
    cwd: Option<String>,
    agent_identity: AgentPromptIdentity,
    rendered_tools: String,
}

#[derive(Debug)]
pub(in crate::daemon) struct SuccessfulProviderTurn {
    pub(in crate::daemon) conversation_id: String,
    pub(in crate::daemon) turn_id: String,
    pub(in crate::daemon) turn_index: u64,
    pub(in crate::daemon) user_item_id: String,
    pub(in crate::daemon) user_input: String,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) provider_kind: String,
    pub(in crate::daemon) model: Option<String>,
    pub(in crate::daemon) initial_stream_id: String,
    pub(in crate::daemon) response: GenerateResponse,
    pub(in crate::daemon) explicit_memory_outcome: ExplicitMemoryOutcome,
    pub(in crate::daemon) agent_identity: AgentPromptIdentity,
    pub(in crate::daemon) tool_capabilities: ProviderToolCapabilities,
    pub(in crate::daemon) continuation_model_tools: ModelTools,
    pub(in crate::daemon) rendered_tools: String,
    pub(in crate::daemon) rendered_continuation_tools: String,
}

#[derive(Debug)]
pub(in crate::daemon) struct ProviderMemoryProposalBatch {
    pub(in crate::daemon) context: ConversationMemoryContext,
    pub(in crate::daemon) proposals: Vec<ExtractorMemoryProposal>,
}

#[derive(Debug, Default)]
pub(in crate::daemon) struct ProviderAssistantResponse {
    pub(in crate::daemon) item_id: Option<String>,
    pub(in crate::daemon) text: String,
    pub(in crate::daemon) items: Vec<AssistantEvidenceItem>,
}

impl ProviderAssistantResponse {
    pub(in crate::daemon) fn push_text(&mut self, text: &str) {
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(text);
    }
}

#[derive(Debug)]
pub(in crate::daemon) struct ValidatedProviderMemoryProposal {
    pub(in crate::daemon) context: ConversationMemoryContext,
    pub(in crate::daemon) proposal: ValidatedMemoryProposal,
    pub(in crate::daemon) proposal_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) enum ExplicitMemoryOutcome {
    None,
    Saved,
    Failed,
}

impl ExplicitMemoryOutcome {
    pub(in crate::daemon) fn was_attempted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

pub(in crate::daemon) struct ProviderActionTurn {
    pub(in crate::daemon) conversation_id: String,
    pub(in crate::daemon) turn_id: String,
    pub(in crate::daemon) turn_index: u64,
    pub(in crate::daemon) user_item_id: String,
    pub(in crate::daemon) provider: String,
    pub(in crate::daemon) stream_id: Option<String>,
}

pub(in crate::daemon) struct ProviderActionOutput {
    pub(in crate::daemon) index: usize,
    pub(in crate::daemon) kind: ConversationItemKind,
    pub(in crate::daemon) status: ConversationItemStatus,
    pub(in crate::daemon) action_kind: &'static str,
    pub(in crate::daemon) title: String,
    pub(in crate::daemon) summary: Option<String>,
    pub(in crate::daemon) payload: serde_json::Value,
    pub(in crate::daemon) display: serde_json::Value,
}

fn is_disallowed_continuation_tool_call(call: &GenerateToolCall) -> bool {
    is_update_own_name_tool(&call.name)
}
