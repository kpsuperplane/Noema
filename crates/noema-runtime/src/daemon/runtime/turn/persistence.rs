impl RuntimeActor {
    pub(super) async fn persist_successful_provider_turn(
        &mut self,
        turn: SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), RuntimeError> {
        let persist_started_at = std::time::Instant::now();
        timing.mark("runtime_persist_successful_turn_started", json!({}));
        let continuation = self
            .prepare_foreground_continuations(&turn, item_tx, timing)
            .await?;
        if self
            .run_foreground_continuations(&turn, continuation, item_tx, timing)
            .await?
        {
            return Ok(());
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

    async fn prepare_foreground_continuations(
        &mut self,
        turn: &SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<ForegroundContinuationState, RuntimeError> {
        let initial_response_count = turn.response.responses.len();
        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: turn.response.provider.clone(),
            model: turn.response.model.clone(),
            response_phase: "initial",
            usage: turn.response.usage.clone(),
            stream_id: Some(turn.initial_stream_id.clone()),
        };
        self.persist_provider_reasoning_items(
            &turn.conversation_id,
            &turn.turn_id,
            &turn.response.provider,
            &turn.response.reasoning_items,
        )
        .await?;
        let citation_response_index =
            turn.response.responses.iter().rposition(|item| {
                matches!(item, noema_providers::GenerateResponseItem::Text { .. })
            });
        let mut citation_sources = CitationSourceRegistry::default();
        citation_sources.observe(0, &turn.response.hosted_web_searches);
        let mut initial_assistant_response = ProviderAssistantResponse::default();
        let initial_tool_calls = local_tool_calls(&turn.response.tool_calls);
        let initial_tool_description = single_tool_display_description(
            &turn.response.responses,
            &turn.response.reasoning_items,
            initial_tool_calls.len(),
        );
        let initial_batch_kind = ForegroundToolBatchKind::for_calls(&initial_tool_calls);
        let initial_phase_has_tools = !initial_tool_calls.is_empty();
        self.persist_hosted_web_searches(
            &action_turn,
            0,
            &turn.response.hosted_web_searches,
            item_tx,
        )
        .await?;
        if !initial_batch_kind.contains_delegation() {
            for (index, response_item) in turn.response.responses.iter().cloned().enumerate() {
                let response_item = match response_item {
                    noema_providers::GenerateResponseItem::Text { phase, text } => {
                        let existing = (citation_response_index == Some(index))
                            .then_some(turn.response.citations.as_slice())
                            .unwrap_or_default();
                        let normalized = self.normalize_provider_citation_text(
                            &citation_sources,
                            &text,
                            existing,
                            "conversation_turn",
                            &turn.turn_id,
                        );
                        initial_assistant_response
                            .set_citations_for(index, normalized.citations);
                        noema_providers::GenerateResponseItem::Text {
                            phase,
                            text: normalized.text,
                        }
                    }
                };
                self.persist_provider_response_item(
                    &action_turn,
                    ProviderResponsePosition {
                        response_index: index,
                        output_index: Some(index),
                    },
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
        }

        let mut next_output_index = provider_output_span(
            initial_response_count,
            initial_tool_calls.len(),
            &turn.response.hosted_web_searches,
        );
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
        let mut local_tool_results = Vec::new();
        let mut waiting_for_interaction = false;
        for call in &initial_tool_calls {
            let persisted_payload = turn
                .initial_model_tools
                .bindings
                .resolve(&call.name)
                .and_then(|binding| binding.persist_arguments(&call.payload));
            timing.mark(
                "runtime_tool_call_started",
                json!({
                    "phase": "initial",
                    "tool_name": call.name,
                    "output_index": call.output_index,
                }),
            );
            let pending_result = if initial_batch_kind != ForegroundToolBatchKind::MixedDelegation {
                match Self::pending_presentation(&turn.conversation_id, call)? {
                    Some(presentation) => Some(
                        self.publish_pending_presentation(
                            turn,
                            call,
                            initial_response_count + call.output_index,
                            initial_tool_description.as_deref(),
                            presentation,
                            item_tx,
                        )
                        .await?,
                    ),
                    None => None,
                }
            } else {
                None
            };
            let call_item_id = if pending_result.is_none() {
                self.persist_provider_tool_call_started(
                    &local_action_turn,
                    initial_response_count + call.output_index,
                    call,
                    persisted_payload,
                    initial_tool_description.as_deref(),
                    item_tx,
                )
                .await?
            } else {
                None
            };
            let tool_started_at = std::time::Instant::now();
            let tool_debug = RuntimeDebugSpan::begin(
                &self.store,
                RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
                RuntimeDebugSpanCategory::Tool,
                call.name.clone(),
                RuntimeDebugMetadata {
                    phase: Some("initial".to_string()),
                    round_index: Some(0),
                    tool_name: Some(call.name.clone()),
                    correlation_id: call.provider_call_id.clone().or(call.call_id.clone()),
                    ..RuntimeDebugMetadata::default()
                },
            )
            .await;
            timing.mark(
                "runtime_tool_execution_started",
                json!({
                    "phase": "initial",
                    "tool_name": call.name,
                    "output_index": call.output_index,
                }),
            );
            let result = if let Some(result) = pending_result {
                result
            } else if initial_batch_kind == ForegroundToolBatchKind::MixedDelegation {
                rejected_mixed_delegation_result(call, &turn.initial_model_tools.bindings)
            } else {
                self.execute_local_tool(turn, &turn.agent_identity, call)
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
                    "phase": "initial",
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
                self.emit_projection_item(&turn.conversation_id, item_id, item_tx)
                    .await?;
            }
            if let Some(item) = local_tool_artifact_reference_item(&result) {
                let context = ConversationMemoryContext {
                    turn_index: turn.turn_index,
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    assistant_item_id: None,
                };
                self.persist_and_send_turn_item(&context, item, item_tx)
                    .await?;
            }
            timing.mark(
                "runtime_tool_result_persisted",
                json!({
                    "phase": "initial",
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
        let task_handoff = initial_batch_kind.is_terminal_handoff();
        let all_local_tool_results = local_tool_results.clone();
        let mut progress_tracker = ContinuationProgressTracker::new(&turn.user_input);
        progress_tracker.observe_results(&local_tool_results);
        let mut continuation_context =
            ContinuationContext::from_provider_input(turn.initial_provider_input.clone());
        continuation_context.append_response(&turn.response);
        continuation_context.append_results(&local_tool_results);
        continuation_context.finish_round();

        let continuation_tool_results = if task_handoff {
            Vec::new()
        } else {
            local_tool_results
                .iter()
                .filter(|result| result.requires_provider_continuation)
                .cloned()
                .collect::<Vec<_>>()
        };
        Ok(ForegroundContinuationState {
            next_output_index,
            task_handoff,
            all_local_tool_results,
            progress_tracker,
            continuation_context,
            continuation_tool_results,
            waiting_for_interaction,
            citation_sources,
        })
    }
}
