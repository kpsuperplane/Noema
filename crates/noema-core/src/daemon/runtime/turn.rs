use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus, ReplayMode, SYSTEM_ERROR_RUNTIME_INVARIANT,
    SystemErrorEvent,
    capability::GatewayToolResult,
    provider::{
        GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
        GenerateStreamEvent, GenerateToolCall, MultipleChoiceOption, MultipleChoiceSelectionMode,
        NoemaAllowedToolsMode, NoemaToolChoice, PromptCacheMode, PromptCacheOptions,
        PromptCacheRetention, ProviderError, ProviderToolCapabilities, ProviderToolTransport,
        TokenUsage,
    },
};
use chrono::{Local, SecondsFormat};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

use super::{
    actor::CodexRuntimeActor,
    continuation_context::ContinuationContext,
    local_tools::{
        LocalToolResult, agent_identity_after_local_tools, local_tool_artifact_reference_item,
        local_tool_result_action_item, local_tool_result_continuation_input,
        local_tool_task_reference_item,
    },
    model_context::{
        AgentIdentityContext, ModelContextState, RuntimeEnvironmentContext, ToolVisibilityContext,
    },
    model_context_ledger::{ModelContextSyncRequest, sync_model_context},
    model_tools::{ModelTools, build_model_tools},
    progress::{
        ContinuationProgressTracker, DeterministicProgressStop, MAX_PROVIDER_TOOL_CONTINUATIONS,
    },
    progress_audit::{
        ProgressAuditDecision, ProgressAuditError, build_no_tools_finalization_prompt,
    },
    tool_lifecycle::{LocalToolCall, local_tool_calls},
    transcript_persistence::{
        assistant_response_stream_id, assistant_stream_id, handle_provider_stream_event,
        send_conversation_item,
    },
    turn_timing::TurnTiming,
};
use crate::daemon::{
    agent_name_tool::is_update_own_name_tool,
    agent_onboarding::AgentPromptIdentity,
    memory::{HUMAN_MEMORY_SCOPE_ID, context::ConversationMemoryContext},
    prompts::{
        build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
    },
    task_tool::is_task_delegate_tool,
};

const MEMORY_OBSERVATION_CONTEXT_ITEM_LIMIT: usize = 4;
const MEMORY_OBSERVATION_CONTEXT_CHAR_LIMIT: usize = 2_000;

#[derive(Debug)]
struct TurnCompletionSignal(Option<oneshot::Sender<()>>);

impl TurnCompletionSignal {
    fn new() -> (Self, oneshot::Receiver<()>) {
        let (sender, receiver) = oneshot::channel();
        (Self(Some(sender)), receiver)
    }
}

impl Drop for TurnCompletionSignal {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::daemon::runtime) struct MultipleChoiceSelectionInput {
    pub(in crate::daemon::runtime) prompt_item_id: String,
    pub(in crate::daemon::runtime) selection_mode: MultipleChoiceSelectionMode,
    pub(in crate::daemon::runtime) selected_options: Vec<MultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
struct MultipleChoicePromptPayload {
    prompt: String,
    selection_mode: MultipleChoiceSelectionMode,
    options: Vec<MultipleChoiceOption>,
}

#[derive(Debug, Clone)]
enum UserTurnInput {
    Text(String),
    MultipleChoiceSelection(MultipleChoiceSelectionInput),
}

impl UserTurnInput {
    fn model_input(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::MultipleChoiceSelection(selection) => render_multiple_choice_selection(selection),
        }
    }

    fn input_chars(&self) -> usize {
        self.model_input().chars().count()
    }
}

fn render_multiple_choice_selection(selection: &MultipleChoiceSelectionInput) -> String {
    let selected = selection
        .selected_options
        .iter()
        .map(|option| format!("{}={}", option.id, option.label))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "user selected multiple_choice options for {}: {}",
        selection.prompt_item_id, selected
    )
}

fn prompt_cache_retention_for(
    tool_capabilities: ProviderToolCapabilities,
) -> Option<PromptCacheRetention> {
    tool_capabilities
        .prompt_cache_retention
        .then_some(PromptCacheRetention::TwentyFourHours)
}

fn prompt_cache_options_for(
    tool_capabilities: ProviderToolCapabilities,
) -> Option<PromptCacheOptions> {
    tool_capabilities
        .prompt_cache_options
        .then_some(PromptCacheOptions {
            mode: PromptCacheMode::Explicit,
            ..PromptCacheOptions::default()
        })
}

fn prompt_cache_breakpoints_for(
    input: &GenerateInput,
    tool_capabilities: ProviderToolCapabilities,
) -> Vec<usize> {
    if !tool_capabilities.prompt_cache_breakpoints {
        return Vec::new();
    }
    let messages = match input {
        GenerateInput::Messages(messages) => messages.iter().collect::<Vec<_>>(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                crate::provider::GenerateInputItem::Message(message) => Some(message),
                crate::provider::GenerateInputItem::Reasoning(_)
                | crate::provider::GenerateInputItem::ToolCall(_)
                | crate::provider::GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::Text(_) | GenerateInput::NativeToolResults(_) => Vec::new(),
    };
    let mut breakpoints = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            (message.role == crate::GenerateMessageRole::Developer).then_some(index)
        })
        .rev()
        .take(4)
        .collect::<Vec<_>>();
    breakpoints.reverse();
    breakpoints
}

pub(super) fn current_runtime_environment(cwd: Option<&str>) -> RuntimeEnvironmentContext {
    let now = Local::now();
    let timezone = std::env::var("TZ")
        .ok()
        .filter(|timezone| !timezone.trim().is_empty())
        .unwrap_or_else(|| format!("UTC{}", now.offset()));
    RuntimeEnvironmentContext::new(
        now.format("%Y-%m-%d").to_string(),
        now.to_rfc3339_opts(SecondsFormat::Secs, false),
        timezone,
        cwd.map(str::to_string),
    )
}

fn model_context_state(
    agent_identity: &AgentPromptIdentity,
    runtime_environment: RuntimeEnvironmentContext,
    model_tools: &ModelTools,
    tools_enabled: bool,
) -> ModelContextState {
    let mut catalog_rows = if tools_enabled {
        model_tools.prompt_rows.clone()
    } else {
        Vec::new()
    };
    catalog_rows.extend(model_tools.unavailable_rows.iter().cloned());
    ModelContextState::new(
        AgentIdentityContext::from(agent_identity),
        runtime_environment,
        ToolVisibilityContext::new(
            if tools_enabled {
                model_tools.transport
            } else {
                ProviderToolTransport::None
            },
            if tools_enabled {
                model_tools
                    .callable_tool_names()
                    .into_iter()
                    .map(|name| name.as_str().to_string())
                    .collect()
            } else {
                Vec::new()
            },
            catalog_rows,
        ),
    )
}

pub(super) fn mcp_health_status_label(status: crate::McpServerHealthStatus) -> &'static str {
    match status {
        crate::McpServerHealthStatus::Unknown => "unknown",
        crate::McpServerHealthStatus::Healthy => "healthy",
        crate::McpServerHealthStatus::Unavailable => "unavailable",
    }
}

pub(super) fn mcp_auth_status_label(status: crate::McpServerAuthStatus) -> &'static str {
    match status {
        crate::McpServerAuthStatus::None => "none",
        crate::McpServerAuthStatus::NeedsAuth => "needs_auth",
        crate::McpServerAuthStatus::Authenticated => "authenticated",
        crate::McpServerAuthStatus::Unavailable => "unavailable",
    }
}

fn build_memory_observation_add_request(
    conversation_id: &str,
    turn_id: &str,
    user_item_id: &str,
    user_text: &str,
    assistant_context: Vec<String>,
) -> Option<crate::MnemosyneAddMemoryRequest> {
    let source_observation = user_text.trim();
    if source_observation.is_empty() {
        return None;
    }

    let mut messages = assistant_context
        .into_iter()
        .filter_map(|content| {
            let content = content.trim();
            (!content.is_empty()).then(|| crate::MnemosyneMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
            })
        })
        .collect::<Vec<_>>();
    messages.push(crate::MnemosyneMessage {
        role: "user".to_string(),
        content: source_observation.to_string(),
    });

    Some(crate::MnemosyneAddMemoryRequest {
        messages,
        user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
        agent_id: Some("agent:local".to_string()),
        run_id: Some(conversation_id.to_string()),
        metadata: json!({
            "noemaConversationId": conversation_id,
            "turnId": turn_id,
            "userItemId": user_item_id,
            "sourceKind": "user_message",
            "sourceObservation": source_observation,
        }),
    })
}

fn bound_memory_observation_assistant_context(messages: Vec<String>) -> Vec<String> {
    let recent = messages
        .into_iter()
        .rev()
        .take(MEMORY_OBSERVATION_CONTEXT_ITEM_LIMIT)
        .collect::<Vec<_>>();
    let mut remaining = MEMORY_OBSERVATION_CONTEXT_CHAR_LIMIT;
    let mut bounded = Vec::new();

    for message in recent.into_iter().rev() {
        let message = message.trim();
        if message.is_empty() || remaining == 0 {
            continue;
        }
        let content = message.chars().take(remaining).collect::<String>();
        remaining = remaining.saturating_sub(content.chars().count());
        bounded.push(content);
    }

    bounded
}

impl CodexRuntimeActor {
    fn log_runtime_invariant(
        &self,
        message: impl Into<String>,
        context: serde_json::Value,
        raw: serde_json::Value,
    ) {
        let message = message.into();
        self.system_errors.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
                .with_context(context)
                .with_error_chain([message])
                .with_raw(raw),
        );
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn start_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection = super::conversation_state::provider_selection_for_conversation(
            &self.store,
            &self.default_provider_kind,
        )
        .await?;
        let new_conversation = crate::NewConversation::local_chat_for_provider(
            &selection.provider_kind,
            selection.model.clone(),
            cwd.clone(),
        );
        let durable_conversation = self.store.create_conversation(new_conversation).await?;
        let conversation_id = durable_conversation.conversation_id;
        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    pub(in crate::daemon) async fn start_primary_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let selection = super::conversation_state::provider_selection_for_conversation(
            &self.store,
            &self.default_provider_kind,
        )
        .await?;
        let durable_conversation = self
            .store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                &selection.provider_kind,
                selection.model.clone(),
                cwd.clone(),
            )
            .await?;
        let conversation_id = durable_conversation.conversation_id;

        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        self.ensure_initial_name_onboarding_message(&conversation_id)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    async fn ensure_initial_name_onboarding_message(
        &mut self,
        conversation_id: &str,
    ) -> Result<(), DaemonError> {
        let agent_identity = self
            .agent_identity_for_conversation(conversation_id)
            .await?;
        if agent_identity.display_name.is_some() {
            return Ok(());
        }
        let replay = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        if !replay.is_empty() {
            return Ok(());
        }

        let conversation = self
            .conversations
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.to_string(),
                trigger_item_id: None,
                metadata: json!({
                    "turn_index": turn_index,
                    "source": "agent_onboarding",
                }),
            })
            .await?;
        let instructions = build_initial_name_onboarding_system_prompt(
            conversation_id,
            turn_index,
            conversation.cwd.as_deref(),
            &agent_identity,
        );
        let provider = self.provider_for_kind(&conversation.provider_kind)?;
        let tool_capabilities = provider.tool_capabilities(conversation.model.as_deref());
        let response = match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.to_string()),
                    model: conversation.model.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        reasoning_effort: conversation.reasoning_effort,
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        ..GenerateOptions::default()
                    },
                    tools: Vec::new(),
                    tool_choice: Default::default(),
                    parallel_tool_calls: false,
                },
                &mut |_| {},
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.store.fail_conversation_turn(&turn.turn_id).await?;
                self.conversations.remove(conversation_id);
                return Err(error.into());
            }
        };
        let raw_provider_response = json!({
            "provider": &response.provider,
            "model": &response.model,
            "response_id": &response.response_id,
            "usage": response.usage.as_ref().map(|usage| json!({
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "total_tokens": usage.total_tokens,
                "cached_input_tokens": usage.cached_input_tokens,
            })),
            "responses": &response.responses,
            "tool_calls": &response.tool_calls,
            "response_status": response.response_status,
        });
        let persisted_count = self
            .persist_agent_initiated_provider_response(
                conversation_id,
                &turn.turn_id,
                turn_index,
                response,
            )
            .await?;
        if persisted_count == 0 {
            self.store.fail_conversation_turn(&turn.turn_id).await?;
            self.conversations.remove(conversation_id);
            self.log_runtime_invariant(
                "initial onboarding response did not include assistant text",
                json!({
                    "conversation_id": conversation_id,
                    "turn_id": turn.turn_id,
                    "turn_index": turn_index,
                    "provider_kind": conversation.provider_kind,
                    "model": conversation.model,
                }),
                json!({
                    "persisted_count": persisted_count,
                    "provider_response": raw_provider_response,
                }),
            );
            return Err(DaemonError::Protocol(
                "initial onboarding response did not include assistant text".to_string(),
            ));
        }
        self.store.complete_conversation_turn(&turn.turn_id).await?;
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }
        Ok(())
    }

    pub(in crate::daemon) async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        self.turn_with_user_input(
            conversation_id,
            UserTurnInput::Text(input),
            item_tx,
            client_message_id,
        )
        .await
    }

    async fn turn_with_multiple_choice_selection(
        &mut self,
        conversation_id: String,
        selection: MultipleChoiceSelectionInput,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        self.turn_with_user_input(
            conversation_id,
            UserTurnInput::MultipleChoiceSelection(selection),
            item_tx,
            client_message_id,
        )
        .await
    }

    pub(in crate::daemon) async fn select_multiple_choice(
        &mut self,
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let selection = self
            .validate_multiple_choice_selection(
                &conversation_id,
                prompt_item_id,
                selected_option_ids,
            )
            .await?;
        self.turn_with_multiple_choice_selection(
            conversation_id,
            selection,
            item_tx,
            client_message_id,
        )
        .await
    }

    async fn validate_multiple_choice_selection(
        &self,
        conversation_id: &str,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
    ) -> Result<MultipleChoiceSelectionInput, DaemonError> {
        let items = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        if items.iter().any(|item| {
            item.kind == ConversationItemKind::MultipleChoiceSelection
                && item
                    .payload_json
                    .get("prompt_item_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(prompt_item_id.as_str())
        }) {
            return Err(DaemonError::Protocol(
                "multiple-choice prompt already has a selection".to_string(),
            ));
        }
        let prompt_item = items
            .iter()
            .find(|item| item.item_id == prompt_item_id)
            .ok_or_else(|| {
                DaemonError::Protocol(format!(
                    "multiple-choice prompt not found: {prompt_item_id}"
                ))
            })?;
        if prompt_item.kind != ConversationItemKind::MultipleChoicePrompt {
            return Err(DaemonError::Protocol(format!(
                "conversation item is not a multiple-choice prompt: {prompt_item_id}"
            )));
        }
        let payload: MultipleChoicePromptPayload =
            serde_json::from_value(prompt_item.payload_json.clone()).map_err(|source| {
                DaemonError::Protocol(format!(
                    "invalid multiple-choice prompt payload for {prompt_item_id}: {source}"
                ))
            })?;
        let _ = &payload.prompt;
        match payload.selection_mode {
            MultipleChoiceSelectionMode::PickOne if selected_option_ids.len() != 1 => {
                return Err(DaemonError::Protocol(
                    "pick_one multiple-choice selection must include exactly one option"
                        .to_string(),
                ));
            }
            MultipleChoiceSelectionMode::PickMany if selected_option_ids.is_empty() => {
                return Err(DaemonError::Protocol(
                    "pick_many multiple-choice selection must include at least one option"
                        .to_string(),
                ));
            }
            MultipleChoiceSelectionMode::PickOne | MultipleChoiceSelectionMode::PickMany => {}
        }

        let selected_ids = selected_option_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if selected_ids.len() != selected_option_ids.len() {
            return Err(DaemonError::Protocol(
                "multiple-choice selected option ids must be unique".to_string(),
            ));
        }
        let option_ids = payload
            .options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<HashSet<_>>();
        if let Some(invalid_id) = selected_option_ids
            .iter()
            .find(|id| !option_ids.contains(id.as_str()))
        {
            return Err(DaemonError::Protocol(format!(
                "multiple-choice option id is not in the prompt: {invalid_id}"
            )));
        }
        let selected_options = payload
            .options
            .into_iter()
            .filter(|option| selected_ids.contains(option.id.as_str()))
            .collect::<Vec<_>>();

        Ok(MultipleChoiceSelectionInput {
            prompt_item_id,
            selection_mode: payload.selection_mode,
            selected_options,
        })
    }

    async fn turn_with_user_input(
        &mut self,
        conversation_id: String,
        user_input: UserTurnInput,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let input = user_input.model_input();
        let pre_turn_started_at = std::time::Instant::now();
        let conversation = self
            .hydrate_active_conversation(&conversation_id, None)
            .await?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({ "turn_index": turn_index }),
            })
            .await?;
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
                "provider_kind": conversation.provider_kind,
                "model": conversation.model,
            }),
        );
        let provider = self.provider_for_kind(&conversation.provider_kind)?;
        let tool_capabilities = provider.tool_capabilities(conversation.model.as_deref());
        let response_continuation = provider.response_continuation(conversation.model.as_deref());
        let agent_identity = self
            .agent_identity_for_conversation(&conversation_id)
            .await?;
        let tools_started_at = std::time::Instant::now();
        let model_tools = self.model_tools(true, tool_capabilities).await?;
        let continuation_model_tools = self.model_tools(false, tool_capabilities).await?;
        timing.mark(
            "runtime_model_tools_ready",
            json!({
                "duration_ms": tools_started_at.elapsed().as_millis(),
                "tool_catalog_count": model_tools.tools.len(),
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
            provider_kind: &conversation.provider_kind,
            model_profile: conversation.model.as_deref(),
            state: &model_context_state,
        })
        .await?;
        timing.mark(
            "runtime_model_context_synced",
            json!({ "update_count": model_context_updates.len() }),
        );
        let prompt_started_at = std::time::Instant::now();
        let mut planned_context =
            super::prompt_context::plan_prompt_context(super::prompt_context::PromptPlanRequest {
                store: &self.store,
                provider: provider.as_ref(),
                conversation_id: &conversation_id,
                provider_kind: &conversation.provider_kind,
                model_profile: conversation.model.as_deref(),
                current_input: &input,
            })
            .await?;
        let reconciled_model_context_updates = self
            .reconcile_model_context_plan(
                provider.as_ref(),
                &conversation_id,
                &turn.turn_id,
                &conversation.provider_kind,
                conversation.model.as_deref(),
                &model_context_state,
                &input,
                &mut planned_context,
            )
            .await?;
        timing.mark(
            "runtime_prompt_context_planned",
            json!({
                "duration_ms": prompt_started_at.elapsed().as_millis(),
                "fits": planned_context.fits,
                "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                "budget_input_tokens": planned_context.budget.available_input_tokens(),
                "budget_output_reserve_tokens": planned_context.budget.output_reserve_tokens(),
                "reconciled_model_context_updates": reconciled_model_context_updates,
            }),
        );
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
                UserTurnInput::MultipleChoiceSelection(selection) => (
                    ConversationItemKind::MultipleChoiceSelection,
                    Some(selection.prompt_item_id.clone()),
                    Some(
                        selection
                            .selected_options
                            .iter()
                            .map(|option| option.label.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    json!({
                        "prompt_item_id": selection.prompt_item_id,
                        "selection_mode": selection.selection_mode,
                        "selected_options": selection.selected_options,
                    }),
                    TurnTranscriptItem::MultipleChoiceSelection {
                        prompt_item_id: selection.prompt_item_id.clone(),
                        selection_mode: selection.selection_mode,
                        selected_options: selection.selected_options.clone(),
                    },
                ),
            };
        let user_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id,
                kind: user_kind,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: user_content_text,
                payload_json: user_payload,
                metadata: user_metadata.clone(),
            })
            .await?;
        let user_item_id = user_item.item_id.clone();
        let user_sequence_index = user_item.sequence_index;
        send_conversation_item(&item_tx, user_item, user_metadata, transcript_item);
        timing.mark("runtime_user_item_persisted", json!({}));
        let _memory_observation_turn_guard = if let UserTurnInput::Text(text) = &user_input {
            let (turn_finished, await_turn_finished) = TurnCompletionSignal::new();
            self.enqueue_user_message_memory_observation(
                &conversation_id,
                &turn.turn_id,
                &user_item_id,
                user_sequence_index,
                text,
                await_turn_finished,
            )
            .await;
            Some(turn_finished)
        } else {
            None
        };
        if super::context_compaction::should_compact_foreground(&planned_context) {
            let compaction_started_at = std::time::Instant::now();
            timing.mark("runtime_foreground_compaction_started", json!({}));
            let compaction_result = super::context_compaction::compact_context_with_retry(
                super::context_compaction::CompactionRequest {
                    store: &self.store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &conversation.provider_kind,
                    model_profile: conversation.model.as_deref(),
                    reasoning_effort: conversation.reasoning_effort,
                    budget: planned_context.budget,
                    mode: super::context_compaction::CompactionMode::Foreground,
                },
            )
            .await;
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
            timing.mark(
                "runtime_foreground_compaction_finished",
                json!({
                    "duration_ms": compaction_started_at.elapsed().as_millis(),
                }),
            );
            sync_model_context(ModelContextSyncRequest {
                store: &self.store,
                conversation_id: &conversation_id,
                turn_id: &turn.turn_id,
                provider_kind: &conversation.provider_kind,
                model_profile: conversation.model.as_deref(),
                state: &model_context_state,
            })
            .await?;
            let prompt_replan_started_at = std::time::Instant::now();
            planned_context = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider: provider.as_ref(),
                    conversation_id: &conversation_id,
                    provider_kind: &conversation.provider_kind,
                    model_profile: conversation.model.as_deref(),
                    current_input: &input,
                },
            )
            .await?;
            self.reconcile_model_context_plan(
                provider.as_ref(),
                &conversation_id,
                &turn.turn_id,
                &conversation.provider_kind,
                conversation.model.as_deref(),
                &model_context_state,
                &input,
                &mut planned_context,
            )
            .await?;
            timing.mark(
                "runtime_prompt_context_replanned_after_compaction",
                json!({
                    "duration_ms": prompt_replan_started_at.elapsed().as_millis(),
                    "fits": planned_context.fits,
                    "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                }),
            );
            if !planned_context.fits {
                let smaller_compaction_started_at = std::time::Instant::now();
                timing.mark("runtime_smaller_compaction_started", json!({}));
                let smaller_compaction = super::context_compaction::compact_active_summary_smaller(
                    super::context_compaction::CompactionRequest {
                        store: &self.store,
                        provider: provider.as_ref(),
                        conversation_id: &conversation_id,
                        provider_kind: &conversation.provider_kind,
                        model_profile: conversation.model.as_deref(),
                        reasoning_effort: conversation.reasoning_effort,
                        budget: planned_context.budget,
                        mode: super::context_compaction::CompactionMode::Foreground,
                    },
                )
                .await;
                if let Err(error) = smaller_compaction {
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
                timing.mark(
                    "runtime_smaller_compaction_finished",
                    json!({
                        "duration_ms": smaller_compaction_started_at.elapsed().as_millis(),
                    }),
                );
                sync_model_context(ModelContextSyncRequest {
                    store: &self.store,
                    conversation_id: &conversation_id,
                    turn_id: &turn.turn_id,
                    provider_kind: &conversation.provider_kind,
                    model_profile: conversation.model.as_deref(),
                    state: &model_context_state,
                })
                .await?;
                let prompt_replan_started_at = std::time::Instant::now();
                planned_context = super::prompt_context::plan_prompt_context(
                    super::prompt_context::PromptPlanRequest {
                        store: &self.store,
                        provider: provider.as_ref(),
                        conversation_id: &conversation_id,
                        provider_kind: &conversation.provider_kind,
                        model_profile: conversation.model.as_deref(),
                        current_input: &input,
                    },
                )
                .await?;
                self.reconcile_model_context_plan(
                    provider.as_ref(),
                    &conversation_id,
                    &turn.turn_id,
                    &conversation.provider_kind,
                    conversation.model.as_deref(),
                    &model_context_state,
                    &input,
                    &mut planned_context,
                )
                .await?;
                timing.mark(
                    "runtime_prompt_context_replanned_after_smaller_compaction",
                    json!({
                        "duration_ms": prompt_replan_started_at.elapsed().as_millis(),
                        "fits": planned_context.fits,
                        "estimated_prompt_tokens": planned_context.estimated_input_tokens,
                    }),
                );
            }
            if !planned_context.fits {
                let error = ProviderError::InvalidRequest {
                    message: "context could not be compacted enough for the selected model"
                        .to_string(),
                };
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
                GenerateStreamEvent::AssistantTextDelta { .. } => handle_provider_stream_event(
                    event,
                    &item_tx,
                    &initial_event_context,
                    &initial_stream_id,
                    0,
                ),
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
        let initial_provider_input = planned_context.input.clone();
        let initial_prompt_cache_breakpoints =
            prompt_cache_breakpoints_for(&planned_context.input, tool_capabilities);
        let initial_provider_tools = model_tools.provider_tools();
        let (initial_tools, initial_tool_choice) = if tool_capabilities.allowed_tools
            && model_tools.transport == ProviderToolTransport::Native
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
                    model: conversation.model.clone(),
                    input: planned_context.input,
                    instructions: Some(planned_context.instructions),
                    options: GenerateOptions {
                        max_output_tokens: planned_context.budget.output_reserve_tokens(),
                        reasoning_effort: conversation.reasoning_effort,
                        require_noema_response: true,
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        prompt_cache_options: prompt_cache_options_for(tool_capabilities),
                        prompt_cache_breakpoints: initial_prompt_cache_breakpoints,
                        store_response: response_continuation.store_response(),
                        ..GenerateOptions::default()
                    },
                    tools: initial_tools,
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
                        "response_status": format!("{:?}", response.response_status),
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
                            cwd: conversation.cwd.clone(),
                            provider_kind: conversation.provider_kind.clone(),
                            model: conversation.model.clone(),
                            reasoning_effort: conversation.reasoning_effort,
                            initial_stream_id: initial_stream_id.clone(),
                            response,
                            agent_identity,
                            runtime_environment,
                            tool_capabilities,
                            provider_tool_catalog: model_tools.provider_tools(),
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
                    provider_kind: conversation.provider_kind.clone(),
                    model_profile: conversation.model.clone(),
                    reasoning_effort: conversation.reasoning_effort,
                    next_turn_index: turn_index.saturating_add(1),
                });

                timing.mark("runtime_turn_ok", json!({}));
                Ok(())
            }
            Err(error) => {
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
                    | ProviderError::HttpFailure { .. }
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

    async fn persist_successful_provider_turn(
        &mut self,
        turn: SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), DaemonError> {
        let persist_started_at = std::time::Instant::now();
        timing.mark("runtime_persist_successful_turn_started", json!({}));
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
                rejected_mixed_delegation_result(call)
            } else {
                self.execute_local_tool(&turn, &turn.agent_identity, call)
                    .await
            };
            timing.mark(
                "runtime_tool_execution_completed",
                json!({
                    "phase": "initial",
                    "tool_name": result.name(),
                    "duration_ms": tool_started_at.elapsed().as_millis(),
                    "success": result.success(),
                    "requires_provider_continuation": result.requires_provider_continuation(),
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
                    "tool_name": result.name(),
                    "success": result.success(),
                }),
            );
            next_output_index += 1;
            local_tool_results.push(result);
        }
        if initial_batch_kind.contains_delegation() {
            self.persist_task_delegation_receipt(
                &turn,
                &turn.initial_stream_id,
                initial_response_count,
                &local_tool_results,
                item_tx,
            )
            .await?;
        }
        let mut task_handoff = initial_batch_kind.is_terminal_handoff();
        let mut all_local_tool_results = local_tool_results.clone();
        let mut progress_tracker = ContinuationProgressTracker::new(&turn.user_input);
        progress_tracker.observe_results(&local_tool_results);
        let mut continuation_context =
            ContinuationContext::from_provider_input(turn.initial_provider_input.clone());
        continuation_context.append_response(&turn.response);
        continuation_context.append_results(&local_tool_results);
        continuation_context.finish_round();

        let mut continuation_tool_results = if task_handoff {
            Vec::new()
        } else {
            local_tool_results
                .iter()
                .filter(|result| result.requires_provider_continuation())
                .cloned()
                .collect::<Vec<_>>()
        };
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
                    &turn,
                    &all_local_tool_results,
                    next_output_index,
                    reason,
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
                                    &turn,
                                    &all_local_tool_results,
                                    next_output_index,
                                    "progress audit requested final answer",
                                    item_tx,
                                    timing,
                                )
                                .await?;
                                continuation_tool_results.clear();
                                break;
                            }
                            ProgressAuditDecision::AskHuman | ProgressAuditDecision::Checkpoint => {
                                self.persist_progress_pause_message(
                                    &turn,
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
                            &turn,
                            &all_local_tool_results,
                            next_output_index,
                            &message,
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
            let provider = self.provider_for_kind(&turn.provider_kind)?;
            let response_continuation = provider.response_continuation(turn.model.as_deref());
            let continuation_context_state = model_context_state(
                &continuation_agent_identity,
                turn.runtime_environment.clone(),
                &turn.continuation_model_tools,
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
            let continuation_input = continuation_context.next_provider_input(
                turn.tool_capabilities.native_tool_results,
                response_continuation,
            );
            let continuation_instructions = build_local_tool_result_continuation_system_prompt();
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
            let mut on_continuation_event = |event| {
                if matches!(
                    &event,
                    GenerateStreamEvent::ToolCallStarted { name, .. }
                        if is_update_own_name_tool(name)
                ) {
                    return;
                }
                if !continuation_stream_seen_for_event.swap(true, Ordering::Relaxed) {
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
                    GenerateStreamEvent::AssistantTextDelta { .. } => {
                        handle_provider_stream_event(
                            event,
                            item_tx,
                            &continuation_event_context,
                            &continuation_stream_id,
                            continuation_output_base,
                        );
                    }
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
                        turn.continuation_model_tools.tools.len()
                    },
                }),
            );
            let continuation_provider_started_at = std::time::Instant::now();
            let chained = continuation_input.previous_response_id.is_some();
            let continuation_prompt_cache_breakpoints =
                prompt_cache_breakpoints_for(&continuation_input.input, turn.tool_capabilities);
            let (continuation_tools, continuation_tool_choice) =
                if turn.tool_capabilities.allowed_tools
                    && turn.continuation_model_tools.transport == ProviderToolTransport::Native
                {
                    (
                        turn.provider_tool_catalog.clone(),
                        if task_handoff {
                            NoemaToolChoice::None
                        } else {
                            turn.continuation_model_tools
                                .allowed_tool_choice(NoemaAllowedToolsMode::Auto)
                        },
                    )
                } else {
                    (
                        if task_handoff {
                            Vec::new()
                        } else {
                            turn.continuation_model_tools.provider_tools()
                        },
                        NoemaToolChoice::Auto,
                    )
                };
            let continuation_request = GenerateRequest {
                conversation_id: Some(turn.conversation_id.clone()),
                model: turn.model.clone(),
                input: continuation_input.input,
                instructions: Some(continuation_instructions.clone()),
                options: GenerateOptions {
                    require_noema_response: true,
                    prompt_cache_retention: prompt_cache_retention_for(turn.tool_capabilities),
                    prompt_cache_options: prompt_cache_options_for(turn.tool_capabilities),
                    prompt_cache_breakpoints: continuation_prompt_cache_breakpoints,
                    reasoning_effort: turn.reasoning_effort,
                    previous_response_id: continuation_input.previous_response_id,
                    store_response: response_continuation.store_response(),
                    ..GenerateOptions::default()
                },
                tools: continuation_tools.clone(),
                tool_choice: continuation_tool_choice.clone(),
                parallel_tool_calls: !task_handoff
                    && turn.continuation_model_tools.transport == ProviderToolTransport::Native
                    && turn.continuation_model_tools.has_callable_tools()
                    && turn.tool_capabilities.parallel_tool_calls,
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
                                require_noema_response: true,
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
                            tool_choice: continuation_tool_choice,
                            parallel_tool_calls: !task_handoff
                                && turn.continuation_model_tools.transport
                                    == ProviderToolTransport::Native
                                && turn.continuation_model_tools.has_callable_tools()
                                && turn.tool_capabilities.parallel_tool_calls,
                        },
                        &mut on_continuation_event,
                    )
                    .await;
            }
            let continuation_response = continuation_result?;
            timing.mark(
                "provider_continuation_response_completed",
                json!({
                    "continuation_step": continuation_step,
                    "duration_ms": continuation_provider_started_at.elapsed().as_millis(),
                    "response_count": continuation_response.responses.len(),
                    "tool_call_count": continuation_response.tool_calls.len(),
                    "response_status": format!("{:?}", continuation_response.response_status),
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
                &continuation_response.reasoning_items,
            )
            .await?;
            let raw_continuation_batch_kind = if !task_handoff
                && continuation_response.response_status == GenerateResponseStatus::NeedsTools
            {
                ForegroundToolBatchKind::for_calls(&local_tool_calls(
                    &continuation_response.tool_calls,
                ))
            } else {
                ForegroundToolBatchKind::Standard
            };
            let continuation_tool_call_items = continuation_response
                .tool_calls
                .iter()
                .filter(|call| !is_disallowed_continuation_tool_call(call))
                .cloned()
                .collect::<Vec<_>>();
            continuation_context.append_response(&GenerateResponse {
                responses: continuation_response.responses.clone(),
                tool_calls: continuation_tool_call_items.clone(),
                reasoning_items: continuation_response.reasoning_items.clone(),
                response_status: continuation_response.response_status,
                provider: continuation_response.provider.clone(),
                model: continuation_response.model.clone(),
                response_id: continuation_response.response_id.clone(),
                usage: continuation_response.usage.clone(),
            });
            let continuation_response_count = continuation_response.responses.len();
            let continuation_tool_calls = if !task_handoff
                && continuation_response.response_status == GenerateResponseStatus::NeedsTools
            {
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
                        continuation_output_base,
                    );
                }
            }
            let continuation_phase_has_tools = !continuation_tool_calls.is_empty();
            if !continuation_batch_kind.contains_delegation() {
                for (offset, response_item) in
                    continuation_response.responses.iter().cloned().enumerate()
                {
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
            next_output_index += continuation_response_count + continuation_tool_calls.len();

            let continuation_turn = SuccessfulProviderTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                user_input: turn.user_input.clone(),
                task_id: turn.task_id.clone(),
                task_run_id: turn.task_run_id.clone(),
                cwd: turn.cwd.clone(),
                provider_kind: turn.provider_kind.clone(),
                model: turn.model.clone(),
                reasoning_effort: turn.reasoning_effort,
                initial_stream_id: continuation_stream_id.clone(),
                response: GenerateResponse {
                    responses: continuation_response.responses.clone(),
                    tool_calls: continuation_tool_call_items,
                    reasoning_items: continuation_response.reasoning_items.clone(),
                    response_status: continuation_response.response_status,
                    provider: continuation_response.provider.clone(),
                    model: continuation_response.model.clone(),
                    response_id: continuation_response.response_id.clone(),
                    usage: continuation_response.usage.clone(),
                },
                agent_identity: continuation_agent_identity,
                runtime_environment: turn.runtime_environment.clone(),
                tool_capabilities: turn.tool_capabilities,
                provider_tool_catalog: turn.provider_tool_catalog.clone(),
                continuation_model_tools: turn.continuation_model_tools.clone(),
                initial_provider_input: turn.initial_provider_input.clone(),
            };
            let mut local_tool_results = Vec::new();
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
            for call in &continuation_tool_calls {
                timing.mark(
                    "runtime_tool_call_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                self.persist_provider_tool_call_started(
                    &local_action_turn,
                    continuation_output_base + continuation_response_count + call.output_index,
                    call,
                    item_tx,
                )
                .await?;
                let tool_started_at = std::time::Instant::now();
                timing.mark(
                    "runtime_tool_execution_started",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": call.name,
                        "output_index": call.output_index,
                    }),
                );
                let result = if continuation_batch_kind == ForegroundToolBatchKind::MixedDelegation
                {
                    rejected_mixed_delegation_result(call)
                } else {
                    self.execute_local_tool(
                        &continuation_turn,
                        &continuation_turn.agent_identity,
                        call,
                    )
                    .await
                };
                timing.mark(
                    "runtime_tool_execution_completed",
                    json!({
                        "phase": "continuation",
                        "continuation_step": continuation_step,
                        "tool_name": result.name(),
                        "duration_ms": tool_started_at.elapsed().as_millis(),
                        "success": result.success(),
                        "requires_provider_continuation": result.requires_provider_continuation(),
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
                        turn_index: continuation_turn.turn_index,
                        conversation_id: continuation_turn.conversation_id.clone(),
                        turn_id: continuation_turn.turn_id.clone(),
                        user_item_id: continuation_turn.user_item_id.clone(),
                        assistant_item_id: None,
                    };
                    self.persist_and_send_turn_item(&context, item, item_tx)
                        .await?;
                }
                if let Some(item) = local_tool_task_reference_item(&result) {
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
                        "tool_name": result.name(),
                        "success": result.success(),
                    }),
                );
                next_output_index += 1;
                local_tool_results.push(result);
            }
            if continuation_batch_kind.contains_delegation() {
                self.persist_task_delegation_receipt(
                    &continuation_turn,
                    &continuation_stream_id,
                    continuation_response_count,
                    &local_tool_results,
                    item_tx,
                )
                .await?;
            }
            task_handoff = continuation_batch_kind.is_terminal_handoff();
            continuation_tool_results = if task_handoff {
                Vec::new()
            } else {
                local_tool_results
                    .iter()
                    .filter(|result| result.requires_provider_continuation())
                    .cloned()
                    .collect::<Vec<_>>()
            };
            progress_tracker.observe_results(&local_tool_results);
            continuation_context.append_results(&local_tool_results);
            continuation_context.finish_round();
            all_local_tool_results.extend(local_tool_results.clone());
        }
        if !continuation_tool_results.is_empty() {
            self.finalize_after_progress_stop(
                &turn,
                &all_local_tool_results,
                next_output_index,
                "maximum provider tool continuations reached",
                item_tx,
                timing,
            )
            .await?;
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

    async fn enqueue_user_message_memory_observation(
        &self,
        conversation_id: &str,
        turn_id: &str,
        user_item_id: &str,
        user_sequence_index: i64,
        user_text: &str,
        await_turn_finished: oneshot::Receiver<()>,
    ) {
        let assistant_context = self
            .memory_observation_assistant_context(
                conversation_id,
                user_item_id,
                user_sequence_index,
            )
            .await;
        let Some(request) = build_memory_observation_add_request(
            conversation_id,
            turn_id,
            user_item_id,
            user_text,
            assistant_context,
        ) else {
            return;
        };
        let Some(client) = self.memory_client() else {
            self.log_runtime_invariant(
                "memory observation could not be submitted",
                json!({
                    "conversation_id": conversation_id,
                    "turn_id": turn_id,
                    "source_item_id": user_item_id,
                }),
                json!({
                    "error_code": "service_unavailable",
                    "error": "memory service is unavailable",
                }),
            );
            return;
        };
        let system_errors = self.system_errors.clone();
        let error_context = json!({
            "conversation_id": conversation_id,
            "turn_id": turn_id,
            "source_item_id": user_item_id,
        });
        self.tasks.spawn(async move {
            let _ = await_turn_finished.await;
            if let Err(error) = client.add_memory(request).await {
                let message = "memory observation submit failed".to_string();
                system_errors.try_append(
                    SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
                        .with_context(error_context)
                        .with_error_chain([message])
                        .with_raw(json!({
                            "error_code": error.sanitized_code(),
                            "error": error.sanitized_message(),
                        })),
                );
            }
        });
    }

    async fn memory_observation_assistant_context(
        &self,
        conversation_id: &str,
        user_item_id: &str,
        user_sequence_index: i64,
    ) -> Vec<String> {
        let Ok(items) = self
            .store
            .list_recent_conversation_items_for_context(conversation_id, 40)
            .await
        else {
            return Vec::new();
        };

        let mut messages = Vec::new();
        for item in items.iter().rev() {
            if item.item_id == user_item_id || item.sequence_index >= user_sequence_index {
                continue;
            }
            match item.kind {
                ConversationItemKind::UserText => break,
                ConversationItemKind::AssistantText => {
                    if let Some(text) = item.content_text.as_deref() {
                        messages.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        messages.reverse();
        bound_memory_observation_assistant_context(messages)
    }

    pub(in crate::daemon) async fn update_conversation_agent_status(
        &mut self,
        conversation_id: &str,
        status: PersistedAgentStatus,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.store
            .update_conversation_agent_status(conversation_id, status)
            .await?;
        let _ = item_tx.send(TurnStreamEvent::AgentStatusChanged {
            conversation_id: conversation_id.to_string(),
            status: AgentStatus::from(status),
        });
        Ok(())
    }

    async fn finalize_after_progress_stop(
        &mut self,
        turn: &SuccessfulProviderTurn,
        results: &[LocalToolResult],
        index: usize,
        reason: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
        timing: &TurnTiming,
    ) -> Result<(), DaemonError> {
        let result_refs = results.iter().collect::<Vec<_>>();
        let provider = self.provider_for_kind(&turn.provider_kind)?;
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
    ) -> Result<(), DaemonError> {
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
                author: ActorRef::agent("agent:primary"),
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
    ) -> Result<(), DaemonError> {
        let delegation_results = results
            .iter()
            .filter(|result| is_task_delegate_tool(result.name()))
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
            "delegation_success_count": delegation_results.iter().filter(|result| result.success()).count(),
            "delegation_failure_count": delegation_results.iter().filter(|result| !result.success()).count(),
        });
        let assistant_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: turn.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: Some(turn.user_item_id.clone()),
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
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

    async fn agent_identity_for_conversation(
        &self,
        _conversation_id: &str,
    ) -> Result<AgentPromptIdentity, DaemonError> {
        let agent_id = "agent:primary".to_string();
        let agent = self
            .store
            .get_agent(&agent_id)
            .await?
            .ok_or_else(|| DaemonError::Protocol(format!("unknown agent id: {agent_id}")))?;
        Ok(AgentPromptIdentity {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
        })
    }

    async fn model_tools(
        &self,
        include_agent_name_tool: bool,
        capabilities: ProviderToolCapabilities,
    ) -> Result<ModelTools, DaemonError> {
        build_model_tools(&self.store, include_agent_name_tool, capabilities)
            .await
            .map_err(|error| DaemonError::Protocol(error.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    async fn reconcile_model_context_plan(
        &self,
        provider: &dyn super::handle::RuntimeModelProvider,
        conversation_id: &str,
        turn_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
        state: &ModelContextState,
        current_input: &str,
        planned_context: &mut super::prompt_context::PlannedPromptContext,
    ) -> Result<usize, DaemonError> {
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
        let Ok(provider) = self.provider_for_kind(&schedule.provider_kind) else {
            return;
        };
        self.tasks.spawn(async move {
            let BackgroundContextCompactionSchedule {
                conversation_id,
                provider_kind,
                model_profile,
                reasoning_effort,
                next_turn_index,
            } = schedule;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            match store.next_conversation_turn_index(&conversation_id).await {
                Ok(current_next_turn_index) if current_next_turn_index == next_turn_index => {}
                Ok(_) | Err(_) => return,
            }
            let plan = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &store,
                    provider: provider.as_ref(),
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
                    provider: provider.as_ref(),
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

fn provider_stream_event_fields(event: &GenerateStreamEvent) -> serde_json::Value {
    match event {
        GenerateStreamEvent::AssistantTextDelta {
            response_index,
            delta,
        } => json!({
            "stream_event": "assistant_text_delta",
            "response_index": response_index,
            "delta_chars": delta.chars().count(),
        }),
        GenerateStreamEvent::ToolCallStarted { output_index, name } => json!({
            "stream_event": "tool_call_started",
            "output_index": output_index,
            "tool_name": name,
        }),
    }
}

fn continuation_provider_stream_event_fields(
    continuation_step: usize,
    event: &GenerateStreamEvent,
) -> serde_json::Value {
    let mut fields = provider_stream_event_fields(event);
    fields["continuation_step"] = json!(continuation_step);
    fields
}

#[derive(Debug)]
struct BackgroundContextCompactionSchedule {
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    reasoning_effort: Option<crate::provider::ReasoningEffort>,
    next_turn_index: u64,
}

#[derive(Debug)]
pub(in crate::daemon) struct SuccessfulProviderTurn {
    pub(in crate::daemon) conversation_id: String,
    pub(in crate::daemon) turn_id: String,
    pub(in crate::daemon) turn_index: u64,
    pub(in crate::daemon) user_item_id: String,
    pub(in crate::daemon) user_input: String,
    pub(in crate::daemon) task_id: Option<String>,
    pub(in crate::daemon) task_run_id: Option<String>,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) provider_kind: String,
    pub(in crate::daemon) model: Option<String>,
    pub(in crate::daemon) reasoning_effort: Option<crate::provider::ReasoningEffort>,
    pub(in crate::daemon) initial_stream_id: String,
    pub(in crate::daemon) response: GenerateResponse,
    pub(in crate::daemon) agent_identity: AgentPromptIdentity,
    pub(in crate::daemon) runtime_environment: RuntimeEnvironmentContext,
    pub(in crate::daemon) tool_capabilities: ProviderToolCapabilities,
    pub(in crate::daemon) provider_tool_catalog: Vec<crate::provider::NoemaToolSpec>,
    pub(in crate::daemon) continuation_model_tools: ModelTools,
    pub(in crate::daemon) initial_provider_input: GenerateInput,
}

#[derive(Debug, Default)]
pub(in crate::daemon) struct ProviderAssistantResponse {
    pub(in crate::daemon) item_id: Option<String>,
    pub(in crate::daemon) text: String,
}

impl ProviderAssistantResponse {
    pub(in crate::daemon) fn push_text(&mut self, text: &str) {
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(text);
    }
}

pub(in crate::daemon) struct ProviderActionTurn {
    pub(in crate::daemon) conversation_id: String,
    pub(in crate::daemon) turn_id: String,
    pub(in crate::daemon) turn_index: u64,
    pub(in crate::daemon) user_item_id: String,
    pub(in crate::daemon) provider: String,
    pub(in crate::daemon) model: String,
    pub(in crate::daemon) response_phase: &'static str,
    pub(in crate::daemon) usage: Option<TokenUsage>,
    pub(in crate::daemon) stream_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::daemon) struct ProviderResponsePosition {
    pub(in crate::daemon) response_index: usize,
    pub(in crate::daemon) output_index: Option<usize>,
}

pub(in crate::daemon) struct ProviderActionOutput {
    pub(in crate::daemon) index: usize,
    pub(in crate::daemon) kind: ConversationItemKind,
    pub(in crate::daemon) status: ConversationItemStatus,
    pub(in crate::daemon) action_kind: &'static str,
    pub(in crate::daemon) title: String,
    pub(in crate::daemon) summary: Option<String>,
    pub(in crate::daemon) payload: serde_json::Value,
    pub(in crate::daemon) display: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForegroundToolBatchKind {
    Standard,
    Delegation,
    MixedDelegation,
}

impl ForegroundToolBatchKind {
    fn for_calls(calls: &[LocalToolCall]) -> Self {
        let delegation_count = calls
            .iter()
            .filter(|call| is_task_delegate_tool(&call.name))
            .count();
        match delegation_count {
            0 => Self::Standard,
            count if count == calls.len() => Self::Delegation,
            _ => Self::MixedDelegation,
        }
    }

    const fn contains_delegation(self) -> bool {
        matches!(self, Self::Delegation | Self::MixedDelegation)
    }

    const fn is_terminal_handoff(self) -> bool {
        matches!(self, Self::Delegation)
    }
}

fn rejected_mixed_delegation_result(call: &LocalToolCall) -> LocalToolResult {
    LocalToolResult::Gateway {
        call_id: call.call_id.clone(),
        provider_call_id: call.provider_call_id.clone(),
        provider_name: call.provider_name.clone(),
        name: call.name.clone(),
        arguments: call.payload.clone(),
        result: GatewayToolResult {
            success: false,
            payload: json!({
                "error": "task_delegate_mixed_tool_batch",
                "message": "task.delegate must be called without other tool kinds in the same provider response",
            }),
            requires_provider_continuation: true,
        },
    }
}

fn task_delegation_receipt(results: &[LocalToolResult]) -> String {
    let (successful, failed) = results.iter().fold((0usize, 0usize), |counts, result| {
        if result.success() {
            (counts.0 + 1, counts.1)
        } else {
            (counts.0, counts.1 + 1)
        }
    });
    match (successful, failed) {
        (1, 0) => "Started 1 background task.".to_string(),
        (successful, 0) => format!("Started {successful} background tasks."),
        (0, 1) => "1 task delegation failed.".to_string(),
        (0, failed) => format!("{failed} task delegations failed."),
        (1, 1) => "Started 1 background task; 1 delegation failed.".to_string(),
        (1, failed) => {
            format!("Started 1 background task; {failed} delegations failed.")
        }
        (successful, 1) => {
            format!("Started {successful} background tasks; 1 delegation failed.")
        }
        (successful, failed) => {
            format!("Started {successful} background tasks; {failed} delegations failed.")
        }
    }
}

fn is_disallowed_continuation_tool_call(call: &GenerateToolCall) -> bool {
    is_update_own_name_tool(&call.name)
}
