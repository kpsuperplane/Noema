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
            let memory_root_context = self.native_memory_context();
            *planned_context = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider,
                    conversation_id,
                    provider_kind,
                    model_profile,
                    current_input,
                    memory_root_context: memory_root_context.as_deref(),
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
        let actor = self.clone_for_background();
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
                    memory_root_context: None,
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
            match result {
                Ok(_) => { actor.schedule_background_native_memory_update(conversation_id.clone()); }
                Err(error) => {
                    let _ = super::context_compaction::record_failed_background_compaction(
                        &store,
                        &conversation_id,
                        &provider_kind,
                        model_profile.as_deref(),
                        &error,
                    )
                    .await;
                }
            }
        });
    }

    pub(super) fn schedule_background_native_memory_update(&self, conversation_id: String) -> bool {
        let Some(native_memory) = self.native_memory.clone() else { return false };
        let active = Arc::clone(&self.native_memory_update_active);
        if active.swap(true, Ordering::AcqRel) {
            return false;
        }
        let actor = self.clone_for_background();
        self.tasks.spawn(async move {
            let result = actor.run_native_memory_update(&native_memory, &conversation_id).await;
            active.store(false, Ordering::Release);
            if let Ok(mut last_error) = actor.native_memory_update_error.write() {
                *last_error = result.as_ref().err().cloned();
            }
            if let Err(error) = result {
                actor.system_errors.try_append(SystemErrorEvent::new(
                    "native_memory_update_failed",
                    "Native memory update failed",
                ).with_error_chain([error]));
            }
        });
        true
    }

    async fn run_native_memory_update(
        &self,
        native_memory: &noema_memory::NativeMemory,
        conversation_id: &str,
    ) -> Result<(), String> {
        let primary = self.store.primary_conversation_for_human("human:local").await.map_err(|error| error.to_string())?;
        if primary.as_ref().map(|conversation| conversation.conversation_id.as_str()) != Some(conversation_id) {
            return Err("native memory updates require the local primary conversation".to_string());
        }
        let checkpoint = native_memory.state().map_err(|error| error.to_string())?;
        let cursor = if checkpoint.conversation_id.as_deref() == Some(conversation_id) {
            checkpoint.last_consolidated_sequence
        } else {
            0
        };
        let captured = self.store.capture_memory_source_range(
            conversation_id,
            cursor,
        ).await.map_err(|error| error.to_string())?;
        if captured.items.is_empty() {
            return Ok(());
        }
        let route = self.resolve_memory_provider().await.map_err(|error| error.to_string())?;
        let selection = route.selection().clone();
        let provider = route.operations();
        let context_budget = provider.context_metadata(selection.model_profile.as_deref())
            .context_window_tokens
            .unwrap_or(8_000)
            .saturating_sub(2_048)
            .saturating_mul(3)
            .max(1) as usize;
        let items = captured.items;
        let mut offset = 0;
        while offset < items.len() {
            let canonical_pages = native_memory.list_pages().map_err(|error| error.to_string())?;
            let mut allowed_sources = canonical_pages
                .iter()
                .flat_map(|page| page.sources.iter().cloned())
                .collect::<std::collections::HashSet<_>>();
            let canonical = serde_json::to_string(&canonical_pages).map_err(|error| error.to_string())?;
            let canonical_chars = canonical.chars().count();
            if canonical_chars >= context_budget {
                return Err(format!("canonical memory pages exceed the model context budget ({canonical_chars} >= {context_budget} characters)"));
            }
            let mut end = offset;
            let mut chunk_chars = canonical_chars;
            while end < items.len() {
                let item_chars = items[end].content_text.as_deref().map_or(0, |text| text.chars().count()).saturating_add(80);
                if end > offset && chunk_chars.saturating_add(item_chars) > context_budget {
                    break;
                }
                if end == offset && chunk_chars.saturating_add(item_chars) > context_budget {
                    return Err(format!("conversation item {} exceeds the model context budget", items[end].item_id));
                }
                chunk_chars = chunk_chars.saturating_add(item_chars);
                end += 1;
            }
            if end == offset {
                return Err("memory update could not fit a conversation item in the model context".to_string());
            }
            let chunk = &items[offset..end];
            allowed_sources.extend(
                chunk
                    .iter()
                    .filter(|item| item.kind == ConversationItemKind::UserText)
                    .map(|item| item.item_id.clone()),
            );
            let source = chunk.iter().filter_map(|item| {
                let role = match item.kind {
                    ConversationItemKind::UserText => "human",
                    ConversationItemKind::AssistantText => "assistant",
                    _ => return None,
                };
                Some(format!("{} [{}] {}", role, item.item_id, item.content_text.as_deref().unwrap_or_default()))
            }).collect::<Vec<_>>().join("\n");
            let response = provider.generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.to_string()),
                    model: selection.model_profile.clone(),
                    input: GenerateInput::Text(source),
                    instructions: Some(format!("Update native Markdown memory. Existing canonical pages (including stable ids and exact hashes) are: {canonical}\nReturn only JSON matching {{\"upserts\":[{{\"id\":null,\"expected_hash\":null,\"path\":\"relative.md\",\"title\":\"Title\",\"body\":\"Claim [^fact]\\n\\n[^fact]: source-id\",\"sources\":[\"source-id\"]}}],\"deletes\":[]}}. Every cited footnote must have one definition whose exact target is a source id, and the definitions must exactly match sources. Preserve ids, expected hashes, hierarchy, and user-authored prose unless evidence requires a change. To move a page, keep its id and expected hash but change its path; the old path is removed automatically. Human messages are evidence; assistant messages are context only and never evidence. Do not copy secrets, tokens, credentials, or private keys. Use owner human:local and scope human:local.")),
                    options: GenerateOptions { generation_priority: GenerationPriority::Background, max_output_tokens: Some(2_048), reasoning_effort: selection.reasoning_effort, ..GenerateOptions::default() },
                    tools: Vec::new(), tool_choice: Default::default(), parallel_tool_calls: false,
                },
                &mut |_| {},
            ).await.map_err(|error| error.to_string())?;
            let text = response.assistant_text();
            let json_start = text.find('{').ok_or_else(|| "memory model returned no JSON change set".to_string())?;
            let json_end = text.rfind('}').ok_or_else(|| "memory model returned incomplete JSON change set".to_string())?;
            let changes: noema_memory::MemoryChangeSet = serde_json::from_str(&text[json_start..=json_end]).map_err(|error| format!("invalid memory change set: {error}"))?;
            if let Some(source) = changes
                .upserts
                .iter()
                .flat_map(|change| &change.sources)
                .find(|source| !allowed_sources.contains(*source))
            {
                return Err(format!(
                    "memory change set cites source {source} that is neither existing provenance nor a human message in this chunk"
                ));
            }
            let last = chunk.last().expect("non-empty chunk");
            native_memory.publish_with_state(&changes, &noema_memory::MemoryState {
                conversation_id: Some(conversation_id.to_string()),
                last_consolidated_sequence: last.sequence_index,
                last_consolidated_item: Some(last.item_id.clone()),
                updated_at: String::new(),
            }).map_err(|error| error.to_string())?;
            offset = end;
        }
        Ok(())
    }
}
