impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    async fn finalize_after_continuation_ceiling(
        &mut self,
        turn: &SuccessfulProviderTurn,
        provider_session: &mut dyn ProviderGenerationSession,
        context: &mut ContinuationContext,
        tool_result_count: usize,
        index: usize,
        task_handoff: bool,
        has_pending_results: bool,
        citation_sources: &mut CitationSourceRegistry,
        provider_round_index: usize,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), RuntimeError> {
        let reason = if task_handoff && tool_result_count > 0 {
            Some("background task handoff completed")
        } else if has_pending_results {
            Some("maximum provider tool continuations reached")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.finalize_after_progress_stop(
                turn,
                provider_session,
                context,
                index,
                reason,
                citation_sources,
                provider_round_index,
                item_tx,
                timing,
            )
            .await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn finalize_after_progress_stop(
        &mut self,
        turn: &SuccessfulProviderTurn,
        provider_session: &mut dyn ProviderGenerationSession,
        context: &mut ContinuationContext,
        index: usize,
        reason: &str,
        citation_sources: &mut CitationSourceRegistry,
        provider_round_index: usize,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), RuntimeError> {
        let provider = turn.provider_route.operations();
        let instructions = build_local_tool_result_continuation_system_prompt(false);
        let tools = turn.initial_model_tools.provider_tools();
        let hosted_web_search = turn.initial_model_tools.hosted_web_search();
        let tool_choice = if turn.tool_capabilities.allowed_tools
            && turn.initial_model_tools.transport == ProviderToolTransport::Native
            && !hosted_web_search
        {
            turn.initial_model_tools
                .allowed_tool_choice(NoemaAllowedToolsMode::Auto)
        } else {
            NoemaToolChoice::Auto
        };
        let parallel_tool_calls = turn.initial_model_tools.transport
            == ProviderToolTransport::Native
            && turn.initial_model_tools.has_callable_tools()
            && turn.tool_capabilities.parallel_tool_calls;
        if !provider_session.has_active_continuation() {
            context
                .compact_to_fit(
                    provider,
                    turn.model.as_deref(),
                    turn.tool_capabilities.native_tool_results,
                    &instructions,
                    &tools,
                    hosted_web_search,
                    None,
                    turn.reasoning_effort,
                    noema_providers::GenerationPriority::Foreground,
                    &turn.user_input,
                )
                .await
                .map_err(RuntimeError::Provider)?;
        }
        context.append_developer_message(build_no_tools_finalization_prompt(reason));
        let continuation_input =
            context.next_provider_input(turn.tool_capabilities.native_tool_results);
        let mut ignore_event = |_| {};
        timing.mark(
            "provider_progress_finalization_request_started",
            json!({
                "reason": reason,
            }),
        );
        let started_at = std::time::Instant::now();
        let finalization_debug = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            RuntimeDebugSpanCategory::Provider,
            "Final provider request",
            RuntimeDebugMetadata {
                provider: Some(turn.provider_kind.clone()),
                model: turn.model.clone(),
                phase: Some("finalization".to_string()),
                ..RuntimeDebugMetadata::default()
            },
        )
        .await;
        let response = provider_session
            .generate(
                GenerateRequest {
                    conversation_id: Some(turn.conversation_id.clone()),
                    model: turn.model.clone(),
                    input: continuation_input.replay.clone(),
                    instructions: Some(instructions.clone()),
                    options: GenerateOptions {
                        hosted_web_search,
                        max_output_tokens: turn.max_output_tokens,
                        prompt_cache_retention: prompt_cache_retention_for(turn.tool_capabilities),
                        prompt_cache_options: prompt_cache_options_for(turn.tool_capabilities),
                        prompt_cache_breakpoints: turn.prompt_cache_breakpoints.clone(),
                        reasoning_effort: turn.reasoning_effort,
                        fast_mode: turn.fast_mode,
                        ..GenerateOptions::default()
                    },
                    tools,
                    tool_transport: turn.initial_model_tools.transport,
                    tool_choice,
                    parallel_tool_calls,
                },
                continuation_input,
                &mut ignore_event,
            )
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                finalization_debug
                    .finish(
                        RuntimeDebugSpanStatus::Failed,
                        Some(provider_session_debug_metadata(provider_session)),
                    )
                    .await;
                return Err(RuntimeError::Provider(error));
            }
        };
        let usage = response.usage.as_ref();
        finalization_debug
            .finish(
                RuntimeDebugSpanStatus::Completed,
                Some(RuntimeDebugMetadata {
                    provider: Some(response.provider.clone()),
                    model: Some(response.model.clone()),
                    phase: Some("finalization".to_string()),
                    input_tokens: usage.map(|value| value.input_tokens),
                    cached_input_tokens: usage.and_then(|value| value.cached_input_tokens),
                    output_tokens: usage.map(|value| value.output_tokens),
                    total_tokens: usage.map(|value| value.total_tokens),
                    ..provider_session_debug_metadata(provider_session)
                }),
            )
            .await;
        timing.mark(
            "provider_progress_finalization_response_completed",
            json!({
                "duration_ms": started_at.elapsed().as_millis(),
                "response_count": response.responses.len(),
                "ignored_tool_call_count": response.tool_calls.len(),
            }),
        );

        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: response.provider.clone(),
            model: response.model.clone(),
            provider_round: provider_round_index,
            response_phase: "continuation",
            usage: response.usage.clone(),
            stream_id: None,
        };
        self.persist_provider_reasoning_items(
            &turn.conversation_id,
            &turn.turn_id,
            &response.provider,
            &response.reasoning_items,
        )
        .await?;
        self.persist_hosted_web_searches(
            &action_turn,
            index,
            &response.hosted_web_searches,
            item_tx,
        )
        .await?;
        let mut assistant_response = ProviderAssistantResponse::default();
        for response_item in response.assistant_response_texts() {
            let offset = response_item.response_index;
            self.persist_provider_response_item(
                &action_turn,
                ProviderResponsePosition {
                    response_index: offset,
                    output_index: Some(index + offset),
                },
                response_item,
                citation_sources,
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
    ) -> Result<(), RuntimeError> {
        let metadata = json!({
            "turn_index": turn.turn_index,
            "response_index": index,
            "phase": "final_answer",
            "source": "progress_audit_pause",
        });
        let Some((assistant_item, _)) = self
            .persist_provider_assistant_text(
                None,
                &CitationSourceRegistry,
                summary.to_string(),
                &[],
                "progress_audit",
                &turn.turn_id,
                NewConversationItem {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: Some(turn.user_item_id.clone()),
                    kind: ConversationItemKind::AssistantText,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::new("agent:primary")
                        .expect("static primary agent id must be valid"),
                    content_text: None,
                    payload_json: json!({}),
                    metadata: metadata.clone(),
                },
            )
            .await?
        else {
            return Ok(());
        };
        let text = assistant_item.content_text.clone().unwrap_or_default();
        let metadata = assistant_item.metadata.clone();
        send_conversation_item(
            item_tx,
            assistant_item,
            metadata,
            TurnTranscriptItem::AssistantText { text },
        );
        Ok(())
    }

    pub(super) async fn agent_identity_for_conversation(
        &self,
    ) -> Result<AgentPromptIdentity, RuntimeError> {
        let agent_id = "agent:primary".to_string();
        let agent = self
            .store
            .get_agent(&agent_id)
            .await?
            .ok_or_else(|| RuntimeError::Protocol(format!("unknown agent id: {agent_id}")))?;
        Ok(AgentPromptIdentity {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
        })
    }

    pub(super) async fn model_tools(
        &self,
        include_agent_name_tool: bool,
        capabilities: ProviderToolCapabilities,
    ) -> Result<ModelTools, RuntimeError> {
        build_model_tools(
            &self.store,
            &self.capability_bindings,
            include_agent_name_tool,
            capabilities,
        )
        .await
        .map_err(|error| RuntimeError::Protocol(error.to_string()))
    }
}
