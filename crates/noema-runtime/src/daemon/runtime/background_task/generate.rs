impl RuntimeActor {
    pub(super) async fn generate_background_task(
        &self,
        request: BackgroundTaskGenerateRequest,
    ) -> Result<BackgroundTaskGenerateResult, RuntimeError> {
        let started_at = Instant::now();
        let setup_debug = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::AgentRun(request.run_id.clone()),
            RuntimeDebugSpanCategory::Runtime,
            "Prepare agent run",
            RuntimeDebugMetadata::default(),
        )
        .await;
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
        setup_debug
            .finish(RuntimeDebugSpanStatus::Completed, None)
            .await;
        let conversation_id = format!("task_run:{}", request.run_id);
        let turn_id = format!("task_turn:{}", request.run_id);
        let user_item_id = format!("task_input:{}", request.run_id);
        let tool_instructions = render_background_instance_identity(
            background_tool_instructions(&request.instructions, &model_tools),
            &request.instance_name,
        );
        let initial_provider_input = task_initial_provider_input(
            &request.input,
            request.runtime_environment.as_deref(),
        );
        let mut context = ContinuationContext::from_provider_input(initial_provider_input.clone());
        let initial_request = GenerateRequest {
            conversation_id: Some(conversation_id.clone()),
            model: provider_selection.model_profile.clone(),
            input: initial_provider_input.clone(),
            instructions: Some(tool_instructions),
            options: GenerateOptions {
                hosted_web_search: model_tools.hosted_web_search(),
                reasoning_effort: provider_selection.reasoning_effort,
                fast_mode: provider_selection.fast_mode,
                max_output_tokens: Some(8_000),
                store_response: response_continuation.store_response(),
                ..GenerateOptions::default()
            },
            tools: model_tools.provider_tools(),
            tool_transport: model_tools.transport,
            tool_choice: if capabilities.allowed_tools && !model_tools.hosted_web_search() {
                model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
            } else {
                NoemaToolChoice::Auto
            },
            parallel_tool_calls: model_tools.transport
                == noema_providers::ProviderToolTransport::Native
                && model_tools.has_callable_tools()
                && capabilities.parallel_tool_calls,
        };
        admit_uncompacted_request(provider, &initial_request).await?;
        let mut citation_sources = CitationSourceRegistry::default();
        let mut next_provider_round = 0;
        let initial_response = self
            .generate_task_provider_round(
                provider,
                initial_request,
                &model_tools.bindings,
                &request.run_id,
                &request.task_id,
                &request.lease_token,
                request.task_generation,
                "initial",
                0,
                deadline,
                &request.cancellation,
                &request.runtime_events,
            )
            .await;
        let mut response = match initial_response {
            Ok(response) => {
                citation_sources.observe(0, &response.hosted_web_searches);
                next_provider_round = 1;
                response
            }
            Err(error) if is_wall_time_error(&error) => {
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
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
        let mut completed_tool_calls = response.hosted_web_searches.len();
        let mut invalid_terminal_attempts = 0usize;

        for continuation_index in 0..=max_continuations {
            if request.cancellation.is_cancelled() {
                return Err(RuntimeError::Protocol(
                    "task execution cancelled".to_string(),
                ));
            }
            let calls = local_tool_calls(&response.tool_calls);
            if calls.is_empty() {
                return self
                    .finalize_background_task(
                        &request,
                        provider,
                        &conversation_id,
                        &model_tools,
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
                        "model returned without the required terminal tool",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let terminal_calls = calls
                .iter()
                .filter(|call| is_task_terminal_tool(&call.name))
                .collect::<Vec<_>>();
            if (!terminal_calls.is_empty() || invalid_terminal_attempts == 1)
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
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            if regular_tool_budget_exceeded(completed_tool_calls, calls.len(), max_tool_calls) {
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
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
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
                task_run_fence: Some(request.work_run_fence()),
                cwd: None,
                provider_kind: provider_selection.provider_kind.clone(),
                model: provider_selection.model_profile.clone(),
                reasoning_effort: provider_selection.reasoning_effort,
                fast_mode: provider_selection.fast_mode,
                provider_route: Arc::clone(&provider_route),
                initial_stream_id: format!("task_stream:{}:{continuation_index}", request.run_id),
                response: response.clone(),
                agent_identity: agent_identity.clone(),
                runtime_environment: current_runtime_environment(None),
                tool_capabilities: capabilities,
                initial_model_tools: model_tools.clone(),
                continuation_model_tools: model_tools.clone(),
                initial_provider_input: initial_provider_input.clone(),
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
                let tool_debug_span = RuntimeDebugSpan::begin(
                    &self.store,
                    RuntimeDebugScope::AgentRun(request.run_id.clone()),
                    RuntimeDebugSpanCategory::Tool,
                    call.name.clone(),
                    RuntimeDebugMetadata {
                        round_index: Some(continuation_index as u64),
                        tool_name: Some(call.name.clone()),
                        correlation_id: Some(correlation_id.clone()),
                        ..RuntimeDebugMetadata::default()
                    },
                )
                .await;
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
                            capabilities,
                            response_continuation,
                            &mut context,
                            &mut citation_sources,
                            next_provider_round,
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
                tool_debug_span
                    .finish(
                        if result.success {
                            RuntimeDebugSpanStatus::Completed
                        } else {
                            RuntimeDebugSpanStatus::Failed
                        },
                        None,
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
                let blocked = result.is_blocked();
                results.push(result);
                if blocked {
                    break;
                }
            }
            completed_tool_calls = completed_tool_calls.saturating_add(results.len());
            if results.iter().any(|result| {
                !result.success
                    && result
                        .payload
                        .get("code")
                        .and_then(serde_json::Value::as_str)
                        == Some("invalid_task_terminal")
            }) {
                invalid_terminal_attempts = invalid_terminal_attempts.saturating_add(1);
            }
            progress.observe_results(&results);
            context.append_results(&results);
            context.finish_round();
            if should_stop_after_tool_results(&results) {
                return Err(RuntimeError::OutcomeUncertain);
            }
            if invalid_terminal_attempts >= 2 {
                return Err(RuntimeError::TaskTerminalInvalid(
                    "terminal payload remained invalid after one repair".to_string(),
                ));
            }
            if !results
                .iter()
                .any(|result| result.requires_provider_continuation)
            {
                response.usage = aggregate_usage;
                return Ok(BackgroundTaskGenerateResult { response });
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
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
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
                            capabilities,
                            response_continuation,
                            &mut context,
                            &mut citation_sources,
                            next_provider_round,
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
                                capabilities,
                                response_continuation,
                                &mut context,
                                &mut citation_sources,
                                next_provider_round,
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
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
                        "maximum provider tool continuations reached",
                        deadline,
                        aggregate_usage,
                    )
                    .await;
            }
            let terminal_repair = invalid_terminal_attempts == 1;
            let repair_tools = task_terminal_tools(&model_tools);
            let instructions = if terminal_repair {
                terminal_tool_instructions(
                    "The previous terminal payload was rejected. Correct it using the exact contract below; do not perform more work or call non-terminal tools.",
                    &model_tools,
                    &repair_tools,
                )
            } else {
                build_role_tool_result_continuation_system_prompt(
                    &request.instructions,
                    &request.input,
                    &render_continuation_tool_names(&model_tools),
                )
            };
            let (continuation_tools, continuation_tool_choice, parallel_tool_calls) =
                if terminal_repair {
                    let choice = if capabilities.allowed_tools {
                        NoemaToolChoice::Allowed(NoemaAllowedTools {
                            mode: NoemaAllowedToolsMode::Required,
                            tools: repair_tools.iter().map(|tool| tool.name.clone()).collect(),
                        })
                    } else {
                        NoemaToolChoice::Required
                    };
                    let tools = if capabilities.allowed_tools {
                        model_tools.provider_tools()
                    } else {
                        model_tools.provider_tools_for_specs(&repair_tools)
                    };
                    (tools, choice, false)
                } else {
                    (
                        model_tools.provider_tools(),
                        if capabilities.allowed_tools && !model_tools.hosted_web_search() {
                            model_tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
                        } else {
                            NoemaToolChoice::Auto
                        },
                        model_tools.transport == noema_providers::ProviderToolTransport::Native
                            && model_tools.has_callable_tools()
                            && capabilities.parallel_tool_calls,
                    )
                };
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
                        capabilities,
                        response_continuation,
                        &mut context,
                        &mut citation_sources,
                        next_provider_round,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    ).await;
                }
                result = context.compact_to_fit(
                    provider,
                    provider_selection.model_profile.as_deref(),
                    capabilities.native_tool_results,
                    &instructions,
                    &continuation_tools,
                    !terminal_repair && model_tools.hosted_web_search(),
                    Some(8_000),
                    provider_selection.reasoning_effort,
                    noema_providers::GenerationPriority::Background,
                    &request.input,
                ) => result,
            };
            if propagate_compaction_result(compaction_result)? {
                append_task_document_after_compaction(
                    &self.store,
                    &request.task_id,
                    &mut context,
                )
                .await?;
                let readmission = context
                    .compact_to_fit(
                        provider,
                        provider_selection.model_profile.as_deref(),
                        capabilities.native_tool_results,
                        &instructions,
                        &continuation_tools,
                        !terminal_repair && model_tools.hosted_web_search(),
                        Some(8_000),
                        provider_selection.reasoning_effort,
                        noema_providers::GenerationPriority::Background,
                        &request.input,
                    )
                    .await;
                propagate_compaction_result(readmission)?;
            }
            let continuation_input = context
                .next_provider_input(capabilities.native_tool_results, response_continuation);
            let continuation_request = GenerateRequest {
                conversation_id: Some(conversation_id.clone()),
                model: provider_selection.model_profile.clone(),
                input: continuation_input.input,
                instructions: Some(instructions.clone()),
                options: GenerateOptions {
                    hosted_web_search: !terminal_repair && model_tools.hosted_web_search(),
                    reasoning_effort: provider_selection.reasoning_effort,
                    fast_mode: provider_selection.fast_mode,
                    max_output_tokens: Some(8_000),
                    previous_response_id: continuation_input.previous_response_id.clone(),
                    store_response: response_continuation.store_response(),
                    ..GenerateOptions::default()
                },
                tools: continuation_tools.clone(),
                tool_transport: model_tools.transport,
                tool_choice: continuation_tool_choice.clone(),
                parallel_tool_calls,
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
                    "continuation",
                    continuation_step as i64,
                    deadline,
                    &request.cancellation,
                    &request.runtime_events,
                )
                .await;
            if matches!(&continuation_response, Err(RuntimeError::Provider(_)))
                && continuation_input.previous_response_id.is_some()
            {
                let fallback_request = GenerateRequest {
                    conversation_id: Some(conversation_id.clone()),
                    model: provider_selection.model_profile.clone(),
                    input: context.provider_input(capabilities.native_tool_results),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        hosted_web_search: !terminal_repair && model_tools.hosted_web_search(),
                        reasoning_effort: provider_selection.reasoning_effort,
                        fast_mode: provider_selection.fast_mode,
                        max_output_tokens: Some(8_000),
                        store_response: response_continuation.store_response(),
                        ..GenerateOptions::default()
                    },
                    tools: continuation_tools,
                    tool_transport: model_tools.transport,
                    tool_choice: continuation_tool_choice,
                    parallel_tool_calls,
                };
                admit_uncompacted_request(provider, &fallback_request).await?;
                continuation_response = self
                    .generate_task_provider_round(
                        provider,
                        fallback_request,
                        &model_tools.bindings,
                        &request.run_id,
                        &request.task_id,
                        &request.lease_token,
                        request.task_generation,
                        "continuation",
                        continuation_step as i64,
                        deadline,
                        &request.cancellation,
                        &request.runtime_events,
                    )
                    .await;
            }
            response = match continuation_response {
                Ok(response) => {
                    citation_sources
                        .observe(continuation_step, &response.hosted_web_searches);
                    completed_tool_calls = completed_tool_calls
                        .saturating_add(response.hosted_web_searches.len());
                    next_provider_round = continuation_step.saturating_add(1);
                    response
                }
                Err(error) if is_wall_time_error(&error) => {
                    return self
                        .finalize_background_task(
                            &request,
                            provider,
                            &conversation_id,
                            &model_tools,
                            capabilities,
                            response_continuation,
                            &mut context,
                            &mut citation_sources,
                            next_provider_round,
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

fn render_background_instance_identity(instructions: String, instance_name: &str) -> String {
    format!(
        "{instructions}\n\nSubagent instance identity:\n- instance_name: {}\n- This label is assigned by Noema and remains stable for this run. Do not rename it or claim it is user-chosen.",
        serde_json::to_string(instance_name).expect("serializing an instance name should not fail")
    )
}

fn regular_tool_budget_exceeded(completed: usize, pending: usize, maximum: usize) -> bool {
    completed.saturating_add(pending) > maximum
}
