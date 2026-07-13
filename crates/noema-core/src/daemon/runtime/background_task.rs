//! Provider/tool continuation loop for supervised task runs.

use std::time::{Duration, Instant};

use crate::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
    agent_execution::ExecutionRole,
    daemon::{agent_onboarding::AgentPromptIdentity, protocol::DaemonError},
    graphql::ConversationSubscriptionRegistry,
    provider::TokenUsage,
    store::NewAgentRunItem,
};

use super::{
    actor::CodexRuntimeActor,
    continuation_context::ContinuationContext,
    local_tools::LocalToolResult,
    model_tools::{ModelTools, build_model_tools_for_role},
    progress::{ContinuationProgressTracker, DeterministicProgressStop},
    progress_audit::ProgressAuditDecision,
    task_continuation::{
        add_usage, background_tool_instructions, build_task_finalization_prompt,
        is_task_terminal_tool, is_valid_terminal_tool, render_tool_names,
        task_tool_result_transcript_payload,
    },
    task_transcript::sanitize_task_tool_payload,
    tool_lifecycle::local_tool_calls,
    turn::SuccessfulProviderTurn,
};
use crate::daemon::prompts::build_role_tool_result_continuation_system_prompt;
use tokio_util::sync::CancellationToken;

/// Provider request for one background executor or reviewer run.
#[derive(Debug, Clone)]
pub(crate) struct BackgroundTaskGenerateRequest {
    /// Durable run id used as the stateless provider conversation id.
    pub run_id: String,
    /// Durable task id used to route GraphQL detail updates.
    pub task_id: String,
    /// Active lease token fencing every durable run write.
    pub lease_token: String,
    /// Per-run cancellation propagated through provider and tool futures.
    pub cancellation: CancellationToken,
    /// Built-in agent identity that owns this run.
    pub agent_id: String,
    /// Role policy applied to advertised and dispatched tools.
    pub role: ExecutionRole,
    /// Provider family selected by the persisted model snapshot.
    pub provider_kind: String,
    /// Provider model/profile selected by the persisted model snapshot.
    pub model: Option<String>,
    /// Explicit reasoning effort from the persisted model snapshot.
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
    /// Immutable provider-independent execution-policy snapshot.
    pub execution_policy: crate::TaskExecutionPolicy,
    /// User/task prompt supplied to the provider.
    pub input: String,
    /// System instructions for the executor or reviewer contract.
    pub instructions: String,
    /// GraphQL task subscription registry for live detail refreshes.
    pub task_subscriptions: ConversationSubscriptionRegistry,
}

impl CodexRuntimeActor {
    pub(super) async fn generate_background_task(
        &self,
        request: BackgroundTaskGenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        let started_at = Instant::now();
        let max_continuations =
            usize::try_from(request.execution_policy.max_provider_continuations)
                .unwrap_or(usize::MAX);
        let max_tool_calls =
            usize::try_from(request.execution_policy.max_tool_calls).unwrap_or(usize::MAX);
        let max_active_duration = Duration::from_secs(
            u64::try_from(request.execution_policy.max_active_minutes)
                .unwrap_or(u64::MAX)
                .saturating_mul(60),
        );
        let deadline = tokio::time::Instant::now() + max_active_duration;
        let provider = self.provider_for_kind(&request.provider_kind)?;
        let capabilities = provider.tool_capabilities(request.model.as_deref());
        let response_continuation = provider.response_continuation(request.model.as_deref());
        let model_tools =
            build_model_tools_for_role(&self.store, request.role, false, capabilities)
                .await
                .map_err(|error| DaemonError::Protocol(error.to_string()))?;
        let agent = self
            .store
            .get_agent(&request.agent_id)
            .await?
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown task agent: {}", request.agent_id))
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
                &provider,
                GenerateRequest {
                    conversation_id: Some(conversation_id.clone()),
                    model: request.model.clone(),
                    input: GenerateInput::Text(request.input.clone()),
                    instructions: Some(tool_instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        reasoning_effort: request.reasoning_effort,
                        max_output_tokens: Some(8_000),
                        store_response: response_continuation.store_response(),
                        ..GenerateOptions::default()
                    },
                    tools: model_tools.native.clone(),
                    tool_choice: Default::default(),
                    parallel_tool_calls: !model_tools.native.is_empty()
                        && capabilities.parallel_tool_calls,
                },
                &request.run_id,
                &request.task_id,
                &request.lease_token,
                0,
                deadline,
                &request.cancellation,
                &request.task_subscriptions,
            )
            .await;
        let mut response = match initial_response {
            Ok(response) => response,
            Err(error) if is_wall_time_error(&error) => {
                return self
                    .finalize_background_task(
                        &request,
                        &provider,
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
                return Err(DaemonError::Protocol(
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
                        &provider,
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
                return Err(DaemonError::Protocol(
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
                        &provider,
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
                        &provider,
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
                provider_kind: request.provider_kind.clone(),
                model: request.model.clone(),
                reasoning_effort: request.reasoning_effort,
                initial_stream_id: format!("task_stream:{}:{continuation_index}", request.run_id),
                response: response.clone(),
                agent_identity: agent_identity.clone(),
                tool_capabilities: capabilities,
                continuation_model_tools: model_tools.clone(),
                rendered_tools: render_tool_names(&model_tools),
                rendered_continuation_tools: render_tool_names(&model_tools),
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
                        return Err(DaemonError::Protocol("task execution cancelled".to_string()));
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
                            &provider,
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
                self.store
                    .record_agent_run_progress(
                        &request.run_id,
                        &request.lease_token,
                        1,
                        i64::try_from(tool_started_at.elapsed().as_millis()).unwrap_or(i64::MAX),
                    )
                    .await?;
                self.persist_task_run_item(
                    &request.task_id,
                    &request.task_subscriptions,
                    NewAgentRunItem {
                        item_id: Some(format!(
                            "run_item:tool_result:{}:{}:{}",
                            request.run_id, continuation_index, correlation_id
                        )),
                        run_id: request.run_id.clone(),
                        round_index: continuation_index as i64,
                        kind: "tool_result".to_string(),
                        status: if result.success() {
                            crate::store::AgentRunItemStatus::Completed
                        } else {
                            crate::store::AgentRunItemStatus::Failed
                        },
                        correlation_id: Some(correlation_id.clone()),
                        parent_item_id: Some(tool_call_item_id.clone()),
                        content_text: Some(result.name().to_string()),
                        payload: task_tool_result_transcript_payload(&result),
                    },
                    &request.lease_token,
                )
                .await;
                self.persist_task_run_item(
                    &request.task_id,
                    &request.task_subscriptions,
                    NewAgentRunItem {
                        item_id: Some(tool_call_item_id),
                        run_id: request.run_id.clone(),
                        round_index: continuation_index as i64,
                        kind: "tool_call".to_string(),
                        status: if result.success() {
                            crate::store::AgentRunItemStatus::Completed
                        } else {
                            crate::store::AgentRunItemStatus::Failed
                        },
                        correlation_id: Some(correlation_id),
                        parent_item_id: None,
                        content_text: Some(call.name.clone()),
                        payload: serde_json::json!({
                            "output_index": call.output_index,
                            "call_id": call.call_id,
                            "provider_call_id": call.provider_call_id,
                            "provider_name": call.provider_name,
                            "arguments": sanitize_task_tool_payload(&call.name, &call.payload),
                        }),
                    },
                    &request.lease_token,
                )
                .await;
                results.push(result);
            }
            completed_tool_calls = completed_tool_calls.saturating_add(results.len());
            progress.observe_results(&results);
            context.append_results(&results);
            context.finish_round();
            let _ = tokio::select! {
                _ = request.cancellation.cancelled() => {
                    return Err(DaemonError::Protocol("task execution cancelled".to_string()));
                }
                _ = tokio::time::sleep_until(deadline) => {
                    return self.finalize_background_task(
                        &request,
                        &provider,
                        &conversation_id,
                        &model_tools,
                        &context,
                        "task active wall-time safety ceiling reached",
                        deadline,
                        aggregate_usage,
                    ).await;
                }
                result = context.compact_if_needed(
                    provider.as_ref(),
                    request.model.as_deref(),
                    request.reasoning_effort,
                    &request.input,
                ) => result,
            };
            if !results
                .iter()
                .any(LocalToolResult::requires_provider_continuation)
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
                        &provider,
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
                        return Err(DaemonError::Protocol("task execution cancelled".to_string()));
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        return self.finalize_background_task(
                            &request,
                            &provider,
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
                                &provider,
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
                        &provider,
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
                &render_tool_names(&model_tools),
            );
            let continuation_request = GenerateRequest {
                conversation_id: Some(conversation_id.clone()),
                model: request.model.clone(),
                input: continuation_input.input,
                instructions: Some(instructions.clone()),
                options: GenerateOptions {
                    require_noema_response: true,
                    reasoning_effort: request.reasoning_effort,
                    max_output_tokens: Some(8_000),
                    previous_response_id: continuation_input.previous_response_id.clone(),
                    store_response: response_continuation.store_response(),
                    ..GenerateOptions::default()
                },
                tools: model_tools.native.clone(),
                tool_choice: Default::default(),
                parallel_tool_calls: !model_tools.native.is_empty()
                    && capabilities.parallel_tool_calls,
            };
            let mut continuation_response = self
                .generate_task_provider_round(
                    &provider,
                    continuation_request,
                    &request.run_id,
                    &request.task_id,
                    &request.lease_token,
                    continuation_step as i64,
                    deadline,
                    &request.cancellation,
                    &request.task_subscriptions,
                )
                .await;
            if matches!(&continuation_response, Err(DaemonError::Provider(_)))
                && continuation_input.previous_response_id.is_some()
            {
                continuation_response = self
                    .generate_task_provider_round(
                        &provider,
                        GenerateRequest {
                            conversation_id: Some(conversation_id.clone()),
                            model: request.model.clone(),
                            input: context.provider_input(capabilities.native_tool_results),
                            instructions: Some(instructions),
                            options: GenerateOptions {
                                require_noema_response: true,
                                reasoning_effort: request.reasoning_effort,
                                max_output_tokens: Some(8_000),
                                store_response: response_continuation.store_response(),
                                ..GenerateOptions::default()
                            },
                            tools: model_tools.native.clone(),
                            tool_choice: Default::default(),
                            parallel_tool_calls: !model_tools.native.is_empty()
                                && capabilities.parallel_tool_calls,
                        },
                        &request.run_id,
                        &request.task_id,
                        &request.lease_token,
                        continuation_step as i64,
                        deadline,
                        &request.cancellation,
                        &request.task_subscriptions,
                    )
                    .await;
            }
            response = match continuation_response {
                Ok(response) => response,
                Err(error) if is_wall_time_error(&error) => {
                    return self
                        .finalize_background_task(
                            &request,
                            &provider,
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

    #[allow(clippy::too_many_arguments)]
    async fn finalize_background_task(
        &self,
        request: &BackgroundTaskGenerateRequest,
        provider: &std::sync::Arc<dyn super::handle::RuntimeModelProvider>,
        conversation_id: &str,
        model_tools: &ModelTools,
        context: &ContinuationContext,
        reason: &str,
        deadline: tokio::time::Instant,
        mut aggregate_usage: Option<TokenUsage>,
    ) -> Result<GenerateResponse, DaemonError> {
        let now = tokio::time::Instant::now();
        let deadline = task_finalization_deadline(deadline, now);
        let terminal_tools = model_tools
            .native
            .iter()
            .filter(|tool| is_task_terminal_tool(tool.name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if terminal_tools.is_empty() {
            return Err(DaemonError::Protocol(
                "task execution role has no terminal contract tool".to_string(),
            ));
        }
        let instructions = build_task_finalization_prompt(request.role, reason, &request.input);
        let response_continuation = provider.response_continuation(request.model.as_deref());
        let continuation_input = context.next_provider_input(
            provider
                .tool_capabilities(request.model.as_deref())
                .native_tool_results,
            response_continuation,
        );
        let chained = continuation_input.previous_response_id.is_some();
        let finalization_request = GenerateRequest {
            conversation_id: Some(conversation_id.to_string()),
            model: request.model.clone(),
            input: continuation_input.input,
            instructions: Some(instructions.clone()),
            options: GenerateOptions {
                require_noema_response: true,
                reasoning_effort: request.reasoning_effort,
                max_output_tokens: Some(8_000),
                previous_response_id: continuation_input.previous_response_id,
                store_response: response_continuation.store_response(),
                ..GenerateOptions::default()
            },
            tools: terminal_tools.clone(),
            tool_choice: Default::default(),
            parallel_tool_calls: false,
        };
        let mut finalization_result = self
            .generate_task_provider_round(
                provider,
                finalization_request,
                &request.run_id,
                &request.task_id,
                &request.lease_token,
                request.execution_policy.max_provider_continuations,
                deadline,
                &request.cancellation,
                &request.task_subscriptions,
            )
            .await;
        if chained && matches!(&finalization_result, Err(DaemonError::Provider(_))) {
            finalization_result = self
                .generate_task_provider_round(
                    provider,
                    GenerateRequest {
                        conversation_id: Some(conversation_id.to_string()),
                        model: request.model.clone(),
                        input: context.provider_input(
                            provider
                                .tool_capabilities(request.model.as_deref())
                                .native_tool_results,
                        ),
                        instructions: Some(instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
                            reasoning_effort: request.reasoning_effort,
                            max_output_tokens: Some(8_000),
                            store_response: response_continuation.store_response(),
                            ..GenerateOptions::default()
                        },
                        tools: terminal_tools.clone(),
                        tool_choice: Default::default(),
                        parallel_tool_calls: false,
                    },
                    &request.run_id,
                    &request.task_id,
                    &request.lease_token,
                    request.execution_policy.max_provider_continuations,
                    deadline,
                    &request.cancellation,
                    &request.task_subscriptions,
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
            return Err(DaemonError::Protocol(format!(
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
            &request.task_subscriptions,
            NewAgentRunItem {
                item_id: Some(tool_call_item_id.clone()),
                run_id: request.run_id.clone(),
                round_index,
                kind: "tool_call".to_string(),
                status: crate::store::AgentRunItemStatus::Completed,
                correlation_id: Some(correlation_id.clone()),
                parent_item_id: None,
                content_text: Some(terminal_call.name.clone()),
                payload: serde_json::json!({
                    "id": terminal_call.id,
                    "call_id": terminal_call.provider_call_id,
                    "provider_name": terminal_call.provider_name,
                    "arguments": sanitize_task_tool_payload(&terminal_call.name, &terminal_call.payload),
                }),
            },
            &request.lease_token,
        )
        .await;
        self.persist_task_run_item(
            &request.task_id,
            &request.task_subscriptions,
            NewAgentRunItem {
                item_id: Some(format!(
                    "run_item:tool_result:{}:{}:{}",
                    request.run_id, round_index, correlation_id
                )),
                run_id: request.run_id.clone(),
                round_index,
                kind: "tool_result".to_string(),
                status: crate::store::AgentRunItemStatus::Completed,
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
            &request.task_subscriptions,
            NewAgentRunItem {
                item_id: None,
                run_id: request.run_id.clone(),
                round_index: 0,
                kind: "progress_notice".to_string(),
                status: crate::store::AgentRunItemStatus::Completed,
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

fn is_wall_time_error(error: &DaemonError) -> bool {
    matches!(
        error,
        DaemonError::Protocol(message)
            if message == "task active wall-time safety ceiling reached"
    )
}

fn task_finalization_deadline(
    _run_deadline: tokio::time::Instant,
    now: tokio::time::Instant,
) -> tokio::time::Instant {
    const FINALIZATION_GRACE: Duration = Duration::from_secs(30);
    now + FINALIZATION_GRACE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_run_deadline_gets_a_bounded_terminal_grace_window() {
        let now = tokio::time::Instant::now();
        let expired = now - Duration::from_secs(1);
        assert_eq!(
            task_finalization_deadline(expired, now),
            now + Duration::from_secs(30)
        );
    }
}
