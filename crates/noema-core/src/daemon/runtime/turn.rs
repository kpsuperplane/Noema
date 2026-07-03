use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus, ReplayMode, SYSTEM_ERROR_RUNTIME_INVARIANT,
    SystemErrorEvent,
    mcp::{mcp_tool_ineligibility, prompt_safe_mcp_tool_description},
    memory::extraction::{ExtractorMemoryProposal, ValidatedMemoryProposal},
    provider::{
        GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
        GenerateStreamEvent, PromptCacheRetention, ProviderError,
    },
};
use serde_json::json;
use tokio::sync::mpsc;

use super::{
    actor::CodexRuntimeActor,
    local_tools::{
        agent_identity_after_local_tools, local_tool_result_continuation_input,
        local_tool_result_output_item,
    },
    transcript_persistence::{
        assistant_stream_id, handle_provider_stream_event, send_conversation_item,
    },
};
use crate::daemon::{
    agent_name_tool::is_update_own_name_tool,
    agent_onboarding::AgentPromptIdentity,
    memory::pipeline::{AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_content},
    prompts::{
        build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt, build_model_available_tools_prompt,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
    },
};

const MAX_PROVIDER_TOOL_CONTINUATIONS: usize = 6;

fn mcp_health_status_label(status: crate::McpServerHealthStatus) -> &'static str {
    match status {
        crate::McpServerHealthStatus::Unknown => "unknown",
        crate::McpServerHealthStatus::Healthy => "healthy",
        crate::McpServerHealthStatus::Unavailable => "unavailable",
    }
}

fn mcp_auth_status_label(status: crate::McpServerAuthStatus) -> &'static str {
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
        let response = match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.to_string()),
                    model: conversation.model.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                        ..GenerateOptions::default()
                    },
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
            "output": &response.output,
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
    ) -> Result<(), DaemonError> {
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
        let provider = self.provider_for_kind(&conversation.provider_kind)?;
        let agent_identity = self
            .agent_identity_for_conversation(&conversation_id)
            .await?;
        let rendered_tools = self.render_available_tools(true).await?;
        let rendered_continuation_tools = self.render_available_tools(false).await?;
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
                current_input: &input,
            })
            .await?;
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
        let explicit_memory_outcome =
            if let Some(explicit_content) = explicit_memory_content(&input) {
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
        if super::context_compaction::should_compact_foreground(&planned_context) {
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
                    current_input: &input,
                },
            )
            .await?;
            if !planned_context.fits {
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
                        current_input: &input,
                    },
                )
                .await?;
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
            handle_provider_stream_event(
                event,
                &item_tx,
                &initial_event_context,
                &initial_stream_id,
                0,
            );
        };

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
                        prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                        ..GenerateOptions::default()
                    },
                },
                &mut on_initial_event,
            )
            .await
        {
            Ok(response) => {
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
                            rendered_tools: rendered_tools.clone(),
                            rendered_continuation_tools,
                        },
                        &item_tx,
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

                Ok(())
            }
            Err(error) => {
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
                Err(error.into())
            }
        }
    }

    async fn persist_successful_provider_turn(
        &mut self,
        turn: SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let initial_memory_proposals = turn.response.memory_proposals();
        let initial_output_count = turn.response.output.len();
        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: turn.response.provider.clone(),
            stream_id: Some(turn.initial_stream_id.clone()),
        };
        let mut initial_assistant_response = ProviderAssistantResponse::default();
        for (index, output) in turn.response.output.iter().cloned().enumerate() {
            self.persist_provider_response_output_item(
                &action_turn,
                index,
                output,
                &mut initial_assistant_response,
                item_tx,
            )
            .await?;
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

        let local_tool_results = self.execute_local_tools(&turn, &turn.agent_identity).await;
        let mut all_local_tool_results = local_tool_results.clone();
        let has_local_tool_results = !local_tool_results.is_empty();
        let mut next_output_index = initial_output_count;
        if has_local_tool_results {
            let local_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: "noema_local".to_string(),
                stream_id: None,
            };
            for (offset, result) in local_tool_results.iter().enumerate() {
                self.persist_provider_action_output_item(
                    &local_action_turn,
                    next_output_index + offset,
                    local_tool_result_output_item(result),
                    item_tx,
                )
                .await?;
            }
            next_output_index += local_tool_results.len();
        }

        let mut continuation_tool_results = local_tool_results
            .iter()
            .filter(|result| result.requires_provider_continuation())
            .cloned()
            .collect::<Vec<_>>();
        for continuation_step in 0..MAX_PROVIDER_TOOL_CONTINUATIONS {
            if continuation_tool_results.is_empty() {
                break;
            }
            let continuation_agent_identity =
                agent_identity_after_local_tools(&turn.agent_identity, &all_local_tool_results);
            let continuation_result_refs = continuation_tool_results.iter().collect::<Vec<_>>();
            let continuation_input =
                local_tool_result_continuation_input(&continuation_result_refs);
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                &turn.conversation_id,
                turn.turn_index,
                turn.cwd.as_deref(),
                &turn.user_input,
                &continuation_agent_identity,
                &turn.rendered_continuation_tools,
            );
            let continuation_stream_suffix = if continuation_step == 0 {
                "continuation".to_string()
            } else {
                format!("continuation-{continuation_step}")
            };
            let continuation_stream_id =
                assistant_stream_id(&turn.turn_id, &continuation_stream_suffix);
            let continuation_output_base = next_output_index;
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
                handle_provider_stream_event(
                    event,
                    item_tx,
                    &continuation_event_context,
                    &continuation_stream_id,
                    continuation_output_base,
                );
            };
            let provider = self.provider_for_kind(&turn.provider_kind)?;
            let continuation_response = provider
                .generate_streaming(
                    GenerateRequest {
                        conversation_id: Some(turn.conversation_id.clone()),
                        model: turn.model.clone(),
                        input: GenerateInput::Text(continuation_input.to_string()),
                        instructions: Some(continuation_instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
                            prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
                            ..GenerateOptions::default()
                        },
                    },
                    &mut on_continuation_event,
                )
                .await?;
            let continuation_memory_proposals = continuation_response.memory_proposals();
            let mut continuation_assistant_response = ProviderAssistantResponse::default();
            let continuation_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: continuation_response.provider.clone(),
                stream_id: Some(continuation_stream_id.clone()),
            };
            let continuation_outputs_for_tools = continuation_response
                .output
                .iter()
                .filter(|output| !is_disallowed_continuation_output(output))
                .cloned()
                .collect::<Vec<_>>();
            let continuation_output_count = continuation_outputs_for_tools.len();
            for (offset, output) in continuation_outputs_for_tools.iter().cloned().enumerate() {
                self.persist_provider_response_output_item(
                    &continuation_action_turn,
                    continuation_output_base + offset,
                    output,
                    &mut continuation_assistant_response,
                    item_tx,
                )
                .await?;
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
            next_output_index += continuation_output_count;

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
                    output: continuation_outputs_for_tools,
                    provider: continuation_response.provider,
                    model: continuation_response.model,
                    response_id: continuation_response.response_id,
                    usage: continuation_response.usage,
                },
                explicit_memory_outcome: ExplicitMemoryOutcome::None,
                agent_identity: continuation_agent_identity,
                rendered_tools: turn.rendered_tools.clone(),
                rendered_continuation_tools: turn.rendered_continuation_tools.clone(),
            };
            let local_tool_results = self
                .execute_local_tools(&continuation_turn, &continuation_turn.agent_identity)
                .await;
            continuation_tool_results = local_tool_results
                .iter()
                .filter(|result| result.requires_provider_continuation())
                .cloned()
                .collect::<Vec<_>>();
            all_local_tool_results.extend(local_tool_results.clone());
            if !local_tool_results.is_empty() {
                let local_action_turn = ProviderActionTurn {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    turn_index: turn.turn_index,
                    user_item_id: turn.user_item_id.clone(),
                    provider: "noema_local".to_string(),
                    stream_id: None,
                };
                for (offset, result) in local_tool_results.iter().enumerate() {
                    self.persist_provider_action_output_item(
                        &local_action_turn,
                        next_output_index + offset,
                        local_tool_result_output_item(result),
                        item_tx,
                    )
                    .await?;
                }
                next_output_index += local_tool_results.len();
            }
        }
        if !continuation_tool_results.is_empty() {
            return Err(ProviderError::ProtocolError {
                provider: turn.provider_kind.clone(),
                message: "tool continuation limit exceeded".to_string(),
            }
            .into());
        }

        if !turn.explicit_memory_outcome.was_attempted() && !provider_memory_batches.is_empty() {
            let memory_provider = self.provider_for_kind(&turn.provider_kind)?;
            self.persist_provider_memory_proposals(
                provider_memory_batches,
                memory_provider,
                item_tx,
            )
            .await?;
        }

        self.store.complete_conversation_turn(&turn.turn_id).await?;
        self.update_conversation_agent_status(
            &turn.conversation_id,
            PersistedAgentStatus::Idle,
            item_tx,
        )
        .await?;

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

    async fn render_available_tools(
        &self,
        include_agent_name_tool: bool,
    ) -> Result<String, DaemonError> {
        let mut rows =
            vec!["- builtin\tsearch_memory\tNoema built-in memory retrieval".to_string()];
        if include_agent_name_tool {
            rows.push("- builtin\tupdate_own_name\tNoema built-in agent naming".to_string());
        }
        let servers = self.store.list_mcp_servers().await?;
        for server in servers {
            if server.enabled
                && (server.health_status != crate::McpServerHealthStatus::Healthy
                    || !matches!(
                        server.auth_status,
                        crate::McpServerAuthStatus::None
                            | crate::McpServerAuthStatus::Authenticated
                    ))
            {
                rows.push(format!(
                    "- unavailable_mcp\t{}\t{}\thealth={}\tauth={}",
                    server.mcp_server_id,
                    server.display_name,
                    mcp_health_status_label(server.health_status),
                    mcp_auth_status_label(server.auth_status),
                ));
            }
            let tools = self
                .store
                .list_mcp_tools_for_server(&server.mcp_server_id)
                .await?;
            for tool in tools {
                let calibration = self.store.get_tool_calibration(&tool.mcp_tool_id).await?;
                if mcp_tool_ineligibility(&server, &tool, calibration.as_ref()).is_some() {
                    continue;
                }
                let hint = prompt_safe_mcp_tool_description(tool.description.as_deref(), 96)
                    .unwrap_or_else(|| "MCP tool".to_string());
                rows.push(format!(
                    "- mcp\tmcp.{}.{}\t{}",
                    server.mcp_server_id, tool.name, hint
                ));
            }
        }
        Ok(build_model_available_tools_prompt(&rows))
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

fn is_disallowed_continuation_output(output: &GenerateOutputItem) -> bool {
    matches!(
        output,
        GenerateOutputItem::ToolCall { name, .. } if is_update_own_name_tool(name)
    )
}
