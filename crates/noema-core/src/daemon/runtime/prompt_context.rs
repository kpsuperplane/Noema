use crate::{
    ConversationContextSummaryRecord, ConversationItemRecord, NoemaStore,
    daemon::{
        agent_onboarding::AgentPromptIdentity,
        prompts::{build_structured_turn_system_prompt, render_recent_transcript_for_prompt},
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
    let rendered_context = render_prompt_context(active_summary.as_ref(), &transcript_items);
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
    );
    let metadata = request.provider.context_metadata(request.model_profile);
    let budget = ContextBudget::from_metadata(metadata);
    let estimated_input_tokens = count_tokens_or_estimate(
        request.provider,
        Some(&instructions),
        request.current_input,
        request.model_profile,
    )
    .await;
    let fits = budget.fits(estimated_input_tokens);
    Ok(PlannedPromptContext {
        context: request.context,
        instructions,
        estimated_input_tokens,
        budget,
        fits,
    })
}

async fn count_tokens_or_estimate(
    provider: &dyn RuntimeModelProvider,
    instructions: Option<&str>,
    input: &str,
    model_profile: Option<&str>,
) -> u32 {
    match provider
        .count_tokens(instructions, input, model_profile)
        .await
    {
        Ok(Some(tokens)) => tokens,
        Ok(None) | Err(_) => {
            instructions.map_or(0, estimate_text_tokens) + estimate_text_tokens(input)
        }
    }
}

fn render_prompt_context(
    summary: Option<&ConversationContextSummaryRecord>,
    transcript_items: &[ConversationItemRecord],
) -> String {
    let transcript = render_recent_transcript_for_prompt(transcript_items);
    match summary {
        Some(summary) => format!(
            "Compacted conversation context:\n{}\n\nRecent transcript after compacted checkpoint:\n{}",
            summary.summary_text, transcript
        ),
        None => transcript,
    }
}
