use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, McpCalibrationStatus,
    McpServerAuthStatus, McpServerHealthStatus, NewConversation, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus, ReplayMode,
    memory::extraction::{ExtractorMemoryProposal, ValidatedMemoryProposal},
    provider::{
        GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
        GenerateStreamEvent, ProviderError,
    },
};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tokio::sync::mpsc;

use super::{
    actor::{ActiveConversation, CachedToolSnapshot, CodexRuntimeActor},
    local_tools::{
        agent_identity_after_local_tools, local_tool_result_continuation_input,
        local_tool_result_output_item,
    },
    transcript_persistence::{
        assistant_stream_id, handle_provider_stream_event, send_conversation_item,
    },
};
use crate::daemon::{
    agent_onboarding::AgentPromptIdentity,
    memory_pipeline::{AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_content},
    prompts::{
        build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt, build_model_available_tools_prompt,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
    },
};

impl CodexRuntimeActor {
    pub(super) async fn start_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection =
            provider_selection_for_conversation(&self.store, &self.default_provider_kind, model)
                .await?;
        let new_conversation = NewConversation::local_chat_for_provider(
            &selection.provider_kind,
            selection.model.clone(),
            cwd.clone(),
        );
        let durable_conversation = self.store.create_conversation(new_conversation).await?;
        let conversation_id = durable_conversation.conversation_id;
        self.conversations.insert(
            conversation_id.clone(),
            ActiveConversation {
                provider_kind: selection.provider_kind,
                model: selection.model,
                cwd,
                next_turn_index: 1,
                tool_snapshot: None,
            },
        );

        Ok(StartedConversation { conversation_id })
    }

    pub(super) async fn start_primary_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection =
            provider_selection_for_conversation(&self.store, &self.default_provider_kind, model)
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

        if !self.conversations.contains_key(&conversation_id) {
            let next_turn_index = self
                .store
                .next_conversation_turn_index(&conversation_id)
                .await?;
            self.conversations.insert(
                conversation_id.clone(),
                ActiveConversation {
                    provider_kind: selection.provider_kind,
                    model: selection.model,
                    cwd,
                    next_turn_index,
                    tool_snapshot: None,
                },
            );
        }

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
                    model: conversation.model.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
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

    pub(super) async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let conversation = self
            .conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
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
        let _tool_snapshot = self.refresh_tool_snapshot(&conversation_id).await?;
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::InputReceived,
            &item_tx,
        )
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
        let tool_snapshot = self.refresh_tool_snapshot(&conversation_id).await?;
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
                rendered_tools: &tool_snapshot.rendered_tools,
                current_input: &input,
            })
            .await?;
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
                    rendered_tools: &tool_snapshot.rendered_tools,
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
                        rendered_tools: &tool_snapshot.rendered_tools,
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
                    model: conversation.model.clone(),
                    input: GenerateInput::Text(input.clone()),
                    instructions: Some(planned_context.instructions),
                    options: GenerateOptions {
                        max_output_tokens: planned_context.budget.output_reserve_tokens(),
                        require_noema_response: true,
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
                    rendered_tools: tool_snapshot.rendered_tools.clone(),
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
                    | ProviderError::UnsupportedFeature { .. }
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
        let has_local_tool_results = !local_tool_results.is_empty();
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
                    initial_output_count + offset,
                    local_tool_result_output_item(result),
                    item_tx,
                )
                .await?;
            }
        }

        let continuation_tool_results = local_tool_results
            .iter()
            .filter(|result| result.requires_provider_continuation())
            .collect::<Vec<_>>();
        if !continuation_tool_results.is_empty() {
            let continuation_agent_identity =
                agent_identity_after_local_tools(&turn.agent_identity, &local_tool_results);
            let continuation_input =
                local_tool_result_continuation_input(&continuation_tool_results);
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                &turn.conversation_id,
                turn.turn_index,
                turn.cwd.as_deref(),
                &turn.user_input,
                &continuation_agent_identity,
            );
            let continuation_stream_id = assistant_stream_id(&turn.turn_id, "continuation");
            let continuation_output_base = initial_output_count + local_tool_results.len();
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
                if !matches!(event, GenerateStreamEvent::ToolCallStarted { .. }) {
                    handle_provider_stream_event(
                        event,
                        item_tx,
                        &continuation_event_context,
                        &continuation_stream_id,
                        continuation_output_base,
                    );
                }
            };
            let provider = self.provider_for_kind(&turn.provider_kind)?;
            let continuation_response = provider
                .generate_streaming(
                    GenerateRequest {
                        model: turn.model.clone(),
                        input: GenerateInput::Text(continuation_input.to_string()),
                        instructions: Some(continuation_instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
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
            for (offset, output) in continuation_response.output.into_iter().enumerate() {
                if matches!(
                    output,
                    GenerateOutputItem::ToolCall { .. }
                        | GenerateOutputItem::ToolResult { .. }
                        | GenerateOutputItem::ApprovalRequest { .. }
                        | GenerateOutputItem::ApprovalResult { .. }
                ) {
                    continue;
                }
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
        }

        if !turn.explicit_memory_outcome.was_attempted() && !provider_memory_batches.is_empty() {
            self.persist_provider_memory_proposals(provider_memory_batches, item_tx)
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

    pub(super) async fn update_conversation_agent_status(
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

    async fn refresh_tool_snapshot(
        &mut self,
        conversation_id: &str,
    ) -> Result<CachedToolSnapshot, DaemonError> {
        let rendered_tools = self.render_available_tools().await?;
        let hash = stable_hash(&rendered_tools);
        let snapshot = CachedToolSnapshot {
            hash,
            rendered_tools,
        };
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            let should_replace = conversation
                .tool_snapshot
                .as_ref()
                .map(|cached| cached.hash != snapshot.hash)
                .unwrap_or(true);
            if should_replace {
                conversation.tool_snapshot = Some(snapshot.clone());
            }
        }
        Ok(snapshot)
    }

    async fn render_available_tools(&self) -> Result<String, DaemonError> {
        let mut rows = vec![
            "- builtin\tsearch_memory\tNoema built-in memory retrieval".to_string(),
            "- builtin\tupdate_own_name\tNoema built-in agent naming".to_string(),
        ];
        let servers = self.store.list_mcp_servers().await?;
        for server in servers {
            if !server.enabled
                || server.health_status != McpServerHealthStatus::Healthy
                || !matches!(
                    server.auth_status,
                    McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
                )
            {
                continue;
            }
            let tools = self
                .store
                .list_mcp_tools_for_server(&server.mcp_server_id)
                .await?;
            for tool in tools {
                let calibration = self.store.get_tool_calibration(&tool.mcp_tool_id).await?;
                let Some(calibration) = calibration else {
                    continue;
                };
                if calibration.status != McpCalibrationStatus::Ready
                    || calibration.reviewed_metadata_fingerprint.as_deref()
                        != Some(tool.metadata_fingerprint.as_str())
                {
                    continue;
                }
                let hint =
                    crate::mcp::autofill::compact_description_hint(tool.description.as_deref(), 96)
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

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

async fn provider_selection_for_conversation(
    store: &crate::NoemaStore,
    default_provider_kind: &str,
    conversation_model: Option<String>,
) -> Result<ConversationProviderSelection, DaemonError> {
    if conversation_model
        .as_ref()
        .is_some_and(|model| !model.trim().is_empty())
    {
        return Ok(ConversationProviderSelection {
            provider_kind: default_provider_kind.to_string(),
            model: conversation_model,
        });
    }
    let Some(preference) = store.get_agent_runtime_preference("agent:primary").await? else {
        return Ok(ConversationProviderSelection {
            provider_kind: default_provider_kind.to_string(),
            model: None,
        });
    };
    Ok(ConversationProviderSelection {
        provider_kind: preference.provider_kind,
        model: Some(preference.model_profile),
    })
}

#[derive(Debug, Clone)]
struct ConversationProviderSelection {
    provider_kind: String,
    model: Option<String>,
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
pub(super) struct SuccessfulProviderTurn {
    pub(super) conversation_id: String,
    pub(super) turn_id: String,
    pub(super) turn_index: u64,
    pub(super) user_item_id: String,
    pub(super) user_input: String,
    pub(super) cwd: Option<String>,
    pub(super) provider_kind: String,
    pub(super) model: Option<String>,
    pub(super) initial_stream_id: String,
    pub(super) response: GenerateResponse,
    pub(super) explicit_memory_outcome: ExplicitMemoryOutcome,
    pub(super) agent_identity: AgentPromptIdentity,
}

#[derive(Debug)]
pub(super) struct ProviderMemoryProposalBatch {
    pub(super) context: ConversationMemoryContext,
    pub(super) proposals: Vec<ExtractorMemoryProposal>,
}

#[derive(Debug, Default)]
pub(super) struct ProviderAssistantResponse {
    pub(super) item_id: Option<String>,
    pub(super) text: String,
    pub(super) items: Vec<AssistantEvidenceItem>,
}

impl ProviderAssistantResponse {
    pub(super) fn push_text(&mut self, text: &str) {
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(text);
    }
}

#[derive(Debug)]
pub(super) struct ValidatedProviderMemoryProposal {
    pub(super) context: ConversationMemoryContext,
    pub(super) proposal: ValidatedMemoryProposal,
    pub(super) proposal_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExplicitMemoryOutcome {
    None,
    Saved,
    Failed,
}

impl ExplicitMemoryOutcome {
    pub(super) fn was_attempted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

pub(super) struct ProviderActionTurn {
    pub(super) conversation_id: String,
    pub(super) turn_id: String,
    pub(super) turn_index: u64,
    pub(super) user_item_id: String,
    pub(super) provider: String,
    pub(super) stream_id: Option<String>,
}

pub(super) struct ProviderActionOutput {
    pub(super) index: usize,
    pub(super) kind: ConversationItemKind,
    pub(super) status: ConversationItemStatus,
    pub(super) action_kind: &'static str,
    pub(super) title: String,
    pub(super) summary: Option<String>,
    pub(super) payload: serde_json::Value,
}
