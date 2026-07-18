use noema_conversations::{
    ActorRef, AgentStatus as PersistedAgentStatus, ConversationItemKind, ConversationItemStatus,
    NewConversationItem, NewConversationTurn, ReplayMode,
};

use chrono::{Local, SecondsFormat};
use noema_home::SystemErrorEvent;
use noema_memory::HUMAN_MEMORY_SCOPE_ID;
use noema_providers::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
    GenerateStreamEvent, GenerateToolCall, MultipleChoiceOption, MultipleChoiceSelectionMode,
    NoemaAllowedToolsMode, NoemaToolChoice, PromptCacheMode, PromptCacheOptions,
    PromptCacheRetention, ProviderError, ProviderRouteLease, ProviderToolCapabilities,
    ProviderToolTransport, TokenUsage,
};
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
    actor::RuntimeActor,
    continuation_context::ContinuationContext,
    local_tools::{
        LocalToolKind, LocalToolResult, agent_identity_after_local_tools,
        local_tool_artifact_reference_item, local_tool_result_action_item,
        local_tool_result_continuation_input, local_tool_task_reference_item,
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
use crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT;
use crate::daemon::{
    agent_name_tool::is_update_own_name_tool,
    agent_onboarding::AgentPromptIdentity,
    memory::context::ConversationMemoryContext,
    prompts::{
        build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt,
    },
    protocol::{RuntimeError, StartedConversation, TurnStreamEvent, TurnTranscriptItem},
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
                noema_providers::GenerateInputItem::Message(message) => Some(message),
                noema_providers::GenerateInputItem::Reasoning(_)
                | noema_providers::GenerateInputItem::ToolCall(_)
                | noema_providers::GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::Text(_) | GenerateInput::NativeToolResults(_) => Vec::new(),
    };
    let mut breakpoints = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            (message.role == noema_providers::GenerateMessageRole::Developer).then_some(index)
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

fn build_memory_observation_add_request(
    conversation_id: &str,
    turn_id: &str,
    user_item_id: &str,
    user_text: &str,
    assistant_context: Vec<String>,
) -> Option<noema_memory::AddMemoryRequest> {
    let source_observation = user_text.trim();
    if source_observation.is_empty() {
        return None;
    }

    let mut messages = assistant_context
        .into_iter()
        .filter_map(|content| {
            let content = content.trim();
            (!content.is_empty()).then(|| noema_memory::MemoryMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
            })
        })
        .collect::<Vec<_>>();
    messages.push(noema_memory::MemoryMessage {
        role: "user".to_string(),
        content: source_observation.to_string(),
    });

    Some(noema_memory::AddMemoryRequest {
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

#[cfg(test)]
mod memory_observation_tests {
    use super::build_memory_observation_add_request;
    use serde_json::json;

    #[test]
    fn observation_keeps_assistant_context_and_current_user_as_the_source() {
        let request = build_memory_observation_add_request(
            "conversation:1",
            "turn:2",
            "item:3",
            "  cars  ",
            vec![" question one ".to_string(), "question two".to_string()],
        )
        .expect("observation");

        assert_eq!(
            serde_json::to_value(request.messages).unwrap(),
            json!([
                {"role": "assistant", "content": "question one"},
                {"role": "assistant", "content": "question two"},
                {"role": "user", "content": "cars"}
            ])
        );
        assert_eq!(request.metadata["sourceObservation"], "cars");
        assert_eq!(request.metadata["userItemId"], "item:3");
    }
}

include!("turn/startup.rs");
include!("turn/provider_request.rs");
include!("turn/persistence.rs");
include!("turn/continuations.rs");
include!("turn/memory_and_status.rs");
include!("turn/finalization.rs");

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

struct ForegroundContinuationState {
    next_output_index: usize,
    task_handoff: bool,
    all_local_tool_results: Vec<LocalToolResult>,
    progress_tracker: ContinuationProgressTracker,
    continuation_context: ContinuationContext,
    continuation_tool_results: Vec<LocalToolResult>,
}

#[derive(Debug)]
struct BackgroundContextCompactionSchedule {
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    reasoning_effort: Option<noema_providers::ReasoningEffort>,
    provider_route: Arc<ProviderRouteLease>,
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
    pub(in crate::daemon) reasoning_effort: Option<noema_providers::ReasoningEffort>,
    pub(in crate::daemon) provider_route: Arc<ProviderRouteLease>,
    pub(in crate::daemon) initial_stream_id: String,
    pub(in crate::daemon) response: GenerateResponse,
    pub(in crate::daemon) agent_identity: AgentPromptIdentity,
    pub(in crate::daemon) runtime_environment: RuntimeEnvironmentContext,
    pub(in crate::daemon) tool_capabilities: ProviderToolCapabilities,
    pub(in crate::daemon) initial_model_tools: ModelTools,
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

fn rejected_mixed_delegation_result(
    call: &LocalToolCall,
    bindings: &noema_capabilities::CapabilityCatalogSnapshot,
) -> LocalToolResult {
    let failure = noema_capabilities::CapabilityDispatchFailure::from_snapshot(
        bindings,
        &call.name,
        &call.payload,
        noema_capabilities::CapabilityError::Denied,
    );
    LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        json!({
            "error": "task_delegate_mixed_tool_batch",
            "message": "task.delegate must be called without other tool kinds in the same provider response",
        }),
        true,
    )
    .with_persisted(failure.persisted)
}

fn task_delegation_receipt(results: &[LocalToolResult]) -> String {
    let (successful, failed) = results.iter().fold((0usize, 0usize), |counts, result| {
        if result.success {
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

#[cfg(test)]
mod delegation_batch_tests {
    use super::*;

    #[test]
    fn batch_policy_and_receipts_cover_homogeneous_and_mixed_delegation() {
        let call = |name: &str| LocalToolCall {
            output_index: 0,
            call_id: None,
            provider_call_id: None,
            provider_name: None,
            name: name.to_string(),
            payload: json!({}),
        };
        assert_eq!(
            ForegroundToolBatchKind::for_calls(&[call("task.delegate"), call("task.delegate")]),
            ForegroundToolBatchKind::Delegation
        );
        assert_eq!(
            ForegroundToolBatchKind::for_calls(&[call("task.delegate"), call("web.search")]),
            ForegroundToolBatchKind::MixedDelegation
        );

        let result = |success| {
            LocalToolResult::from_call(
                &call("task.delegate"),
                LocalToolKind::Gateway,
                success,
                json!({}),
                false,
            )
        };
        assert_eq!(
            task_delegation_receipt(&[result(true), result(true), result(false)]),
            "Started 2 background tasks; 1 delegation failed."
        );
    }
}
