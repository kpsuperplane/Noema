use crate::{
    ConversationContextSummaryRecord, ConversationItemKind, ConversationItemRecord, GenerateInput,
    GenerateMessage, GenerateMessageRole, NoemaStore,
    daemon::{
        agent_onboarding::AgentPromptIdentity,
        prompts::{PromptToolExposure, build_structured_turn_system_prompt},
        protocol::DaemonError,
    },
    provider::{
        GenerateInputItem, GenerateReasoningInput, GenerateToolCallInput, GenerateToolResultInput,
    },
};
use serde_json::Value;

use super::{
    context_window::{ContextBudget, estimate_text_tokens},
    handle::RuntimeModelProvider,
};

/// Model-visible context selected for one provider turn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PromptContext {
    pub(super) active_summary: Option<ConversationContextSummaryRecord>,
    pub(super) transcript_items: Vec<ConversationItemRecord>,
    pub(super) rendered_context: Option<String>,
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
    pub(super) agent_identity: &'a AgentPromptIdentity,
    pub(super) rendered_tools: &'a str,
    pub(super) native_tools_available: bool,
    pub(super) legacy_builtin_envelope_tools: &'a [String],
    pub(super) current_input: &'a str,
}

struct LoadedPromptPlanRequest<'a> {
    provider: &'a dyn RuntimeModelProvider,
    model_profile: Option<&'a str>,
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
        model_profile: request.model_profile,
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
        request.agent_identity,
        request.rendered_tools,
        PromptToolExposure {
            native_tools_available: request.native_tools_available,
            legacy_builtin_envelope_tools: request.legacy_builtin_envelope_tools,
        },
    );
    let input = build_turn_input(
        request.context.rendered_context.as_deref(),
        &request.context.transcript_items,
        request.current_input,
    );
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
    rendered_context: Option<&str>,
    transcript_items: &[ConversationItemRecord],
    current_input: &str,
) -> GenerateInput {
    let mut has_structured_items = false;
    let mut items = rendered_context
        .map(|content| {
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::Assistant,
                content: content.to_string(),
            })
        })
        .into_iter()
        .collect::<Vec<_>>();
    items.extend(
        transcript_items
            .iter()
            .filter_map(input_item_from_transcript_item),
    );
    if !current_input.trim().is_empty() {
        items.push(GenerateInputItem::Message(GenerateMessage {
            role: GenerateMessageRole::User,
            content: current_input.to_string(),
        }));
    }
    for item in &items {
        if !matches!(item, GenerateInputItem::Message(_)) {
            has_structured_items = true;
            break;
        }
    }
    if has_structured_items {
        GenerateInput::Items(items)
    } else {
        GenerateInput::Messages(
            items
                .into_iter()
                .filter_map(|item| match item {
                    GenerateInputItem::Message(message) => Some(message),
                    GenerateInputItem::Reasoning(_)
                    | GenerateInputItem::ToolCall(_)
                    | GenerateInputItem::ToolResult(_) => None,
                })
                .collect(),
        )
    }
}

pub(super) fn input_item_from_transcript_item(
    item: &ConversationItemRecord,
) -> Option<GenerateInputItem> {
    match item.kind {
        ConversationItemKind::UserText => text_message_item(item, GenerateMessageRole::User),
        ConversationItemKind::AssistantText => {
            text_message_item(item, GenerateMessageRole::Assistant)
        }
        ConversationItemKind::MultipleChoicePrompt => multiple_choice_prompt_message_item(item),
        ConversationItemKind::MultipleChoiceSelection => {
            multiple_choice_selection_message_item(item)
        }
        ConversationItemKind::ToolCall => tool_call_input_item(item),
        ConversationItemKind::ToolResult => tool_result_input_item(item),
        ConversationItemKind::Reasoning => reasoning_input_item(item),
        ConversationItemKind::Activity
        | ConversationItemKind::A2uiCard
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult
        | ConversationItemKind::ArtifactReference
        | ConversationItemKind::ErrorNotice => None,
    }
}

fn reasoning_input_item(item: &ConversationItemRecord) -> Option<GenerateInputItem> {
    let value = item.payload_json.pointer("/provider_reasoning")?;
    let encrypted_content = action_string(value, "encrypted_content")?;
    Some(GenerateInputItem::Reasoning(GenerateReasoningInput {
        id: action_string(value, "id"),
        encrypted_content,
    }))
}

fn text_message_item(
    item: &ConversationItemRecord,
    role: GenerateMessageRole,
) -> Option<GenerateInputItem> {
    let content = item.content_text.as_deref()?.trim();
    if content.is_empty() {
        return None;
    }
    Some(GenerateInputItem::Message(GenerateMessage {
        role,
        content: content.to_string(),
    }))
}

fn multiple_choice_prompt_message_item(item: &ConversationItemRecord) -> Option<GenerateInputItem> {
    let prompt = item.payload_json.get("prompt")?.as_str()?;
    let options = item.payload_json.get("options")?.as_array()?;
    let rendered_options = options
        .iter()
        .filter_map(|option| {
            let id = option.get("id")?.as_str()?;
            let label = option.get("label")?.as_str()?;
            Some(format!("{id}={label}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    Some(GenerateInputItem::Message(GenerateMessage {
        role: GenerateMessageRole::Assistant,
        content: format!("assistant multiple_choice: {prompt}\noptions: {rendered_options}"),
    }))
}

fn multiple_choice_selection_message_item(
    item: &ConversationItemRecord,
) -> Option<GenerateInputItem> {
    let prompt_item_id = item.payload_json.get("prompt_item_id")?.as_str()?;
    let options = item.payload_json.get("selected_options")?.as_array()?;
    let rendered_options = options
        .iter()
        .filter_map(|option| {
            let id = option.get("id")?.as_str()?;
            let label = option.get("label")?.as_str()?;
            Some(format!("{id}={label}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    Some(GenerateInputItem::Message(GenerateMessage {
        role: GenerateMessageRole::User,
        content: format!("user selected for {prompt_item_id}: {rendered_options}"),
    }))
}

fn tool_call_input_item(item: &ConversationItemRecord) -> Option<GenerateInputItem> {
    let action = item.payload_json.pointer("/metadata/action")?;
    let call_id =
        action_string(action, "provider_call_id").or_else(|| action_string(action, "id"))?;
    let name = action_string(action, "name")?;
    Some(GenerateInputItem::ToolCall(GenerateToolCallInput {
        id: provider_function_call_item_id(action),
        call_id,
        provider_name: action_string(action, "provider_name"),
        name,
        arguments: action.get("payload").cloned().unwrap_or(Value::Null),
    }))
}

fn tool_result_input_item(item: &ConversationItemRecord) -> Option<GenerateInputItem> {
    let action = item.payload_json.pointer("/metadata/action")?;
    let call_id =
        action_string(action, "provider_call_id").or_else(|| action_string(action, "call_id"))?;
    let name = action_string(action, "name")?;
    Some(GenerateInputItem::ToolResult(GenerateToolResultInput {
        id: provider_function_call_item_id(action),
        call_id,
        provider_name: action_string(action, "provider_name"),
        name,
        arguments: Value::Null,
        success: action
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(item.status == crate::ConversationItemStatus::Completed),
        payload: action.get("payload").cloned().unwrap_or(Value::Null),
    }))
}

fn action_string(action: &Value, key: &str) -> Option<String> {
    action
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_function_call_item_id(action: &Value) -> Option<String> {
    action_string(action, "id").filter(|id| id.starts_with("fc"))
}

fn render_prompt_context(summary: Option<&ConversationContextSummaryRecord>) -> Option<String> {
    summary.map(|summary| {
        format!(
            "Compacted conversation context:\n{}\n\nRecent transcript after this compacted checkpoint follows in subsequent messages.",
            summary.summary_text
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
        provider::GenerateInputItem,
    };

    #[test]
    fn persisted_local_tool_call_id_is_not_replayed_as_provider_item_id() {
        let item = ConversationItemRecord {
            item_id: "item:1".to_string(),
            conversation_id: "conversation:1".to_string(),
            turn_id: None,
            sequence_index: 1,
            cursor: "conversation_item:1".to_string(),
            kind: ConversationItemKind::ToolCall,
            status: ConversationItemStatus::Completed,
            content_text: Some("Tool call: update_own_name".to_string()),
            payload_json: serde_json::json!({
                "metadata": {
                    "action": {
                        "id": "call_name_1",
                        "provider_call_id": null,
                        "provider_name": null,
                        "name": "update_own_name",
                        "payload": {"name": "Momo"}
                    }
                }
            }),
            metadata: serde_json::json!({}),
        };

        let Some(GenerateInputItem::ToolCall(call)) = input_item_from_transcript_item(&item) else {
            panic!("expected tool call input item");
        };

        assert_eq!(call.id, None);
        assert_eq!(call.call_id, "call_name_1");
    }
}
