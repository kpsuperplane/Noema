use crate::{
    ConversationContextSummaryRecord, ConversationItemKind, ConversationItemRecord, GenerateInput,
    GenerateMessage, GenerateMessageRole, NoemaStore,
    daemon::{
        agent_onboarding::AgentPromptIdentity,
        prompts::{PromptToolExposure, build_structured_turn_system_prompt},
        protocol::DaemonError,
    },
};

use super::{
    context_window::{ContextBudget, estimate_text_tokens},
    handle::RuntimeModelProvider,
};

/// Model-visible context selected for one provider turn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PromptContext {
    pub(super) active_summary: Option<ConversationContextSummaryRecord>,
    pub(super) transcript_items: Vec<ConversationItemRecord>,
    pub(super) rendered_context: String,
}

/// Prompt context plus context-window fit decision.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedPromptContext {
    pub(super) context: PromptContext,
    pub(super) instructions: String,
    pub(super) input: GenerateInput,
    pub(super) estimated_input_tokens: u32,
    pub(super) budget: ContextBudget,
    pub(super) fits: bool,
}

pub(super) struct PromptPlanRequest<'a> {
    pub(super) store: &'a NoemaStore,
    pub(super) provider: &'a dyn RuntimeModelProvider,
    pub(super) conversation_id: &'a str,
    pub(super) provider_kind: &'a str,
    pub(super) model_profile: Option<&'a str>,
    pub(super) turn_index: u64,
    pub(super) cwd: Option<&'a str>,
    pub(super) agent_identity: &'a AgentPromptIdentity,
    pub(super) rendered_tools: &'a str,
    pub(super) native_tools_available: bool,
    pub(super) legacy_builtin_envelope_tools: &'a [String],
    pub(super) current_input: &'a str,
}

struct LoadedPromptPlanRequest<'a> {
    provider: &'a dyn RuntimeModelProvider,
    conversation_id: &'a str,
    model_profile: Option<&'a str>,
    turn_index: u64,
    cwd: Option<&'a str>,
    agent_identity: &'a AgentPromptIdentity,
    rendered_tools: &'a str,
    native_tools_available: bool,
    legacy_builtin_envelope_tools: &'a [String],
    current_input: &'a str,
    context: PromptContext,
}

async fn load_prompt_context(
    store: &NoemaStore,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
) -> Result<PromptContext, crate::StoreError> {
    let active_summary = store
        .latest_active_context_summary(conversation_id, provider_kind, model_profile)
        .await?;
    let after_sequence = active_summary
        .as_ref()
        .map_or(0, |summary| summary.covered_item_end_sequence);
    let transcript_items = store
        .list_all_conversation_items_after_sequence_for_context(conversation_id, after_sequence)
        .await?;
    let rendered_context = render_prompt_context(active_summary.as_ref());
    Ok(PromptContext {
        active_summary,
        transcript_items,
        rendered_context,
    })
}

pub(super) async fn plan_prompt_context(
    request: PromptPlanRequest<'_>,
) -> Result<PlannedPromptContext, DaemonError> {
    let context = load_prompt_context(
        request.store,
        request.conversation_id,
        request.provider_kind,
        request.model_profile,
    )
    .await?;
    plan_loaded_prompt_context(LoadedPromptPlanRequest {
        provider: request.provider,
        conversation_id: request.conversation_id,
        model_profile: request.model_profile,
        turn_index: request.turn_index,
        cwd: request.cwd,
        agent_identity: request.agent_identity,
        rendered_tools: request.rendered_tools,
        native_tools_available: request.native_tools_available,
        legacy_builtin_envelope_tools: request.legacy_builtin_envelope_tools,
        current_input: request.current_input,
        context,
    })
    .await
}

async fn plan_loaded_prompt_context(
    request: LoadedPromptPlanRequest<'_>,
) -> Result<PlannedPromptContext, DaemonError> {
    let instructions = build_structured_turn_system_prompt(
        request.conversation_id,
        request.turn_index,
        request.cwd,
        &request.context.rendered_context,
        request.agent_identity,
        request.rendered_tools,
        PromptToolExposure {
            native_tools_available: request.native_tools_available,
            legacy_builtin_envelope_tools: request.legacy_builtin_envelope_tools,
        },
    );
    let input = build_turn_input(&request.context.transcript_items, request.current_input);
    let metadata = request.provider.context_metadata(request.model_profile);
    let budget = ContextBudget::from_metadata(metadata);
    let estimated_input_tokens = count_tokens_or_estimate(
        request.provider,
        Some(&instructions),
        &input,
        request.model_profile,
    )
    .await;
    let fits = budget.fits(estimated_input_tokens);
    Ok(PlannedPromptContext {
        context: request.context,
        instructions,
        input,
        estimated_input_tokens,
        budget,
        fits,
    })
}

async fn count_tokens_or_estimate(
    provider: &dyn RuntimeModelProvider,
    instructions: Option<&str>,
    input: &GenerateInput,
    model_profile: Option<&str>,
) -> u32 {
    let rendered_input = input.render_for_token_count();
    match provider
        .count_tokens(instructions, &rendered_input, model_profile)
        .await
    {
        Ok(Some(tokens)) => tokens,
        Ok(None) | Err(_) => {
            instructions.map_or(0, estimate_text_tokens) + estimate_text_tokens(&rendered_input)
        }
    }
}

fn build_turn_input(
    transcript_items: &[ConversationItemRecord],
    current_input: &str,
) -> GenerateInput {
    let mut messages = transcript_items
        .iter()
        .filter_map(message_from_transcript_item)
        .collect::<Vec<_>>();
    if !current_input.trim().is_empty() {
        messages.push(GenerateMessage {
            role: GenerateMessageRole::User,
            content: current_input.to_string(),
        });
    }
    GenerateInput::Messages(messages)
}

fn message_from_transcript_item(item: &ConversationItemRecord) -> Option<GenerateMessage> {
    let role = match item.kind {
        ConversationItemKind::UserText => GenerateMessageRole::User,
        ConversationItemKind::AssistantText => GenerateMessageRole::Assistant,
        ConversationItemKind::Activity
        | ConversationItemKind::A2uiCard
        | ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult
        | ConversationItemKind::ErrorNotice => return None,
    };
    let content = item.content_text.as_deref()?.trim();
    if content.is_empty() {
        return None;
    }
    Some(GenerateMessage {
        role,
        content: content.to_string(),
    })
}

fn render_prompt_context(summary: Option<&ConversationContextSummaryRecord>) -> String {
    match summary {
        Some(summary) => format!(
            "Compacted conversation context:\n{}\n\nRecent transcript after compacted checkpoint: sent as role-tagged provider input messages",
            summary.summary_text
        ),
        None => "none".to_string(),
    }
}
