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

pub(super) async fn load_prompt_context(
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
        .list_conversation_items_after_sequence_for_context(conversation_id, after_sequence, 40)
        .await?;
    let rendered_context = render_prompt_context(active_summary.as_ref(), &transcript_items);
    Ok(PromptContext {
        active_summary,
        transcript_items,
        rendered_context,
    })
}

pub(super) async fn plan_prompt_context(
    store: &NoemaStore,
    provider: &dyn RuntimeModelProvider,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
    turn_index: u64,
    cwd: Option<&str>,
    agent_identity: &AgentPromptIdentity,
    rendered_tools: &str,
    current_input: &str,
) -> Result<PlannedPromptContext, DaemonError> {
    let context = load_prompt_context(store, conversation_id, provider_kind, model_profile).await?;
    plan_loaded_prompt_context(
        provider,
        conversation_id,
        model_profile,
        turn_index,
        cwd,
        agent_identity,
        rendered_tools,
        current_input,
        context,
    )
    .await
}

pub(super) async fn plan_loaded_prompt_context(
    provider: &dyn RuntimeModelProvider,
    conversation_id: &str,
    model_profile: Option<&str>,
    turn_index: u64,
    cwd: Option<&str>,
    agent_identity: &AgentPromptIdentity,
    rendered_tools: &str,
    current_input: &str,
    context: PromptContext,
) -> Result<PlannedPromptContext, DaemonError> {
    let instructions = build_structured_turn_system_prompt(
        conversation_id,
        turn_index,
        cwd,
        &context.rendered_context,
        agent_identity,
        rendered_tools,
    );
    let metadata = provider.context_metadata(model_profile);
    let budget = ContextBudget::from_metadata(metadata);
    let estimated_input_tokens = provider
        .count_tokens(Some(&instructions), current_input, model_profile)
        .await?
        .unwrap_or_else(|| {
            estimate_text_tokens(&instructions) + estimate_text_tokens(current_input)
        });
    let fits = budget.fits(estimated_input_tokens);
    Ok(PlannedPromptContext {
        context,
        instructions,
        estimated_input_tokens,
        budget,
        fits,
    })
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
