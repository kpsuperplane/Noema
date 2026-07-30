impl RuntimeActor {
    async fn turn_with_user_input(
        &mut self,
        conversation_id: String,
        user_input: UserTurnInput,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        let input = user_input.model_input();
        let memory_root_context = self.native_memory_context();
        let pre_turn_started_at = std::time::Instant::now();
        let conversation = self
            .hydrate_active_conversation(&conversation_id, None)
            .await?;
        let provider_route = Arc::new(self.resolve_primary_provider().await?);
        let provider_selection = provider_route.selection().clone();
        let provider_kind = provider_selection.provider_kind.as_str();
        let model_profile = provider_selection.model_profile.as_deref();
        let reasoning_effort = provider_selection.reasoning_effort;
        let turn_index = conversation.next_turn_index;
        let turn = if let Some((intervention_id, trigger_item_id)) = user_input.human_intervention() {
            let (turn, inserted) = self
                .store
                .create_conversation_turn_with_id_if_absent(
                    format!("turn:human_intervention:{intervention_id}"),
                    NewConversationTurn {
                        conversation_id: conversation_id.clone(),
                        trigger_item_id: Some(trigger_item_id.to_string()),
                        metadata: json!({
                            "turn_index": turn_index,
                            "source": "human_intervention_continuation",
                            "intervention_id": intervention_id,
                        }),
                    },
                )
                .await?;
            if !inserted {
                return Ok(());
            }
            turn
        } else {
            self.store
                .create_conversation_turn(NewConversationTurn {
                    conversation_id: conversation_id.clone(),
                    trigger_item_id: None,
                    metadata: json!({ "turn_index": turn_index }),
                })
                .await?
        };
        let timing = TurnTiming::new(
            conversation_id.clone(),
            turn.turn_id.clone(),
            turn_index,
            client_message_id.clone(),
        );
        timing.mark(
            "runtime_turn_started",
            json!({
                "hydrate_and_create_turn_ms": pre_turn_started_at.elapsed().as_millis(),
                "input_chars": user_input.input_chars(),
                "provider_kind": provider_kind,
                "model": model_profile,
            }),
        );
        let provider = provider_route.operations();
        let tool_capabilities = provider.tool_capabilities(model_profile);
        let response_continuation = provider.response_continuation(model_profile);
        let agent_identity = self
            .agent_identity_for_conversation()
            .await?;
        let tools_started_at = std::time::Instant::now();
        let tools_debug = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            RuntimeDebugSpanCategory::Runtime,
            "Resolve tool catalog",
            RuntimeDebugMetadata::default(),
        )
        .await;
        let model_tools = self.model_tools(true, tool_capabilities).await?;
        let continuation_model_tools = self.model_tools(true, tool_capabilities).await?;
        let initial_request_tools = model_tools.provider_tools();
        tools_debug
            .finish(RuntimeDebugSpanStatus::Completed, None)
            .await;
        timing.mark(
            "runtime_model_tools_ready",
            json!({
                "duration_ms": tools_started_at.elapsed().as_millis(),
                "tool_catalog_count": model_tools.bindings.len(),
                "callable_tool_count": model_tools.callable_tool_names().len(),
                "continuation_callable_tool_count": continuation_model_tools.callable_tool_names().len(),
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
        let runtime_environment = current_runtime_environment(conversation.cwd.as_deref());
        let model_context_state = model_context_state(
            &agent_identity,
            runtime_environment.clone(),
            &model_tools,
            true,
        );
        let model_context_updates = sync_model_context(ModelContextSyncRequest {
            store: &self.store,
            conversation_id: &conversation_id,
            turn_id: &turn.turn_id,
            provider_kind,
            model_profile,
            state: &model_context_state,
        })
        .await?;
        timing.mark(
            "runtime_model_context_synced",
            json!({ "update_count": model_context_updates.len() }),
        );
        let prompt_started_at = std::time::Instant::now();
        let context_debug = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            RuntimeDebugSpanCategory::Runtime,
            "Plan model context",
            RuntimeDebugMetadata::default(),
        )
        .await;
        let mut planned_context =
            super::prompt_context::plan_prompt_context(super::prompt_context::PromptPlanRequest {
                store: &self.store,
                provider,
                conversation_id: &conversation_id,
                provider_kind,
                model_profile,
                current_input: &input,
                memory_root_context: memory_root_context.as_deref(),
            })
            .await?;
        let reconciled_model_context_updates = self
            .reconcile_model_context_plan(
                provider,
                &conversation_id,
                &turn.turn_id,
                provider_kind,
                model_profile,
                &model_context_state,
                &input,
                &mut planned_context,
            )
            .await?;
        let mut initial_admission = admit_request(
            provider,
            RequestContext {
                model: model_profile,
                instructions: Some(&planned_context.instructions),
                input: &planned_context.input,
                tools: &initial_request_tools,
                hosted_web_search: model_tools.hosted_web_search(),
                output_reserve_tokens: planned_context.budget.output_reserve_tokens(),
                has_compactable_history: planned_context.context.active_summary.is_some()
                    || !planned_context.context.transcript_items.is_empty(),
            },
        )
        .await;
        context_debug
            .finish(RuntimeDebugSpanStatus::Completed, None)
            .await;
        timing.mark(
            "runtime_prompt_context_planned",
            json!({
                "duration_ms": prompt_started_at.elapsed().as_millis(),
                "fits": !matches!(initial_admission, ContextAdmission::HardOverflowWithOnlyActiveContext { .. }),
                "estimated_prompt_tokens": initial_admission.estimated_input_tokens(),
                "budget_input_tokens": planned_context.budget.available_input_tokens(),
                "budget_output_reserve_tokens": planned_context.budget.output_reserve_tokens(),
                "reconciled_model_context_updates": reconciled_model_context_updates,
            }),
        );
        let user_item_id = if let Some((_, trigger_item_id)) = user_input.human_intervention() {
            trigger_item_id.to_string()
        } else {
            let user_metadata = json!({
                "turn_index": turn_index,
                "client_message_id": client_message_id,
            });
            let (user_kind, parent_item_id, user_content_text, user_payload, transcript_item) =
                match &user_input {
                UserTurnInput::Text(text) => (
                    ConversationItemKind::UserText,
                    None,
                    Some(text.clone()),
                    json!({}),
                    TurnTranscriptItem::UserText { text: text.clone() },
                ),
                UserTurnInput::HumanInterventionContinuation { .. } => unreachable!(),
            };
            let user_item = self
                .store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id,
                    kind: user_kind,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::human("human:local")
                        .expect("static local human actor id must be valid"),
                    content_text: user_content_text,
                    payload_json: user_payload,
                    metadata: user_metadata.clone(),
                })
                .await?;
            let user_item_id = user_item.item_id.clone();
            send_conversation_item(&item_tx, user_item, user_metadata, transcript_item);
            if matches!(user_input, UserTurnInput::Text(_)) {
                self.runtime_events
                    .publish_memory(crate::daemon::MemoryRuntimeEvent::Changed);
            }
            user_item_id
        };
        timing.mark("runtime_user_item_persisted", json!({}));
        let mut compacted_context = false;
        while initial_admission.requires_compaction() {
            let estimated_before_compaction = initial_admission.estimated_input_tokens();
            let shortening_active_summary = planned_context.context.transcript_items.is_empty();
            let compaction_started_at = std::time::Instant::now();
            timing.mark("runtime_foreground_compaction_started", json!({}));
            let compaction_request = super::context_compaction::CompactionRequest {
                store: &self.store,
                provider,
                conversation_id: &conversation_id,
                provider_kind,
                model_profile,
                reasoning_effort,
                budget: planned_context.budget,
                mode: super::context_compaction::CompactionMode::Foreground,
            };
            let compaction_result = if planned_context.context.transcript_items.is_empty() {
                super::context_compaction::compact_active_summary_smaller(compaction_request).await
            } else {
                super::context_compaction::compact_context_with_retry(compaction_request).await
            };
            if let Err(error) = compaction_result {
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
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
            compacted_context = true;
            self.schedule_background_native_memory_update(conversation_id.clone());
            timing.mark(
                "runtime_foreground_compaction_finished",
                json!({ "duration_ms": compaction_started_at.elapsed().as_millis() }),
            );
            sync_model_context(ModelContextSyncRequest {
                store: &self.store,
                conversation_id: &conversation_id,
                turn_id: &turn.turn_id,
                provider_kind,
                model_profile,
                state: &model_context_state,
            })
            .await?;
            let prompt_replan_started_at = std::time::Instant::now();
            planned_context =
                super::prompt_context::plan_prompt_context(super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider,
                    conversation_id: &conversation_id,
                    provider_kind,
                    model_profile,
                    current_input: &input,
                    memory_root_context: memory_root_context.as_deref(),
                })
                .await?;
            self.reconcile_model_context_plan(
                provider,
                &conversation_id,
                &turn.turn_id,
                provider_kind,
                model_profile,
                &model_context_state,
                &input,
                &mut planned_context,
            )
            .await?;
            initial_admission = admit_request(
                provider,
                RequestContext {
                    model: model_profile,
                    instructions: Some(&planned_context.instructions),
                    input: &planned_context.input,
                    tools: &initial_request_tools,
                    hosted_web_search: model_tools.hosted_web_search(),
                    output_reserve_tokens: planned_context.budget.output_reserve_tokens(),
                    has_compactable_history: planned_context.context.active_summary.is_some()
                        || !planned_context.context.transcript_items.is_empty(),
                },
            )
            .await;
            if shortening_active_summary
                && initial_admission.estimated_input_tokens() >= estimated_before_compaction
                && initial_admission.requires_compaction()
            {
                initial_admission = admit_request(
                    provider,
                    RequestContext {
                        model: model_profile,
                        instructions: Some(&planned_context.instructions),
                        input: &planned_context.input,
                        tools: &initial_request_tools,
                        hosted_web_search: model_tools.hosted_web_search(),
                        output_reserve_tokens: planned_context.budget.output_reserve_tokens(),
                        has_compactable_history: false,
                    },
                )
                .await;
            }
            timing.mark(
                "runtime_prompt_context_replanned_after_compaction",
                json!({
                    "duration_ms": prompt_replan_started_at.elapsed().as_millis(),
                    "estimated_prompt_tokens": initial_admission.estimated_input_tokens(),
                }),
            );
        }
        if compacted_context {
            let notice = super::context_compaction::persist_context_compaction_notice(
                &self.store,
                &conversation_id,
                Some(&turn.turn_id),
                Some(&user_item_id),
            )
            .await?;
            let _ = item_tx.send(notice);
        }
        if matches!(
            initial_admission,
            ContextAdmission::HardOverflowWithOnlyActiveContext { .. }
        ) {
            let error = hard_overflow_error(initial_admission);
            let error_context = ConversationMemoryContext {
                turn_index,
                conversation_id: conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: user_item_id.clone(),
                assistant_item_id: None,
            };
            self.record_turn_failure_notice(&error_context, error.to_string(), true, &item_tx)
                .await?;
            self.conversations.remove(&conversation_id);
            return Err(error.into());
        }
        let initial_stream_id = assistant_stream_id(&turn.turn_id, "initial");
        let mut initial_stream_seen = false;
        let mut initial_assistant_delta_seen = false;
        let mut initial_tool_start_events = Vec::new();
        let initial_event_context = ConversationMemoryContext {
            turn_index,
            conversation_id: conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            user_item_id: user_item_id.clone(),
            assistant_item_id: None,
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
            match event {
                GenerateStreamEvent::ToolCallStarted { .. } => {
                    initial_tool_start_events.push(event);
                }
                GenerateStreamEvent::AssistantTextDelta { .. }
                | GenerateStreamEvent::HostedWebSearchStarted { .. } => {
                    handle_provider_stream_event(
                        event,
                        &item_tx,
                        &initial_event_context,
                        &initial_stream_id,
                        0,
                    );
                }
            }
        };

        timing.mark(
            "provider_initial_request_started",
            json!({
                "provider_tool_count": model_tools.provider_tools().len(),
                "parallel_tool_calls": model_tools.transport == ProviderToolTransport::Native
                    && model_tools.has_callable_tools()
                    && tool_capabilities.parallel_tool_calls,
            }),
        );
        let initial_provider_started_at = std::time::Instant::now();
        let initial_provider_debug = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            RuntimeDebugSpanCategory::Provider,
            "Initial provider request",
            RuntimeDebugMetadata {
                provider: Some(provider_kind.to_string()),
                model: model_profile.map(str::to_string),
                phase: Some("initial".to_string()),
                response_index: Some(0),
                ..RuntimeDebugMetadata::default()
            },
        )
        .await;
        let initial_provider_input = planned_context.input.clone();
        let initial_prompt_cache_breakpoints =
            prompt_cache_breakpoints_for(&planned_context.input, tool_capabilities);
        let initial_provider_tools = initial_request_tools;
        let (initial_tools, initial_tool_choice) = if tool_capabilities.allowed_tools
            && model_tools.transport == ProviderToolTransport::Native
            && !model_tools.hosted_web_search()
        {
            (
                initial_provider_tools,
                model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto),
            )
        } else {
            (initial_provider_tools, NoemaToolChoice::Auto)
        };
        match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.clone()),
                    model: provider_selection.model_profile.clone(),
                    input: planned_context.input,
                    instructions: Some(planned_context.instructions),
                    options: GenerateOptions {
                        max_output_tokens: planned_context.budget.output_reserve_tokens(),
                        reasoning_effort,
                        hosted_web_search: model_tools.hosted_web_search(),
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        prompt_cache_options: prompt_cache_options_for(tool_capabilities),
                        prompt_cache_breakpoints: initial_prompt_cache_breakpoints,
                        store_response: response_continuation.store_response(),
                        ..GenerateOptions::default()
                    },
                    tools: initial_tools,
                    tool_transport: model_tools.transport,
                    tool_choice: initial_tool_choice,
                    parallel_tool_calls: model_tools.transport == ProviderToolTransport::Native
                        && model_tools.has_callable_tools()
                        && tool_capabilities.parallel_tool_calls,
                },
                &mut on_initial_event,
            )
            .await
        {
            Ok(response) => {
                let usage = response.usage.as_ref();
                initial_provider_debug
                    .finish(
                        RuntimeDebugSpanStatus::Completed,
                        Some(RuntimeDebugMetadata {
                            provider: Some(response.provider.clone()),
                            model: Some(response.model.clone()),
                            phase: Some("initial".to_string()),
                            response_index: Some(0),
                            input_tokens: usage.map(|value| value.input_tokens),
                            cached_input_tokens: usage.and_then(|value| value.cached_input_tokens),
                            output_tokens: usage.map(|value| value.output_tokens),
                            total_tokens: usage.map(|value| value.total_tokens),
                            ..RuntimeDebugMetadata::default()
                        }),
                    )
                    .await;
                let initial_batch_kind =
                    ForegroundToolBatchKind::for_calls(&local_tool_calls(&response.tool_calls));
                if initial_batch_kind != ForegroundToolBatchKind::MixedDelegation {
                    for event in initial_tool_start_events {
                        handle_provider_stream_event(
                            event,
                            &item_tx,
                            &initial_event_context,
                            &initial_stream_id,
                            0,
                        );
                    }
                }
                timing.mark(
                    "provider_initial_response_completed",
                    json!({
                        "duration_ms": initial_provider_started_at.elapsed().as_millis(),
                        "response_count": response.responses.len(),
                        "tool_call_count": response.tool_calls.len(),
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
                            task_id: None,
                            task_run_id: None,
                            task_run_fence: None,
                            task_terminal_contract: None,
                            cwd: conversation.cwd.clone(),
                            provider_kind: provider_selection.provider_kind.clone(),
                            model: provider_selection.model_profile.clone(),
                            reasoning_effort,
                            provider_route: Arc::clone(&provider_route),
                            initial_stream_id: initial_stream_id.clone(),
                            response,
                            agent_identity,
                            runtime_environment,
                            tool_capabilities,
                            initial_model_tools: model_tools,
                            continuation_model_tools,
                            initial_provider_input,
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
                    };
                    self.record_turn_failure(&failure_context, error.to_string(), &item_tx)
                        .await?;
                    self.conversations.remove(&conversation_id);
                    return Err(error);
                }
                self.schedule_background_context_compaction(BackgroundContextCompactionSchedule {
                    conversation_id: conversation_id.clone(),
                    provider_kind: provider_selection.provider_kind.clone(),
                    model_profile: provider_selection.model_profile.clone(),
                    reasoning_effort,
                    provider_route,
                    next_turn_index: turn_index.saturating_add(1),
                });

                timing.mark("runtime_turn_ok", json!({}));
                Ok(())
            }
            Err(error) => {
                initial_provider_debug
                    .finish(RuntimeDebugSpanStatus::Failed, None)
                    .await;
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
                    | ProviderError::TransportFailure { .. }
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
                };
                if let Some((provider, output)) = partial_output {
                    let action_turn = ProviderActionTurn {
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id,
                        turn_index,
                        user_item_id,
                        provider,
                        model: "unknown".to_string(),
                        response_phase: "continuation",
                        usage: None,
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
}
