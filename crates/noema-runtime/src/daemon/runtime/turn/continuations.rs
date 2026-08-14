impl RuntimeActor {
    async fn run_foreground_continuations(
        &mut self,
        turn: &SuccessfulProviderTurn,
        continuation: ForegroundContinuationState,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<bool, RuntimeError> {
        let ForegroundContinuationState {
            mut next_output_index,
            mut task_handoff,
            mut all_local_tool_results,
            mut progress_tracker,
            mut continuation_context,
            mut continuation_tool_results,
            waiting_for_interaction,
            mut citation_sources,
        } = continuation;
        if waiting_for_interaction {
            self.update_conversation_agent_status(
                &turn.conversation_id,
                PersistedAgentStatus::ToolRunning,
                item_tx,
            )
            .await?;
            return Ok(true);
        }
        if all_local_tool_results
            .iter()
            .any(LocalToolResult::has_uncertain_outcome)
        {
            self.fail_uncertain_foreground_turn(turn, item_tx).await?;
            return Ok(true);
        }
        let mut waiting_for_interaction = false;
        let mut next_provider_round_index = 1;
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
                    turn,
                    &mut continuation_context,
                    next_output_index,
                    reason,
                    &mut citation_sources,
                    continuation_step_number,
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
                    model: "noema_local".to_string(),
                    response_phase: "continuation",
                    usage: None,
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
                                    turn,
                                    &mut continuation_context,
                                    next_output_index,
                                    "progress audit requested final answer",
                                    &mut citation_sources,
                                    continuation_step_number,
                                    item_tx,
                                    timing,
                                )
                                .await?;
                                continuation_tool_results.clear();
                                break;
                            }
                            ProgressAuditDecision::AskHuman | ProgressAuditDecision::Checkpoint => {
                                self.persist_progress_pause_message(
                                    turn,
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
                            turn,
                            &mut continuation_context,
                            next_output_index,
                            &message,
                            &mut citation_sources,
                            continuation_step_number,
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
            let continuation_result_count = continuation_tool_results.len();
            let provider = turn.provider_route.operations();
            let response_continuation = provider.response_continuation(turn.model.as_deref());
            let active_continuation_model_tools = if task_handoff {
                ModelTools::empty(crate::agent_execution::ExecutionRole::PrimaryConversation)
            } else {
                ModelTools::retained_catalog_with_policy(
                    &turn.initial_model_tools,
                    &turn.continuation_model_tools,
                )
            };
            let continuation_context_state = model_context_state(
                &continuation_agent_identity,
                turn.runtime_environment.clone(),
                &active_continuation_model_tools,
                !task_handoff,
            );
            let context_updates = sync_model_context(ModelContextSyncRequest {
                store: &self.store,
                conversation_id: &turn.conversation_id,
                turn_id: &turn.turn_id,
                provider_kind: &turn.provider_kind,
                model_profile: turn.model.as_deref(),
                state: &continuation_context_state,
            })
            .await?;
            for update in context_updates {
                continuation_context.append_developer_message(update.model_visible_content());
            }
            let task_delegation_available = active_continuation_model_tools
                .callable_tool_names()
                .iter()
                .any(|name| name.as_str() == TASK_DELEGATE_TOOL);
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                should_nudge_task_delegation(continuation_step_number, task_delegation_available),
            );
            let continuation_stream_suffix = if continuation_step == 0 {
                "continuation".to_string()
            } else {
                format!("continuation-{continuation_step}")
            };
            let continuation_stream_id =
                assistant_stream_id(&turn.turn_id, &continuation_stream_suffix);
            let continuation_output_base = next_output_index;
            let continuation_stream_seen = Arc::new(AtomicBool::new(false));
            let continuation_stream_seen_for_event = Arc::clone(&continuation_stream_seen);
            let mut continuation_assistant_delta_seen = false;
            let mut continuation_tool_start_events = Vec::new();
            let continuation_event_context = ConversationMemoryContext {
                turn_index: turn.turn_index,
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: turn.user_item_id.clone(),
                assistant_item_id: None,
            };
            let continuation_provider_started_at = std::time::Instant::now();
            let continuation_debug = RuntimeDebugSpan::begin(
                &self.store,
                RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
                RuntimeDebugSpanCategory::Provider,
                format!("Provider continuation {continuation_step}"),
                RuntimeDebugMetadata {
                    provider: Some(turn.provider_kind.clone()),
                    model: turn.model.clone(),
                    phase: Some("continuation".to_string()),
                    response_index: Some(continuation_step as u64),
                    ..RuntimeDebugMetadata::default()
                },
            )
            .await;
            let mut provider_timeline = ProviderDebugTimeline::begin();
            let mut on_continuation_event = |event| {
                provider_timeline.observe(&event);
                if !matches!(&event, GenerateStreamEvent::ProviderTiming { .. })
                    && !continuation_stream_seen_for_event.swap(true, Ordering::Relaxed)
                {
                    timing.mark(
                        "provider_continuation_first_stream_event",
                        continuation_provider_stream_event_fields(continuation_step, &event),
                    );
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
                match event {
                    GenerateStreamEvent::ToolCallStarted { .. } => {
                        continuation_tool_start_events.push(event);
                    }
                    GenerateStreamEvent::AssistantTextDelta { .. }
                    | GenerateStreamEvent::HostedWebSearchStarted { .. } => {
                        handle_provider_stream_event(
                            event,
                            item_tx,
                            &continuation_event_context,
                            &continuation_stream_id,
                            continuation_output_base,
                        );
                    }
                    GenerateStreamEvent::ProviderTiming { .. } => {}
                }
            };
            timing.mark(
                "provider_continuation_request_started",
                json!({
                    "continuation_step": continuation_step,
                    "tool_result_count": continuation_result_count,
                    "provider_tool_count": if task_handoff
                        || turn.continuation_model_tools.transport != ProviderToolTransport::Native
                    {
                        0
                    } else {
                        turn.continuation_model_tools.bindings.len()
                    },
                }),
            );
            let (continuation_tools, continuation_tool_choice) =
                if turn.tool_capabilities.allowed_tools
                    && turn.continuation_model_tools.transport == ProviderToolTransport::Native
                    && !active_continuation_model_tools.hosted_web_search()
                {
                    if task_handoff {
                        (Vec::new(), NoemaToolChoice::None)
                    } else {
                        (
                            turn.initial_model_tools.provider_tools(),
                            active_continuation_model_tools
                                .allowed_tool_choice(NoemaAllowedToolsMode::Auto),
                        )
                    }
                } else {
                    (
                        if task_handoff {
                            Vec::new()
                        } else {
                            active_continuation_model_tools.policy_filtered_provider_tools()
                        },
                        NoemaToolChoice::Auto,
                    )
                };
            let hosted_web_search =
                !task_handoff && active_continuation_model_tools.hosted_web_search();
            let parallel_tool_calls = !task_handoff
                && turn.continuation_model_tools.transport == ProviderToolTransport::Native
                && active_continuation_model_tools.has_callable_tools()
                && turn.tool_capabilities.parallel_tool_calls;
            admit_foreground_context(
                &mut continuation_context,
                turn,
                provider,
                &continuation_instructions,
                &continuation_tools,
                hosted_web_search,
            )
            .await?;
            let continuation_input = continuation_context.next_provider_input(
                turn.tool_capabilities.native_tool_results,
                response_continuation,
            );
            let chained = continuation_input.previous_response_id.is_some();
            let continuation_prompt_cache_breakpoints =
                prompt_cache_breakpoints_for(&continuation_input.input, turn.tool_capabilities);
            let continuation_request = GenerateRequest {
                conversation_id: Some(turn.conversation_id.clone()),
                model: turn.model.clone(),
                input: continuation_input.input,
                instructions: Some(continuation_instructions.clone()),
                options: GenerateOptions {
                    hosted_web_search,
                    prompt_cache_retention: prompt_cache_retention_for(turn.tool_capabilities),
                    prompt_cache_options: prompt_cache_options_for(turn.tool_capabilities),
                    prompt_cache_breakpoints: continuation_prompt_cache_breakpoints,
                    reasoning_effort: turn.reasoning_effort,
                    previous_response_id: continuation_input.previous_response_id,
                    store_response: response_continuation.store_response(),
                    ..GenerateOptions::default()
                },
                tools: continuation_tools.clone(),
                tool_transport: turn.continuation_model_tools.transport,
                tool_choice: continuation_tool_choice.clone(),
                parallel_tool_calls,
            };
            let mut continuation_result = provider
                .generate_streaming(continuation_request, &mut on_continuation_event)
                .await;
            if chained
                && continuation_result.is_err()
                && !continuation_stream_seen.load(Ordering::Relaxed)
            {
                timing.mark(
                    "provider_continuation_chain_fallback",
                    json!({"continuation_step": continuation_step}),
                );
                admit_foreground_context(
                    &mut continuation_context,
                    turn,
                    provider,
                    &continuation_instructions,
                    &continuation_tools,
                    hosted_web_search,
                )
                .await?;
                let fallback_input =
                    continuation_context.provider_input(turn.tool_capabilities.native_tool_results);
                let fallback_prompt_cache_breakpoints =
                    prompt_cache_breakpoints_for(&fallback_input, turn.tool_capabilities);
                continuation_result = provider
                    .generate_streaming(
                        GenerateRequest {
                            conversation_id: Some(turn.conversation_id.clone()),
                            model: turn.model.clone(),
                            input: fallback_input,
                            instructions: Some(continuation_instructions),
                            options: GenerateOptions {
                                hosted_web_search,
                                prompt_cache_retention: prompt_cache_retention_for(
                                    turn.tool_capabilities,
                                ),
                                prompt_cache_options: prompt_cache_options_for(
                                    turn.tool_capabilities,
                                ),
                                prompt_cache_breakpoints: fallback_prompt_cache_breakpoints,
                                reasoning_effort: turn.reasoning_effort,
                                store_response: response_continuation.store_response(),
                                ..GenerateOptions::default()
                            },
                            tools: continuation_tools,
                            tool_transport: turn.continuation_model_tools.transport,
                            tool_choice: continuation_tool_choice,
                            parallel_tool_calls,
                        },
                        &mut on_continuation_event,
                    )
                    .await;
            }
            let provider_children = provider_timeline.completed_spans();
            let continuation_response = match continuation_result {
                Ok(response) => response,
                Err(error) => {
                    continuation_debug
                        .finish_with_children(
                            RuntimeDebugSpanStatus::Failed,
                            None,
                            &provider_children,
                        )
                        .await;
                    return Err(RuntimeError::Provider(error));
                }
            };
            let continuation_usage = continuation_response.usage.as_ref();
            continuation_debug
                .finish_with_children(
                    RuntimeDebugSpanStatus::Completed,
                    Some(RuntimeDebugMetadata {
                        provider: Some(continuation_response.provider.clone()),
                        model: Some(continuation_response.model.clone()),
                        phase: Some("continuation".to_string()),
                        response_index: Some(continuation_step as u64),
                        input_tokens: continuation_usage.map(|value| value.input_tokens),
                        cached_input_tokens: continuation_usage
                            .and_then(|value| value.cached_input_tokens),
                        output_tokens: continuation_usage.map(|value| value.output_tokens),
                        total_tokens: continuation_usage.map(|value| value.total_tokens),
                        ..RuntimeDebugMetadata::default()
                    }),
                    &provider_children,
                )
                .await;
            timing.mark(
                "provider_continuation_response_completed",
                json!({
                    "continuation_step": continuation_step,
                    "duration_ms": continuation_provider_started_at.elapsed().as_millis(),
                    "response_count": continuation_response.responses.len(),
                    "tool_call_count": continuation_response.tool_calls.len(),
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
            let citation_response_index =
                continuation_response.responses.iter().rposition(|item| {
                    matches!(item, noema_providers::GenerateResponseItem::Text { .. })
                });
            citation_sources.observe(
                continuation_step_number,
                &continuation_response.hosted_web_searches,
            );
            next_provider_round_index = continuation_step_number + 1;
            let mut continuation_assistant_response = ProviderAssistantResponse::default();
            let continuation_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: continuation_response.provider.clone(),
                model: continuation_response.model.clone(),
                response_phase: "continuation",
                usage: continuation_response.usage.clone(),
                stream_id: Some(continuation_stream_id.clone()),
            };
            self.persist_provider_reasoning_items(
                &turn.conversation_id,
                &turn.turn_id,
                &continuation_response.provider,
                &continuation_response.reasoning_items,
            )
            .await?;
            let raw_continuation_batch_kind =
                if !task_handoff && !continuation_response.tool_calls.is_empty() {
                    ForegroundToolBatchKind::for_calls(&local_tool_calls(
                        &continuation_response.tool_calls,
                    ))
                } else {
                    ForegroundToolBatchKind::Standard
                };
            let continuation_tool_call_items = continuation_response.tool_calls.clone();
            continuation_context.append_response(&GenerateResponse {
                responses: continuation_response.responses.clone(),
                tool_calls: continuation_tool_call_items.clone(),
                reasoning_items: continuation_response.reasoning_items.clone(),
                hosted_web_searches: continuation_response.hosted_web_searches.clone(),
                citations: continuation_response.citations.clone(),
                provider: continuation_response.provider.clone(),
                model: continuation_response.model.clone(),
                response_id: continuation_response.response_id.clone(),
                usage: continuation_response.usage.clone(),
            });
            let continuation_response_count = continuation_response.responses.len();
            let continuation_tool_calls =
                if !task_handoff && !continuation_response.tool_calls.is_empty() {
                    local_tool_calls(&continuation_tool_call_items)
                } else {
                    Vec::new()
                };
            let continuation_batch_kind =
                if raw_continuation_batch_kind == ForegroundToolBatchKind::MixedDelegation {
                    ForegroundToolBatchKind::MixedDelegation
                } else {
                    ForegroundToolBatchKind::for_calls(&continuation_tool_calls)
                };
            if continuation_batch_kind != ForegroundToolBatchKind::MixedDelegation {
                for event in continuation_tool_start_events {
                    handle_provider_stream_event(
                        event,
                        item_tx,
                        &continuation_event_context,
                        &continuation_stream_id,
                        continuation_output_base + continuation_response_count,
                    );
                }
            }
            let continuation_phase_has_tools = !continuation_tool_calls.is_empty();
            let continuation_tool_description = single_tool_display_description(
                &continuation_response.responses,
                &continuation_response.reasoning_items,
                continuation_response.tool_calls.len(),
            );
            self.persist_hosted_web_searches(
                &continuation_action_turn,
                continuation_output_base,
                &continuation_response.hosted_web_searches,
                item_tx,
            )
            .await?;
            if !continuation_batch_kind.contains_delegation() {
                for (offset, response_item) in
                    continuation_response.responses.iter().cloned().enumerate()
                {
                    let response_item = match response_item {
                        noema_providers::GenerateResponseItem::Text { phase, text } => {
                            let existing = (citation_response_index == Some(offset))
                                .then_some(continuation_response.citations.as_slice())
                                .unwrap_or_default();
                            let normalized = self.normalize_provider_citation_text(
                                &citation_sources,
                                &text,
                                existing,
                                "conversation_turn",
                                &turn.turn_id,
                            );
                            continuation_assistant_response
                                .set_citations_for(offset, normalized.citations);
                            noema_providers::GenerateResponseItem::Text {
                                phase,
                                text: normalized.text,
                            }
                        }
                    };
                    self.persist_provider_response_item(
                        &continuation_action_turn,
                        ProviderResponsePosition {
                            response_index: offset,
                            output_index: Some(continuation_output_base + offset),
                        },
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
            }
            next_output_index += provider_output_span(
                continuation_response_count,
                continuation_tool_calls.len(),
                &continuation_response.hosted_web_searches,
            );

            let continuation_turn = SuccessfulProviderTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                user_input: turn.user_input.clone(),
                task_id: turn.task_id.clone(),
                task_run_id: turn.task_run_id.clone(),
                task_run_fence: turn.task_run_fence.clone(),
                task_terminal_contract: turn.task_terminal_contract.clone(),
                cwd: turn.cwd.clone(),
                provider_kind: turn.provider_kind.clone(),
                model: turn.model.clone(),
                reasoning_effort: turn.reasoning_effort,
                provider_route: Arc::clone(&turn.provider_route),
                initial_stream_id: continuation_stream_id.clone(),
                response: GenerateResponse {
                    responses: continuation_response.responses.clone(),
                    tool_calls: continuation_tool_call_items,
                    reasoning_items: continuation_response.reasoning_items.clone(),
                    hosted_web_searches: continuation_response.hosted_web_searches.clone(),
                    citations: continuation_response.citations.clone(),
                    provider: continuation_response.provider.clone(),
                    model: continuation_response.model.clone(),
                    response_id: continuation_response.response_id.clone(),
                    usage: continuation_response.usage.clone(),
                },
                agent_identity: continuation_agent_identity,
                runtime_environment: turn.runtime_environment.clone(),
                tool_capabilities: turn.tool_capabilities,
                initial_model_tools: active_continuation_model_tools,
                continuation_model_tools: turn.continuation_model_tools.clone(),
                initial_provider_input: turn.initial_provider_input.clone(),
            };
            let mut local_tool_results = Vec::new();
            let local_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: turn.provider_kind.clone(),
                model: "noema_local".to_string(),
                response_phase: "continuation",
                usage: None,
                stream_id: None,
            };
            for call in &continuation_tool_calls {
                let persisted_payload = continuation_turn
                    .initial_model_tools
                    .bindings
                    .resolve(&call.name)
                    .and_then(|binding| binding.persist_arguments(&call.payload));
                timing.mark(
                    "runtime_tool_call_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                let pending_result =
                    match Self::pending_presentation(&continuation_turn.conversation_id, call)? {
                        Some(presentation) => Some(
                            self.publish_pending_presentation(
                                &continuation_turn,
                                call,
                                continuation_output_base
                                    + continuation_response_count
                                    + call.output_index,
                                continuation_tool_description.as_deref(),
                                presentation,
                                item_tx,
                            )
                            .await?,
                        ),
                        None => None,
                    };
                let call_item_id = if pending_result.is_none() {
                    self.persist_provider_tool_call_started(
                        &local_action_turn,
                        continuation_output_base + continuation_response_count + call.output_index,
                        call,
                        persisted_payload,
                        continuation_tool_description.as_deref(),
                        item_tx,
                    )
                    .await?
                } else {
                    None
                };
                let tool_started_at = std::time::Instant::now();
                let tool_debug = RuntimeDebugSpan::begin(
                    &self.store,
                    RuntimeDebugScope::ConversationTurn(continuation_turn.turn_id.clone()),
                    RuntimeDebugSpanCategory::Tool,
                    call.name.clone(),
                    RuntimeDebugMetadata {
                        phase: Some("continuation".to_string()),
                        round_index: Some(continuation_step as u64),
                        tool_name: Some(call.name.clone()),
                        correlation_id: call.provider_call_id.clone().or(call.call_id.clone()),
                        ..RuntimeDebugMetadata::default()
                    },
                )
                .await;
                timing.mark(
                    "runtime_tool_execution_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                let result = if let Some(result) = pending_result {
                    result
                } else if continuation_batch_kind == ForegroundToolBatchKind::MixedDelegation {
                    rejected_mixed_delegation_result(
                        call,
                        &continuation_turn.initial_model_tools.bindings,
                    )
                } else {
                    self.execute_local_tool(
                        &continuation_turn,
                        &continuation_turn.agent_identity,
                        call,
                    )
                    .await
                };
                tool_debug
                    .finish(
                        if result.success {
                            RuntimeDebugSpanStatus::Completed
                        } else {
                            RuntimeDebugSpanStatus::Failed
                        },
                        None,
                    )
                    .await;
                timing.mark(
                    "runtime_tool_execution_completed",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": result.name,
                        "duration_ms": tool_started_at.elapsed().as_millis(),
                        "success": result.success,
                        "requires_provider_continuation": result.requires_provider_continuation,
                    }),
                );
                if result.is_waiting_for_interaction() {
                    waiting_for_interaction = true;
                    break;
                }
                self.persist_provider_action_item(
                    &local_action_turn,
                    next_output_index,
                    local_tool_result_action_item(&result),
                    call_item_id.as_deref(),
                    item_tx,
                )
                .await?;
                if let Some(item_id) = result
                    .payload
                    .get("projection_item_id")
                    .and_then(serde_json::Value::as_str)
                {
                    self.emit_projection_item(&continuation_turn.conversation_id, item_id, item_tx)
                        .await?;
                }
                if let Some(item) = local_tool_artifact_reference_item(&result) {
                    let context = ConversationMemoryContext {
                        turn_index: continuation_turn.turn_index,
                        conversation_id: continuation_turn.conversation_id.clone(),
                        turn_id: continuation_turn.turn_id.clone(),
                        user_item_id: continuation_turn.user_item_id.clone(),
                        assistant_item_id: None,
                    };
                    self.persist_and_send_turn_item(&context, item, item_tx)
                        .await?;
                }
                timing.mark(
                    "runtime_tool_result_persisted",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": result.name,
                        "success": result.success,
                    }),
                );
                next_output_index += 1;
                let blocked = result.is_blocked();
                local_tool_results.push(result);
                if blocked {
                    break;
                }
            }
            task_handoff = continuation_batch_kind.is_terminal_handoff();
            continuation_tool_results = if task_handoff {
                Vec::new()
            } else {
                local_tool_results
                    .iter()
                    .filter(|result| result.requires_provider_continuation)
                    .cloned()
                    .collect::<Vec<_>>()
            };
            progress_tracker.observe_results(&local_tool_results);
            continuation_context.append_results(&local_tool_results);
            continuation_context.finish_round();
            all_local_tool_results.extend(local_tool_results.clone());
            if waiting_for_interaction {
                break;
            }
        }
        if waiting_for_interaction {
            self.update_conversation_agent_status(
                &turn.conversation_id,
                PersistedAgentStatus::ToolRunning,
                item_tx,
            )
            .await?;
            return Ok(true);
        }
        if all_local_tool_results
            .iter()
            .any(LocalToolResult::has_uncertain_outcome)
        {
            self.fail_uncertain_foreground_turn(turn, item_tx).await?;
            return Ok(true);
        }
        self.finalize_after_continuation_ceiling(
            turn,
            &mut continuation_context,
            all_local_tool_results.len(),
            next_output_index,
            task_handoff,
            !continuation_tool_results.is_empty(),
            &mut citation_sources,
            next_provider_round_index,
            item_tx,
            timing,
        )
        .await?;
        Ok(false)
    }

    async fn fail_uncertain_foreground_turn(
        &mut self,
        turn: &SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let context = ConversationMemoryContext {
            turn_index: turn.turn_index,
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            user_item_id: turn.user_item_id.clone(),
            assistant_item_id: None,
        };
        self.record_turn_failure_notice(
            &context,
            "Noema couldn’t confirm whether that action completed. Check the connected service before trying again to avoid a duplicate."
                .to_string(),
            false,
            item_tx,
        )
        .await?;
        self.conversations.remove(&turn.conversation_id);
        Ok(())
    }
}
