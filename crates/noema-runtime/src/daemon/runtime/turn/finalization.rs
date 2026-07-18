impl RuntimeActor {
    async fn finalize_after_progress_stop(
        &mut self,
        turn: &SuccessfulProviderTurn,
        results: &[LocalToolResult],
        index: usize,
        reason: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), RuntimeError> {
        let result_refs = results.iter().collect::<Vec<_>>();
        let provider = turn.provider_route.operations();
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
                        reasoning_effort: turn.reasoning_effort,
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
            }),
        );

        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: response.provider.clone(),
            model: response.model.clone(),
            response_phase: "continuation",
            usage: response.usage.clone(),
            stream_id: None,
        };
        self.persist_provider_reasoning_items(
            &turn.conversation_id,
            &turn.turn_id,
            &response.reasoning_items,
        )
        .await?;
        let mut assistant_response = ProviderAssistantResponse::default();
        for (offset, response_item) in response.responses.into_iter().enumerate() {
            self.persist_provider_response_item(
                &action_turn,
                ProviderResponsePosition {
                    response_index: offset,
                    output_index: Some(index + offset),
                },
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
    ) -> Result<(), RuntimeError> {
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
                author: ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
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

    async fn persist_task_delegation_receipt(
        &mut self,
        turn: &SuccessfulProviderTurn,
        response_stream_id: &str,
        response_count: usize,
        results: &[LocalToolResult],
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let delegation_results = results
            .iter()
            .filter(|result| is_task_delegate_tool(&result.name))
            .cloned()
            .collect::<Vec<_>>();
        if delegation_results.is_empty() {
            return Ok(());
        }
        let text = task_delegation_receipt(&delegation_results);
        let reconciled_stream_ids = (0..response_count)
            .map(|response_index| assistant_response_stream_id(response_stream_id, response_index))
            .collect::<Vec<_>>();
        let metadata = json!({
            "turn_index": turn.turn_index,
            "response_index": 0,
            "stream_id": reconciled_stream_ids.first(),
            "reconciled_stream_ids": reconciled_stream_ids,
            "phase": "final_answer",
            "source": "task_delegation_receipt",
            "delegation_success_count": delegation_results.iter().filter(|result| result.success).count(),
            "delegation_failure_count": delegation_results.iter().filter(|result| !result.success).count(),
        });
        let assistant_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: turn.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: Some(turn.user_item_id.clone()),
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: Some(text.clone()),
                payload_json: json!({}),
                metadata: metadata.clone(),
            })
            .await?;
        send_conversation_item(
            item_tx,
            assistant_item,
            metadata,
            TurnTranscriptItem::AssistantText { text },
        );
        Ok(())
    }

    async fn agent_identity_for_conversation(&self) -> Result<AgentPromptIdentity, RuntimeError> {
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

    async fn model_tools(
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

    #[allow(clippy::too_many_arguments)]
    async fn reconcile_model_context_plan(
        &self,
        provider: &dyn noema_providers::ProviderOperations,
        conversation_id: &str,
        turn_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
        state: &ModelContextState,
        current_input: &str,
        planned_context: &mut super::prompt_context::PlannedPromptContext,
    ) -> Result<usize, RuntimeError> {
        let mut appended_update_count = 0usize;
        loop {
            let updates = sync_model_context(ModelContextSyncRequest {
                store: &self.store,
                conversation_id,
                turn_id,
                provider_kind,
                model_profile,
                state,
            })
            .await?;
            if updates.is_empty() {
                return Ok(appended_update_count);
            }
            appended_update_count = appended_update_count.saturating_add(updates.len());
            *planned_context = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider,
                    conversation_id,
                    provider_kind,
                    model_profile,
                    current_input,
                },
            )
            .await?;
        }
    }

    fn schedule_background_context_compaction(
        &self,
        schedule: BackgroundContextCompactionSchedule,
    ) {
        let store = self.store.clone();
        self.tasks.spawn(async move {
            let BackgroundContextCompactionSchedule {
                conversation_id,
                provider_kind,
                model_profile,
                reasoning_effort,
                provider_route,
                next_turn_index,
            } = schedule;
            let provider = provider_route.operations();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            match store.next_conversation_turn_index(&conversation_id).await {
                Ok(current_next_turn_index) if current_next_turn_index == next_turn_index => {}
                Ok(_) | Err(_) => return,
            }
            let plan = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &store,
                    provider,
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
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
                    provider,
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
                    reasoning_effort,
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
