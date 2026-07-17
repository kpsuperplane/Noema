impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    async fn finalize_background_task(
        &self,
        request: &BackgroundTaskGenerateRequest,
        provider: &dyn noema_providers::ProviderOperations,
        conversation_id: &str,
        model_tools: &ModelTools,
        context: &ContinuationContext,
        reason: &str,
        deadline: tokio::time::Instant,
        mut aggregate_usage: Option<TokenUsage>,
    ) -> Result<GenerateResponse, RuntimeError> {
        let now = tokio::time::Instant::now();
        let deadline = task_finalization_deadline(deadline, now);
        let terminal_tools = terminal_contract_tools(model_tools);
        if terminal_tools.is_empty() {
            return Err(RuntimeError::Protocol(
                "task execution role has no terminal contract tool".to_string(),
            ));
        }
        let terminal_bindings = binding_snapshot_for_specs(model_tools, &terminal_tools)?;
        let instructions = terminal_tool_instructions(
            &build_task_finalization_prompt(request.role, reason, &request.input),
            model_tools,
            &terminal_tools,
        );
        let capabilities =
            provider.tool_capabilities(request.provider_selection.model_profile.as_deref());
        let response_continuation =
            provider.response_continuation(request.provider_selection.model_profile.as_deref());
        let continuation_input =
            context.next_provider_input(capabilities.native_tool_results, response_continuation);
        let (finalization_tools, finalization_tool_choice) = if capabilities.allowed_tools {
            (
                model_tools.provider_tools(),
                NoemaToolChoice::Allowed(NoemaAllowedTools {
                    mode: NoemaAllowedToolsMode::Required,
                    tools: terminal_tools
                        .iter()
                        .map(|tool| tool.name.clone())
                        .collect(),
                }),
            )
        } else {
            (terminal_tools.clone(), NoemaToolChoice::Required)
        };
        let chained = continuation_input.previous_response_id.is_some();
        let finalization_request = GenerateRequest {
            conversation_id: Some(conversation_id.to_string()),
            model: request.provider_selection.model_profile.clone(),
            input: continuation_input.input,
            instructions: Some(instructions.clone()),
            options: GenerateOptions {
                require_noema_response: true,
                reasoning_effort: request.provider_selection.reasoning_effort,
                max_output_tokens: Some(8_000),
                previous_response_id: continuation_input.previous_response_id,
                store_response: response_continuation.store_response(),
                ..GenerateOptions::default()
            },
            tools: finalization_tools.clone(),
            tool_choice: finalization_tool_choice.clone(),
            parallel_tool_calls: false,
        };
        let mut finalization_result = self
            .generate_task_provider_round(
                provider,
                finalization_request,
                &terminal_bindings,
                &request.run_id,
                &request.task_id,
                &request.lease_token,
                request.execution_policy.max_provider_continuations,
                deadline,
                &request.cancellation,
                &request.runtime_events,
            )
            .await;
        if chained && matches!(&finalization_result, Err(RuntimeError::Provider(_))) {
            finalization_result = self
                .generate_task_provider_round(
                    provider,
                    GenerateRequest {
                        conversation_id: Some(conversation_id.to_string()),
                        model: request.provider_selection.model_profile.clone(),
                        input: context.provider_input(
                            provider
                                .tool_capabilities(
                                    request.provider_selection.model_profile.as_deref(),
                                )
                                .native_tool_results,
                        ),
                        instructions: Some(instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
                            reasoning_effort: request.provider_selection.reasoning_effort,
                            max_output_tokens: Some(8_000),
                            store_response: response_continuation.store_response(),
                            ..GenerateOptions::default()
                        },
                        tools: finalization_tools,
                        tool_choice: finalization_tool_choice,
                        parallel_tool_calls: false,
                    },
                    &terminal_bindings,
                    &request.run_id,
                    &request.task_id,
                    &request.lease_token,
                    request.execution_policy.max_provider_continuations,
                    deadline,
                    &request.cancellation,
                    &request.runtime_events,
                )
                .await;
        }
        let mut response = finalization_result?;
        add_usage(&mut aggregate_usage, response.usage.as_ref());
        response.usage = aggregate_usage;
        let terminal_calls = response
            .tool_calls
            .iter()
            .filter(|call| is_task_terminal_tool(&call.name))
            .collect::<Vec<_>>();
        if response.tool_calls.len() != 1
            || terminal_calls.len() != 1
            || !is_valid_terminal_tool(request.role, &terminal_calls[0].name)
        {
            return Err(RuntimeError::Protocol(format!(
                "task terminal contract missing or ambiguous after {reason}"
            )));
        }
        let terminal_call = terminal_calls[0];
        let correlation_id = terminal_call
            .provider_call_id
            .clone()
            .or_else(|| terminal_call.id.clone())
            .unwrap_or_else(|| "terminal".to_string());
        let round_index = request.execution_policy.max_provider_continuations;
        let tool_call_item_id = format!(
            "run_item:tool_call:{}:{}:{}",
            request.run_id, round_index, correlation_id
        );
        self.persist_task_run_item(
            &request.task_id,
            &request.runtime_events,
            NewAgentRunItem {
                item_id: Some(tool_call_item_id.clone()),
                run_id: request.run_id.clone(),
                round_index,
                kind: noema_tasks::AgentRunItemKind::ToolCall,
                status: noema_tasks::AgentRunItemStatus::Completed,
                correlation_id: Some(correlation_id.clone()),
                parent_item_id: None,
                content_text: Some(terminal_call.name.clone()),
                payload: serde_json::json!({
                    "id": terminal_call.id,
                    "call_id": terminal_call.provider_call_id,
                    "provider_name": terminal_call.provider_name,
                    "arguments": persisted_capability_arguments(&terminal_bindings, &terminal_call.name, &terminal_call.payload),
                }),
            },
            &request.lease_token,
        )
        .await;
        self.persist_task_run_item(
            &request.task_id,
            &request.runtime_events,
            NewAgentRunItem {
                item_id: Some(format!(
                    "run_item:tool_result:{}:{}:{}",
                    request.run_id, round_index, correlation_id
                )),
                run_id: request.run_id.clone(),
                round_index,
                kind: noema_tasks::AgentRunItemKind::ToolResult,
                status: noema_tasks::AgentRunItemStatus::Completed,
                correlation_id: Some(correlation_id),
                parent_item_id: Some(tool_call_item_id),
                content_text: Some(terminal_call.name.clone()),
                payload: serde_json::json!({"accepted": true}),
            },
            &request.lease_token,
        )
        .await;
        Ok(response)
    }

    async fn persist_progress_notice(
        &self,
        request: &BackgroundTaskGenerateRequest,
        message: &str,
    ) {
        self.persist_task_run_item(
            &request.task_id,
            &request.runtime_events,
            NewAgentRunItem {
                item_id: None,
                run_id: request.run_id.clone(),
                round_index: 0,
                kind: noema_tasks::AgentRunItemKind::ProgressNotice,
                status: noema_tasks::AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some(message.to_string()),
                payload: serde_json::json!({}),
            },
            &request.lease_token,
        )
        .await;
    }
}
