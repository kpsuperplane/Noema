impl RuntimeActor {
    async fn persist_successful_provider_turn(
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
        self.run_foreground_continuations(&turn, continuation, item_tx, timing)
            .await?;
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
            &turn.response.reasoning_items,
        )
        .await?;
        let mut initial_assistant_response = ProviderAssistantResponse::default();
        let initial_tool_calls = local_tool_calls(&turn.response.tool_calls);
        let initial_batch_kind = ForegroundToolBatchKind::for_calls(&initial_tool_calls);
        let initial_phase_has_tools = !initial_tool_calls.is_empty();
        if !initial_batch_kind.contains_delegation() {
            for (index, response_item) in turn.response.responses.iter().cloned().enumerate() {
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

        let mut next_output_index = initial_response_count + initial_tool_calls.len();
        let local_action_turn = ProviderActionTurn {
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
            let result = if initial_batch_kind == ForegroundToolBatchKind::MixedDelegation {
                rejected_mixed_delegation_result(call, &turn.initial_model_tools.bindings)
            } else {
                self.execute_local_tool(turn, &turn.agent_identity, call)
                    .await
            };
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
            self.persist_provider_action_item(
                &local_action_turn,
                next_output_index,
                local_tool_result_action_item(&result),
                item_tx,
            )
            .await?;
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
            if let Some(item) = local_tool_task_reference_item(&result) {
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
            local_tool_results.push(result);
        }
        if initial_batch_kind.contains_delegation() {
            self.persist_task_delegation_receipt(
                turn,
                &turn.initial_stream_id,
                initial_response_count,
                &local_tool_results,
                item_tx,
            )
            .await?;
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
        })
    }
}
