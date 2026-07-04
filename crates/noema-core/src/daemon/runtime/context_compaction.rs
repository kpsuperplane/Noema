use crate::{
    ConversationContextSummaryRecord, ConversationContextSummaryStatus, ConversationItemRecord,
    NewConversationContextSummary, NoemaStore,
    daemon::protocol::DaemonError,
    provider::{
        GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseItem,
        GenerateStreamEvent, ProviderError,
    },
};

use super::{
    context_window::{ContextBudget, estimate_text_tokens},
    handle::RuntimeModelProvider,
    prompt_context::PlannedPromptContext,
};

const BACKGROUND_COMPACTION_THRESHOLD_NUMERATOR: u32 = 7;
const BACKGROUND_COMPACTION_THRESHOLD_DENOMINATOR: u32 = 10;
const MIN_RETRY_SUMMARY_TARGET_TOKENS: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompactionMode {
    Foreground,
    Background,
}

#[derive(Debug, Clone)]
pub(super) struct CompactionRequest<'a> {
    pub(super) store: &'a NoemaStore,
    pub(super) provider: &'a dyn RuntimeModelProvider,
    pub(super) conversation_id: &'a str,
    pub(super) provider_kind: &'a str,
    pub(super) model_profile: Option<&'a str>,
    pub(super) budget: ContextBudget,
    pub(super) mode: CompactionMode,
}

pub(super) fn should_compact_foreground(plan: &PlannedPromptContext) -> bool {
    !plan.fits
}

pub(super) fn should_compact_background(plan: &PlannedPromptContext) -> bool {
    let Some(available) = plan.budget.available_input_tokens() else {
        return false;
    };
    if available == 0 {
        return true;
    }
    let threshold = available.saturating_mul(BACKGROUND_COMPACTION_THRESHOLD_NUMERATOR)
        / BACKGROUND_COMPACTION_THRESHOLD_DENOMINATOR;
    plan.estimated_input_tokens >= threshold
}

pub(super) async fn compact_context(
    request: CompactionRequest<'_>,
) -> Result<ConversationContextSummaryRecord, DaemonError> {
    compact_context_with_target(request, None).await
}

pub(super) async fn compact_context_with_retry(
    request: CompactionRequest<'_>,
) -> Result<ConversationContextSummaryRecord, DaemonError> {
    let mut latest_summary = compact_context_chunk_with_retry(request.clone()).await?;
    if request.mode != CompactionMode::Foreground {
        return Ok(latest_summary);
    }

    while load_summary_seed(
        request.store,
        request.conversation_id,
        request.provider_kind,
        request.model_profile,
    )
    .await?
    .is_some()
    {
        latest_summary = compact_context_chunk_with_retry(request.clone()).await?;
    }

    Ok(latest_summary)
}

async fn compact_context_chunk_with_retry(
    request: CompactionRequest<'_>,
) -> Result<ConversationContextSummaryRecord, DaemonError> {
    match compact_context_with_target(request.clone(), None).await {
        Ok(summary) => Ok(summary),
        Err(error) if request.mode == CompactionMode::Foreground => {
            let retry_target = retry_summary_target(request.budget);
            match retry_target {
                Some(target) => compact_context_with_target(request, Some(target)).await,
                None => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn compact_active_summary_smaller(
    request: CompactionRequest<'_>,
) -> Result<ConversationContextSummaryRecord, DaemonError> {
    let Some(active_summary) = request
        .store
        .latest_active_context_summary(
            request.conversation_id,
            request.provider_kind,
            request.model_profile,
        )
        .await?
    else {
        return Err(ProviderError::InvalidRequest {
            message: "context compaction retry requested without an active summary".to_string(),
        }
        .into());
    };
    let target_tokens =
        retry_summary_target(request.budget).unwrap_or(MIN_RETRY_SUMMARY_TARGET_TOKENS);
    let instructions = compaction_instructions(target_tokens);
    let input = format!(
        "Previous compacted context to shorten:\n{}",
        active_summary.summary_text
    );
    let input_token_estimate = count_tokens_or_estimate(
        request.provider,
        Some(&instructions),
        &input,
        request.model_profile,
    )
    .await;
    let response = generate_compaction_summary(
        request.provider,
        request.model_profile,
        instructions,
        input,
        target_tokens,
    )
    .await?;
    let summary_text = parse_compaction_summary(response)?;
    let summary_token_estimate =
        count_tokens_or_estimate(request.provider, None, &summary_text, request.model_profile)
            .await;

    request
        .store
        .insert_conversation_context_summary(NewConversationContextSummary {
            conversation_id: request.conversation_id.to_string(),
            provider_kind: request.provider_kind.to_string(),
            model_profile: request.model_profile.map(str::to_string),
            summary_text,
            covered_item_start_sequence: active_summary.covered_item_start_sequence,
            covered_item_end_sequence: active_summary.covered_item_end_sequence,
            source_item_ids: active_summary.source_item_ids,
            input_token_estimate: u64::from(input_token_estimate),
            summary_token_estimate: u64::from(summary_token_estimate),
            compaction_provider_kind: request.provider_kind.to_string(),
            compaction_model_profile: request.model_profile.map(str::to_string),
            status: ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .map_err(Into::into)
}

pub(super) async fn record_failed_background_compaction(
    store: &NoemaStore,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
    error: &DaemonError,
) -> Result<(), DaemonError> {
    let Some(summary_seed) =
        load_summary_seed(store, conversation_id, provider_kind, model_profile).await?
    else {
        return Ok(());
    };
    store
        .insert_conversation_context_summary(NewConversationContextSummary {
            conversation_id: conversation_id.to_string(),
            provider_kind: provider_kind.to_string(),
            model_profile: model_profile.map(str::to_string),
            summary_text: String::new(),
            covered_item_start_sequence: summary_seed.covered_item_start_sequence,
            covered_item_end_sequence: summary_seed.covered_item_end_sequence,
            source_item_ids: summary_seed.source_item_ids,
            input_token_estimate: 0,
            summary_token_estimate: 0,
            compaction_provider_kind: provider_kind.to_string(),
            compaction_model_profile: model_profile.map(str::to_string),
            status: ConversationContextSummaryStatus::Failed,
            error_code: Some("background_compaction_failed".to_string()),
            error_message: Some(error.to_string()),
        })
        .await?;
    Ok(())
}

async fn compact_context_with_target(
    request: CompactionRequest<'_>,
    summary_target_tokens: Option<u32>,
) -> Result<ConversationContextSummaryRecord, DaemonError> {
    let Some(mut summary_seed) = load_summary_seed(
        request.store,
        request.conversation_id,
        request.provider_kind,
        request.model_profile,
    )
    .await?
    else {
        return Err(ProviderError::InvalidRequest {
            message: "context compaction requested without transcript items to compact".to_string(),
        }
        .into());
    };
    let target_tokens = summary_target_tokens
        .or_else(|| request.budget.compact_summary_target_tokens())
        .unwrap_or(512);
    let instructions = compaction_instructions(target_tokens);
    let input_token_estimate = bound_summary_seed_to_context_budget(
        request.provider,
        request.model_profile,
        request.budget,
        target_tokens,
        &instructions,
        &mut summary_seed,
    )
    .await?;
    let input = render_compaction_input(
        summary_seed.previous_summary.as_ref(),
        &summary_seed.transcript_items,
    );
    let response = generate_compaction_summary(
        request.provider,
        request.model_profile,
        instructions,
        input,
        target_tokens,
    )
    .await?;
    let summary_text = parse_compaction_summary(response)?;
    let summary_token_estimate =
        count_tokens_or_estimate(request.provider, None, &summary_text, request.model_profile)
            .await;

    let summary = request
        .store
        .insert_conversation_context_summary(NewConversationContextSummary {
            conversation_id: request.conversation_id.to_string(),
            provider_kind: request.provider_kind.to_string(),
            model_profile: request.model_profile.map(str::to_string),
            summary_text,
            covered_item_start_sequence: summary_seed.covered_item_start_sequence,
            covered_item_end_sequence: summary_seed.covered_item_end_sequence,
            source_item_ids: summary_seed.source_item_ids,
            input_token_estimate: u64::from(input_token_estimate),
            summary_token_estimate: u64::from(summary_token_estimate),
            compaction_provider_kind: request.provider_kind.to_string(),
            compaction_model_profile: request.model_profile.map(str::to_string),
            status: ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await?;
    Ok(summary)
}

async fn generate_compaction_summary(
    provider: &dyn RuntimeModelProvider,
    model_profile: Option<&str>,
    instructions: String,
    input: String,
    target_tokens: u32,
) -> Result<GenerateResponse, ProviderError> {
    let mut ignore_event = |_: GenerateStreamEvent| {};
    provider
        .generate_streaming(
            GenerateRequest {
                conversation_id: None,
                model: model_profile.map(str::to_string),
                input: GenerateInput::Text(input),
                instructions: Some(instructions),
                options: GenerateOptions {
                    max_output_tokens: Some(target_tokens),
                    require_noema_response: false,
                    ..GenerateOptions::default()
                },
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            },
            &mut ignore_event,
        )
        .await
}

fn parse_compaction_summary(response: GenerateResponse) -> Result<String, ProviderError> {
    let text = response
        .responses
        .into_iter()
        .filter_map(|item| match item {
            GenerateResponseItem::Text { text, .. } => Some(text),
            GenerateResponseItem::Structured { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "context compaction response did not include summary text".to_string(),
        });
    }
    Ok(text)
}

async fn load_summary_seed(
    store: &NoemaStore,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
) -> Result<Option<SummarySeed>, DaemonError> {
    let previous_summary = store
        .latest_active_context_summary(conversation_id, provider_kind, model_profile)
        .await?;
    let after_sequence = previous_summary
        .as_ref()
        .map_or(0, |summary| summary.covered_item_end_sequence);
    let transcript_items = store
        .list_all_conversation_items_after_sequence_for_context(conversation_id, after_sequence)
        .await?;
    if transcript_items.is_empty() {
        return Ok(None);
    }
    let first_sequence = previous_summary
        .as_ref()
        .map_or(transcript_items[0].sequence_index, |summary| {
            summary.covered_item_start_sequence
        });
    let last_sequence = transcript_items
        .last()
        .map_or(first_sequence, |item| item.sequence_index);
    let source_item_ids =
        bounded_combined_source_item_ids(previous_summary.as_ref(), &transcript_items);
    Ok(Some(SummarySeed {
        previous_summary,
        transcript_items,
        covered_item_start_sequence: first_sequence,
        covered_item_end_sequence: last_sequence,
        source_item_ids,
    }))
}

fn render_compaction_input(
    previous_summary: Option<&ConversationContextSummaryRecord>,
    transcript_items: &[ConversationItemRecord],
) -> String {
    let mut sections = Vec::new();
    if let Some(summary) = previous_summary {
        sections.push(format!(
            "Previous compacted context:\n{}",
            summary.summary_text
        ));
    }
    sections.push(format!(
        "Transcript items to compact:\n{}",
        render_compaction_transcript(transcript_items)
    ));
    sections.join("\n\n")
}

fn render_compaction_transcript(items: &[ConversationItemRecord]) -> String {
    items
        .iter()
        .filter_map(|item| {
            let text = item.content_text.as_deref()?;
            let role = match item.kind {
                crate::ConversationItemKind::UserText => "User",
                crate::ConversationItemKind::AssistantText => "Noema",
                crate::ConversationItemKind::Activity
                | crate::ConversationItemKind::ToolCall
                | crate::ConversationItemKind::ToolResult
                | crate::ConversationItemKind::ApprovalRequest
                | crate::ConversationItemKind::ApprovalResult
                | crate::ConversationItemKind::A2uiCard
                | crate::ConversationItemKind::ErrorNotice => return None,
            };
            Some(format!("[{}] {role}: {text}", item.sequence_index))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compaction_instructions(target_tokens: u32) -> String {
    format!(
        "Compact Noema conversation context into a durable rolling summary.\n\
         Write plain assistant text only. Target at most {target_tokens} tokens.\n\
         Preserve active user goals, durable decisions, unresolved references, \
         recently active entities, projects, files, tools, and explicit uncertainty.\n\
         Do not invent facts. Do not convert conversation-local details into memory claims."
    )
}

fn retry_summary_target(budget: ContextBudget) -> Option<u32> {
    let target = budget.compact_summary_target_tokens()?;
    Some((target / 2).max(MIN_RETRY_SUMMARY_TARGET_TOKENS))
}

async fn bound_summary_seed_to_context_budget(
    provider: &dyn RuntimeModelProvider,
    model_profile: Option<&str>,
    budget: ContextBudget,
    target_tokens: u32,
    instructions: &str,
    summary_seed: &mut SummarySeed,
) -> Result<u32, DaemonError> {
    let full_input = render_compaction_input(
        summary_seed.previous_summary.as_ref(),
        &summary_seed.transcript_items,
    );
    let full_token_estimate =
        count_tokens_or_estimate(provider, Some(instructions), &full_input, model_profile).await;
    if budget.fits_with_output_reserve(full_token_estimate, target_tokens) {
        return Ok(full_token_estimate);
    }

    let (bounded_len, bounded_token_estimate) = largest_fitting_transcript_prefix(
        provider,
        model_profile,
        budget,
        target_tokens,
        instructions,
        summary_seed.previous_summary.as_ref(),
        &summary_seed.transcript_items,
    )
    .await?;
    summary_seed.transcript_items.truncate(bounded_len);
    summary_seed.covered_item_end_sequence = summary_seed
        .transcript_items
        .last()
        .map_or(summary_seed.covered_item_end_sequence, |item| {
            item.sequence_index
        });
    summary_seed.source_item_ids = bounded_combined_source_item_ids(
        summary_seed.previous_summary.as_ref(),
        &summary_seed.transcript_items,
    );
    Ok(bounded_token_estimate)
}

async fn largest_fitting_transcript_prefix(
    provider: &dyn RuntimeModelProvider,
    model_profile: Option<&str>,
    budget: ContextBudget,
    target_tokens: u32,
    instructions: &str,
    previous_summary: Option<&ConversationContextSummaryRecord>,
    transcript_items: &[ConversationItemRecord],
) -> Result<(usize, u32), DaemonError> {
    let mut low = 0;
    let mut high = transcript_items.len();
    let mut best_token_estimate = 0;

    while low < high {
        let midpoint = (low + high).div_ceil(2);
        let input = render_compaction_input(previous_summary, &transcript_items[..midpoint]);
        let token_estimate =
            count_tokens_or_estimate(provider, Some(instructions), &input, model_profile).await;
        if budget.fits_with_output_reserve(token_estimate, target_tokens) {
            low = midpoint;
            best_token_estimate = token_estimate;
        } else {
            high = midpoint - 1;
        }
    }

    if low == 0 {
        let available = budget
            .available_input_tokens_with_output_reserve(target_tokens)
            .map_or_else(|| "unknown".to_string(), |tokens| tokens.to_string());
        return Err(ProviderError::InvalidRequest {
            message: format!(
                "context compaction input exceeds provider context window for a single transcript item; available input tokens: {available}"
            ),
        }
        .into());
    }

    if best_token_estimate == 0 {
        let input = render_compaction_input(previous_summary, &transcript_items[..low]);
        best_token_estimate =
            count_tokens_or_estimate(provider, Some(instructions), &input, model_profile).await;
    }

    Ok((low, best_token_estimate))
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

fn bounded_combined_source_item_ids(
    previous_summary: Option<&ConversationContextSummaryRecord>,
    items: &[ConversationItemRecord],
) -> Vec<String> {
    let mut source_item_ids = previous_summary
        .map(|summary| summary.source_item_ids.clone())
        .unwrap_or_default();
    source_item_ids.extend(items.iter().map(|item| item.item_id.clone()));
    bounded_source_item_ids_from_strings(source_item_ids)
}

fn bounded_source_item_ids_from_strings(source_item_ids: Vec<String>) -> Vec<String> {
    const MAX_SOURCE_IDS: usize = 64;
    if source_item_ids.len() <= MAX_SOURCE_IDS {
        return source_item_ids;
    }
    let head = source_item_ids.iter().take(MAX_SOURCE_IDS / 2).cloned();
    let tail = source_item_ids
        .iter()
        .rev()
        .take(MAX_SOURCE_IDS / 2)
        .cloned()
        .collect::<Vec<_>>();
    head.chain(tail.into_iter().rev()).collect()
}

#[derive(Debug)]
struct SummarySeed {
    previous_summary: Option<ConversationContextSummaryRecord>,
    transcript_items: Vec<ConversationItemRecord>,
    covered_item_start_sequence: i64,
    covered_item_end_sequence: i64,
    source_item_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversationItemKind, ConversationItemStatus};

    #[test]
    fn background_threshold_uses_context_budget() {
        let budget = ContextBudget::from_metadata(crate::ProviderContextMetadata {
            context_window_tokens: Some(1_000),
            default_output_reserve_tokens: Some(100),
            compact_summary_target_tokens: Some(100),
        });
        let plan = PlannedPromptContext {
            context: crate::daemon::runtime::prompt_context::PromptContext {
                active_summary: None,
                transcript_items: Vec::new(),
                rendered_context: String::new(),
            },
            instructions: String::new(),
            input: crate::GenerateInput::Text(String::new()),
            estimated_input_tokens: 611,
            budget,
            fits: true,
        };

        assert!(should_compact_background(&plan));
    }

    #[test]
    fn compaction_transcript_keeps_sequence_roles_and_text() {
        let transcript = render_compaction_transcript(&[
            ConversationItemRecord {
                item_id: "item:1".to_string(),
                conversation_id: "conversation:1".to_string(),
                turn_id: None,
                sequence_index: 1,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                content_text: Some("hello".to_string()),
                payload_json: serde_json::json!({}),
            },
            ConversationItemRecord {
                item_id: "item:2".to_string(),
                conversation_id: "conversation:1".to_string(),
                turn_id: None,
                sequence_index: 2,
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                content_text: Some("hi".to_string()),
                payload_json: serde_json::json!({}),
            },
        ]);

        assert_eq!(transcript, "[1] User: hello\n[2] Noema: hi");
    }

    #[test]
    fn combined_source_ids_include_previous_summary_provenance() {
        let previous_summary = ConversationContextSummaryRecord {
            summary_id: "context-summary:1".to_string(),
            conversation_id: "conversation:1".to_string(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "previous".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 10,
            source_item_ids: vec!["item:1".to_string(), "item:10".to_string()],
            input_token_estimate: 100,
            summary_token_estimate: 20,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        };
        let source_item_ids = bounded_combined_source_item_ids(
            Some(&previous_summary),
            &[ConversationItemRecord {
                item_id: "item:11".to_string(),
                conversation_id: "conversation:1".to_string(),
                turn_id: None,
                sequence_index: 11,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                content_text: Some("next".to_string()),
                payload_json: serde_json::json!({}),
            }],
        );

        assert_eq!(
            source_item_ids,
            vec![
                "item:1".to_string(),
                "item:10".to_string(),
                "item:11".to_string()
            ]
        );
    }
}
