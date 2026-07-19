impl RuntimeActor {
    pub(super) async fn generate_background_task(
        &self,
        request: BackgroundTaskGenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
        let started_at = Instant::now();
        let run_fence = request.work_run_fence();
        let max_continuations =
            usize::try_from(request.execution_policy.max_provider_continuations)
                .unwrap_or(usize::MAX);
        let max_tool_calls =
            usize::try_from(request.execution_policy.max_tool_calls).unwrap_or(usize::MAX);
        let max_active_duration = Duration::from_secs(
            u64::from(request.execution_policy.max_active_minutes).saturating_mul(60),
        );
        let deadline = tokio::time::Instant::now() + max_active_duration;
        let provider_route = Arc::new(
            self.resolve_static_provider_route(request.provider_selection.clone())
                .await?,
        );
        let provider_selection = provider_route.selection();
        let provider = provider_route.operations();
        let capabilities = provider.tool_capabilities(provider_selection.model_profile.as_deref());
        let response_continuation =
            provider.response_continuation(provider_selection.model_profile.as_deref());
        let model_tools = build_model_tools_for_role(
            &self.store,
            &self.capability_bindings,
            request.role,
            false,
            capabilities,
        )
        .await
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
        let agent = self
            .store
            .get_agent(&request.agent_id)
            .await?
            .ok_or_else(|| {
                RuntimeError::Protocol(format!("unknown task agent: {}", request.agent_id))
            })?;
        let agent_identity = AgentPromptIdentity {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
        };
        let conversation_id = format!("task_run:{}", request.run_id);
        let turn_id = format!("task_turn:{}", request.run_id);
        let user_item_id = format!("task_input:{}", request.run_id);
        let tool_instructions = background_tool_instructions(&request.instructions, &model_tools);
        let mut context = ContinuationContext::new(&request.input);
        let initial_response = self
            .generate_task_provider_round(
                provider,
                GenerateRequest {
                    conversation_id: Some(conversation_id.clone()),
                    model: provider_selection.model_profile.clone(),
                    input: GenerateInput::Text(request.input.clone()),
                    instructions: Some(tool_instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        reasoning_effort: provider_selection.reasoning_effort,
                        max_output_tokens: Some(8_000),
                        store_response: response_continuation.store_response(),
                        ..GenerateOptions::default()
                    },
                    tools: model_tools.provider_tools(),
                    tool_choice: if capabilities.allowed_tools {
                        model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
                    } else {
                        NoemaToolChoice::Auto
                    },
                    parallel_tool_calls: model_tools.transport
                        == noema_providers::ProviderToolTransport::Native
                        && model_tools.has_callable_tools()
                        && capabilities.parallel_tool_calls,
                },
                &model_tools.bindings,
                &request.run_id,
                &request.task_id,
                &request.lease_token,
                request.task_generation,
                request.contract_id.as_ref(),
                0,
                deadline,
                &request.cancellation,
                &request.runtime_events,
            )
            .await;
        let mut response = match initial_response {
            Ok(response) => response,
            Err(error) if is_wall_time_error(&error) => {
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        None,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        context.append_response(&response);
        let mut aggregate_usage = response.usage.clone();
        let mut progress = ContinuationProgressTracker::new(&request.input);
        let mut completed_tool_calls = 0usize;

        for continuation_index in 0..=max_continuations {
            if request.cancellation.is_cancelled() {
                return Err(RuntimeError::Protocol(
                    "task execution cancelled".to_string(),
                ));
            }
            let calls = if response.response_status == GenerateResponseStatus::NeedsTools {
                local_tool_calls(&response.tool_calls)
            } else {
                Vec::new()
            };
            if calls.is_empty() {
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "model returned without the required terminal contract",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let terminal_calls = calls
                .iter()
                .filter(|call| is_task_terminal_tool(&call.name))
                .collect::<Vec<_>>();
            if !terminal_calls.is_empty()
                && (calls.len() != 1
                    || terminal_calls.len() != 1
                    || !is_valid_terminal_tool(request.role, &terminal_calls[0].name))
            {
                return Err(RuntimeError::Protocol(
                    "task response must contain exactly one role-valid terminal call and no mixed calls"
                        .to_string(),
                ));
            }
            if started_at.elapsed() >= max_active_duration {
                self.mark_task_calls_skipped(
                    &request,
                    continuation_index as i64,
                    &calls,
                    "task active wall-time safety ceiling reached",
                )
                .await;
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            if completed_tool_calls.saturating_add(calls.len()) > max_tool_calls {
                self.mark_task_calls_skipped(
                    &request,
                    continuation_index as i64,
                    &calls,
                    "task tool-call safety ceiling reached",
                )
                .await;
                self.persist_progress_notice(
                    &request,
                    "Task tool-call safety ceiling reached; finalizing with completed work.",
                )
                .await;
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "task tool-call safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let turn = SuccessfulProviderTurn {
                conversation_id: conversation_id.clone(),
                turn_id: turn_id.clone(),
                turn_index: 0,
                user_item_id: user_item_id.clone(),
                user_input: request.input.clone(),
                task_id: Some(request.task_id.clone()),
                task_run_id: Some(request.run_id.clone()),
                cwd: None,
                provider_kind: provider_selection.provider_kind.clone(),
                model: provider_selection.model_profile.clone(),
                reasoning_effort: provider_selection.reasoning_effort,
                provider_route: Arc::clone(&provider_route),
                initial_stream_id: format!("task_stream:{}:{continuation_index}", request.run_id),
                response: response.clone(),
                agent_identity: agent_identity.clone(),
                runtime_environment: current_runtime_environment(None),
                tool_capabilities: capabilities,
                initial_model_tools: model_tools.clone(),
                continuation_model_tools: model_tools.clone(),
                initial_provider_input: GenerateInput::Text(request.input.clone()),
            };
            let mut results = Vec::with_capacity(calls.len());
            for (call_index, call) in calls.iter().enumerate() {
                let correlation_id = call
                    .provider_call_id
                    .clone()
                    .or_else(|| call.call_id.clone())
                    .unwrap_or_else(|| format!("output-{}", call.output_index));
                let tool_call_item_id = format!(
                    "run_item:tool_call:{}:{}:{}",
                    request.run_id, continuation_index, correlation_id
                );
                let tool_started_at = Instant::now();
                let result = tokio::select! {
                    _ = request.cancellation.cancelled() => {
                        return Err(RuntimeError::Protocol("task execution cancelled".to_string()));
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        self.mark_task_calls_skipped(
                            &request,
                            continuation_index as i64,
                            &calls[call_index..],
                            "task active wall-time safety ceiling reached",
                        ).await;
                        context.append_results(&results);
                        context.finish_round();
                        return self.finalize_background_task(
                            &request,
                            provider,
                            &conversation_id,
                            &model_tools,
                            &context,
                            "task active wall-time safety ceiling reached",
                            deadline,
                            aggregate_usage,
                        ).await;
                    }
                    result = self.execute_local_tool_with_policy(
                        &turn,
                        &agent_identity,
                        call,
                        &model_tools.tool_policy,
                    ) => result,
                };
                self.persist_progress_notice(
                    &request,
                    &format!(
                        "Tool call {} completed in {} ms.",
                        call.name,
                        tool_started_at.elapsed().as_millis()
                    ),
                )
                .await;
                self.persist_task_run_item(
                    &request.task_id,
                    &request.runtime_events,
                    NewAgentRunItem {
                        item_id: Some(format!(
                            "run_item:tool_result:{}:{}:{}",
                            request.run_id, continuation_index, correlation_id
                        )),
                        run_id: request.run_id.clone(),
                        round_index: continuation_index as i64,
                        kind: noema_tasks::AgentRunItemKind::ToolResult,
                        status: if result.success {
                            noema_tasks::AgentRunItemStatus::Completed
                        } else {
                            noema_tasks::AgentRunItemStatus::Failed
                        },
                        correlation_id: Some(correlation_id.clone()),
                        parent_item_id: Some(tool_call_item_id.clone()),
                        content_text: Some(result.name.clone()),
                        payload: task_tool_result_transcript_payload(&result),
                    },
                    &run_fence,
                )
                .await;
                self.persist_task_run_item(
                    &request.task_id,
                    &request.runtime_events,
                    NewAgentRunItem {
                        item_id: Some(tool_call_item_id),
                        run_id: request.run_id.clone(),
                        round_index: continuation_index as i64,
                        kind: noema_tasks::AgentRunItemKind::ToolCall,
                        status: if result.success {
                            noema_tasks::AgentRunItemStatus::Completed
                        } else {
                            noema_tasks::AgentRunItemStatus::Failed
                        },
                        correlation_id: Some(correlation_id),
                        parent_item_id: None,
                        content_text: Some(call.name.clone()),
                        payload: serde_json::json!({
                            "output_index": call.output_index,
                            "call_id": call.call_id,
                            "provider_call_id": call.provider_call_id,
                            "provider_name": call.provider_name,
                            "arguments": result.persisted.arguments.clone().unwrap_or_else(super::task_transcript::omitted_capability_payload),
                        }),
                    },
                    &run_fence,
                )
                .await;
                let blocked = result.blocked_action_id.is_some();
                results.push(result);
                if blocked {
                    break;
                }
            }
            completed_tool_calls = completed_tool_calls.saturating_add(results.len());
            progress.observe_results(&results);
            context.append_results(&results);
            context.finish_round();
            let compaction_result = tokio::select! {
                _ = request.cancellation.cancelled() => {
                    return Err(RuntimeError::Protocol("task execution cancelled".to_string()));
                }
                _ = tokio::time::sleep_until(deadline) => {
                    return self.finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    ).await;
                }
                result = context.compact_if_needed(
                    provider,
                    provider_selection.model_profile.as_deref(),
                    provider_selection.reasoning_effort,
                    noema_providers::GenerationPriority::Background,
                    &request.input,
                ) => result,
            };
            propagate_compaction_result(compaction_result)?;
            if !results
                .iter()
                .any(|result| result.requires_provider_continuation)
            {
                response.usage = aggregate_usage;
                return Ok(response);
            }
            let continuation_step = continuation_index + 1;
            progress.mark_continuation_step(continuation_step);
            if let Some(stop) = progress.deterministic_stop() {
                let reason = match stop {
                    DeterministicProgressStop::RepeatedArguments => "repeated tool arguments",
                    DeterministicProgressStop::FailureStreak => "repeated tool failures",
                };
                self.persist_progress_notice(
                    &request,
                    &format!("Task progress stopped after {reason}; finalizing current work."),
                )
                .await;
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        reason,
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let audit_interval = usize::try_from(request.execution_policy.progress_audit_interval)
                .unwrap_or(usize::MAX);
            if continuation_step > 0 && continuation_step.is_multiple_of(audit_interval) {
                let digest = progress.digest(continuation_step);
                let audit_result = tokio::select! {
                    _ = request.cancellation.cancelled() => {
                        return Err(RuntimeError::Protocol("task execution cancelled".to_string()));
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        return self.finalize_background_task(
                            &request,
                            provider,
                            &conversation_id,
                            &model_tools,
                            &context,
                            "task active wall-time safety ceiling reached",
                            deadline,
                            aggregate_usage,
                        ).await;
                    }
                    result = self.run_progress_audit(&digest) => result,
                };
                if let Ok(audit) = audit_result {
                    self.persist_progress_notice(&request, &audit.user_summary)
                        .await;
                    progress.update_current_goal(audit.next_goal);
                    progress.reset_window();
                    if audit.decision != ProgressAuditDecision::Continue {
                        let reason = match audit.decision {
                            ProgressAuditDecision::Finalize => {
                                "progress audit requested finalization"
                            }
                            ProgressAuditDecision::AskHuman => {
                                "progress audit requires human input"
                            }
                            ProgressAuditDecision::Checkpoint => {
                                "progress audit requested a checkpoint"
                            }
                            ProgressAuditDecision::Continue => unreachable!(),
                        };
                        return self
                            .finalize_background_task(
                                &request,
                                provider,
                                &conversation_id,
                                &model_tools,
                                &context,
                                reason,
                                deadline,
                                aggregate_usage,
                            )
                            .await;
                    }
                }
            }
            if continuation_step >= max_continuations {
                self.persist_progress_notice(
                    &request,
                    "Task continuation safety ceiling reached; finalizing with completed work.",
                )
                .await;
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "maximum provider tool continuations reached",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let continuation_input = context
                .next_provider_input(capabilities.native_tool_results, response_continuation);
            let instructions = build_role_tool_result_continuation_system_prompt(
                &request.instructions,
                &request.input,
                &render_continuation_tool_names(&model_tools),
            );
            let continuation_request = GenerateRequest {
                conversation_id: Some(conversation_id.clone()),
                model: provider_selection.model_profile.clone(),
                input: continuation_input.input,
                instructions: Some(instructions.clone()),
                options: GenerateOptions {
                    require_noema_response: true,
                    reasoning_effort: provider_selection.reasoning_effort,
                    max_output_tokens: Some(8_000),
                    previous_response_id: continuation_input.previous_response_id.clone(),
                    store_response: response_continuation.store_response(),
                    ..GenerateOptions::default()
                },
                tools: model_tools.provider_tools(),
                tool_choice: if capabilities.allowed_tools {
                    model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
                } else {
                    NoemaToolChoice::Auto
                },
                parallel_tool_calls: model_tools.transport
                    == noema_providers::ProviderToolTransport::Native
                    && model_tools.has_callable_tools()
                    && capabilities.parallel_tool_calls,
            };
            let mut continuation_response = self
                .generate_task_provider_round(
                    provider,
                    continuation_request,
                    &model_tools.bindings,
                    &request.run_id,
                    &request.task_id,
                    &request.lease_token,
                    request.task_generation,
                    request.contract_id.as_ref(),
                    continuation_step as i64,
                    deadline,
                    &request.cancellation,
                    &request.runtime_events,
                )
                .await;
            if matches!(&continuation_response, Err(RuntimeError::Provider(_)))
                && continuation_input.previous_response_id.is_some()
            {
                continuation_response = self
                    .generate_task_provider_round(
                        provider,
                        GenerateRequest {
                            conversation_id: Some(conversation_id.clone()),
                            model: provider_selection.model_profile.clone(),
                            input: context.provider_input(capabilities.native_tool_results),
                            instructions: Some(instructions),
                            options: GenerateOptions {
                                require_noema_response: true,
                                reasoning_effort: provider_selection.reasoning_effort,
                                max_output_tokens: Some(8_000),
                                store_response: response_continuation.store_response(),
                                ..GenerateOptions::default()
                            },
                            tools: model_tools.provider_tools(),
                            tool_choice: if capabilities.allowed_tools {
                                model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
                            } else {
                                NoemaToolChoice::Auto
                            },
                            parallel_tool_calls: model_tools.transport
                                == noema_providers::ProviderToolTransport::Native
                                && model_tools.has_callable_tools()
                                && capabilities.parallel_tool_calls,
                        },
                        &model_tools.bindings,
                        &request.run_id,
                        &request.task_id,
                        &request.lease_token,
                        request.task_generation,
                        request.contract_id.as_ref(),
                        continuation_step as i64,
                        deadline,
                        &request.cancellation,
                        &request.runtime_events,
                    )
                    .await;
            }
            response = match continuation_response {
                Ok(response) => response,
                Err(error) if is_wall_time_error(&error) => {
                    return self
                        .finalize_background_task(
                            &request,
                            provider,
                            &conversation_id,
                            &model_tools,
                            &context,
                            "task active wall-time safety ceiling reached",
                            deadline,
                            aggregate_usage,
                        )
                        .await;
                }
                Err(error) => return Err(error),
            };
            add_usage(&mut aggregate_usage, response.usage.as_ref());
            context.append_response(&response);
        }
        unreachable!("task continuation loop exits through a terminal outcome")
    }
}
