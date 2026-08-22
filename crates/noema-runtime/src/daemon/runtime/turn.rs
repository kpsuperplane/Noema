use noema_conversations::{
    ActorRef, AgentStatus as PersistedAgentStatus, ConversationItemKind, ConversationItemStatus,
    ConversationTurnStatus, NewConversationItem, NewConversationTurn, ReplayMode,
};

use jiff::{Timestamp, tz::TimeZone};
use noema_home::SystemErrorEvent;
use noema_providers::{
    GenerateHostedWebSearch, GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse,
    GenerateStreamEvent, GenerationPriority, MultipleChoiceOption, MultipleChoiceSelectionMode,
    NoemaAllowedToolsMode, NoemaToolChoice, PromptCacheMode, PromptCacheOptions,
    PromptCacheRetention, ProviderError, ProviderRouteLease, ProviderToolCapabilities,
    ProviderToolTransport, TokenUsage,
};
use noema_store::{
    RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc;

use super::{
    actor::RuntimeActor,
    citation_markers::CitationSourceRegistry,
    context_window::{ContextAdmission, RequestContext, admit_request, hard_overflow_error},
    continuation_context::ContinuationContext,
    interaction_lifecycle::resolved_interaction_tool_result_item,
    local_tools::{
        LocalToolKind, LocalToolResult, agent_identity_after_local_tools,
        local_tool_artifact_reference_item, local_tool_result_action_item,
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
    runtime_debug::{ProviderDebugTimeline, RuntimeDebugSpan},
    tool_lifecycle::{LocalToolCall, local_tool_calls, single_tool_display_description},
    transcript_persistence::{
        assistant_stream_id, handle_provider_stream_event, send_conversation_item,
    },
    turn_timing::TurnTiming,
};
use crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT;
use crate::daemon::{
    agent_onboarding::AgentPromptIdentity,
    memory::context::ConversationMemoryContext,
    prompts::{
        build_initial_name_onboarding_system_prompt,
        build_local_tool_result_continuation_system_prompt,
    },
    protocol::{
        RuntimeError, StartedConversation, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem,
    },
    task_tool::{TASK_DELEGATE_TOOL, is_task_delegate_tool},
};

#[derive(Debug, Clone)]
pub(in crate::daemon::runtime) struct MultipleChoiceSelectionInput {
    pub(in crate::daemon::runtime) prompt_item_id: String,
    pub(in crate::daemon::runtime) interaction_id: String,
    pub(in crate::daemon::runtime) interaction_revision: u64,
    pub(in crate::daemon::runtime) selection_mode: MultipleChoiceSelectionMode,
    pub(in crate::daemon::runtime) selected_options: Vec<MultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
struct MultipleChoicePromptPayload {
    prompt: String,
    selection_mode: MultipleChoiceSelectionMode,
    options: Vec<MultipleChoiceOption>,
    interaction_id: Option<String>,
    interaction_revision: Option<u64>,
}

#[derive(Debug, Clone)]
enum UserTurnInput {
    Text(String),
    HumanInterventionContinuation {
        intervention_id: String,
        trigger_item_id: String,
    },
}

impl UserTurnInput {
    fn model_input(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::HumanInterventionContinuation { .. } => String::new(),
        }
    }

    fn input_chars(&self) -> usize {
        self.model_input().chars().count()
    }

    fn human_intervention(&self) -> Option<(&str, &str)> {
        match self {
            Self::HumanInterventionContinuation {
                intervention_id,
                trigger_item_id,
            } => Some((intervention_id, trigger_item_id)),
            Self::Text(_) => None,
        }
    }
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
                noema_providers::GenerateInputItem::AssistantText(_)
                | noema_providers::GenerateInputItem::Reasoning(_)
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

async fn admit_foreground_context(
    context: &mut ContinuationContext,
    turn: &SuccessfulProviderTurn,
    provider: &dyn noema_providers::ProviderOperations,
    instructions: &str,
    tools: &[noema_providers::ProviderTool],
    hosted_web_search: bool,
) -> Result<(), RuntimeError> {
    context
        .compact_to_fit(
            provider,
            turn.model.as_deref(),
            turn.tool_capabilities.native_tool_results,
            instructions,
            tools,
            hosted_web_search,
            None,
            turn.reasoning_effort,
            GenerationPriority::Foreground,
            &turn.user_input,
        )
        .await
        .map(|_| ())
        .map_err(RuntimeError::Provider)
}

pub(super) fn current_runtime_environment(cwd: Option<&str>) -> RuntimeEnvironmentContext {
    current_runtime_environment_with_timezone(cwd, None)
}

pub(crate) fn current_runtime_environment_with_timezone(
    cwd: Option<&str>,
    client_time_zone: Option<&str>,
) -> RuntimeEnvironmentContext {
    runtime_environment_at(Timestamp::now(), cwd, client_time_zone)
}

fn runtime_environment_at(
    now: Timestamp,
    cwd: Option<&str>,
    client_time_zone: Option<&str>,
) -> RuntimeEnvironmentContext {
    let requested_zone = client_time_zone.and_then(|name| {
        TimeZone::get(name)
            .ok()
            .map(|zone| (zone, name.to_string()))
    });
    let (zone, timezone) = requested_zone.unwrap_or_else(|| {
        let zone = TimeZone::system();
        let name = zone.iana_name().unwrap_or("UTC").to_string();
        (zone, name)
    });
    let now = now.to_zoned(zone);
    RuntimeEnvironmentContext::new(
        now.strftime("%Y-%m-%d").to_string(),
        now.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        timezone,
        cwd.map(str::to_string),
    )
}

#[cfg(test)]
mod runtime_environment_tests {
    use super::*;

    #[test]
    fn client_zone_controls_the_rendered_date_and_time() {
        let now = "2026-08-04T04:37:23Z".parse().expect("UTC instant");
        let context = runtime_environment_at(now, None, Some("America/Los_Angeles"));

        assert_eq!(context.current_date, "2026-08-03");
        assert_eq!(context.current_time, "2026-08-03T21:37:23-07:00");
        assert_eq!(context.timezone, "America/Los_Angeles");
    }
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
                model_tools.exposed_tool_names()
            } else {
                Vec::new()
            },
            if tools_enabled {
                model_tools.source_tool_names()
            } else {
                Vec::new()
            },
            tools_enabled && model_tools.hosted_web_search(),
            catalog_rows,
        ),
    )
}

include!("turn/startup.rs");
include!("turn/provider_request.rs");
include!("turn/persistence.rs");
include!("turn/continuations.rs");
include!("turn/progress_finalization.rs");
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
        GenerateStreamEvent::ToolCallStarted {
            output_index, name, ..
        } => json!({
            "stream_event": "tool_call_started",
            "output_index": output_index,
            "tool_name": name,
        }),
        GenerateStreamEvent::HostedWebSearchStarted { output_index, .. } => json!({
            "stream_event": "hosted_web_search_started",
            "output_index": output_index,
        }),
        GenerateStreamEvent::ProviderTiming {
            milestone,
            output_index,
        } => json!({
            "stream_event": "provider_timing",
            "milestone": format!("{milestone:?}"),
            "output_index": output_index,
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
    waiting_for_interaction: bool,
    citation_sources: CitationSourceRegistry,
}

#[derive(Debug)]
struct BackgroundContextCompactionSchedule {
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    reasoning_effort: Option<noema_providers::ReasoningEffort>,
    fast_mode: bool,
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
    pub(in crate::daemon) task_run_fence: Option<noema_store::WorkRunFence>,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) provider_kind: String,
    pub(in crate::daemon) model: Option<String>,
    pub(in crate::daemon) reasoning_effort: Option<noema_providers::ReasoningEffort>,
    pub(in crate::daemon) fast_mode: bool,
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

fn provider_output_span(
    response_count: usize,
    tool_call_count: usize,
    hosted_web_searches: &[GenerateHostedWebSearch],
) -> usize {
    let persisted_output_count = response_count + tool_call_count;
    hosted_web_searches
        .iter()
        .map(|search| search.output_index + 1)
        .max()
        .unwrap_or_default()
        .max(persisted_output_count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForegroundToolBatchKind {
    Standard,
    Delegation,
    MixedDelegation,
}

const FOREGROUND_DELEGATION_NUDGE_ROUNDS: usize = 3;

fn should_nudge_task_delegation(completed_tool_rounds: usize, available: bool) -> bool {
    available && completed_tool_rounds >= FOREGROUND_DELEGATION_NUDGE_ROUNDS
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
    let payload = json!({
        "error": "task_delegate_mixed_tool_batch",
        "message": "task.delegate must be called without other tool kinds in the same provider response",
    });
    let persisted = bindings
        .resolve(&call.name)
        .map(|binding| noema_capabilities::PersistedCapabilityPayload {
            arguments: binding.persist_arguments(&call.payload),
            output: binding.persist_output(&payload),
        })
        .unwrap_or_else(noema_capabilities::PersistedCapabilityPayload::omitted);
    LocalToolResult::from_call(call, LocalToolKind::Gateway, false, payload, true)
        .with_persisted(persisted)
}

#[cfg(test)]
mod delegation_batch_tests {
    use super::*;

    #[test]
    fn batch_policy_covers_homogeneous_and_mixed_delegation() {
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
    }

    #[test]
    fn delegation_nudge_starts_after_three_tool_rounds_when_available() {
        assert!(!should_nudge_task_delegation(2, true));
        assert!(should_nudge_task_delegation(3, true));
        assert!(should_nudge_task_delegation(4, true));
        assert!(!should_nudge_task_delegation(3, false));
    }
}

#[cfg(test)]
mod provider_output_span_tests {
    use super::*;

    #[test]
    fn hosted_search_positions_advance_the_next_provider_output_base() {
        let searches = [GenerateHostedWebSearch {
            output_index: 2,
            id: None,
            tool_name: "web.search".to_string(),
            arguments: json!({}),
            result: json!({}),
            status: "completed".to_string(),
            sources: Vec::new(),
        }];

        assert_eq!(provider_output_span(1, 0, &searches), 3);
        assert_eq!(provider_output_span(2, 2, &searches), 4);
    }
}

#[cfg(test)]
#[path = "turn/uncertain_outcome_tests.rs"]
mod uncertain_outcome_tests;
