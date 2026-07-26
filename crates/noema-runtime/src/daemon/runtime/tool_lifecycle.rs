use noema_providers::{
    AssistantTextPhase, GenerateActionItem, GenerateReasoningItem, GenerateResponseItem,
    GenerateToolCall,
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LocalToolCall {
    pub(super) output_index: usize,
    pub(super) call_id: Option<String>,
    pub(super) provider_call_id: Option<String>,
    pub(super) provider_name: Option<String>,
    pub(super) name: String,
    pub(super) payload: Value,
}

pub(super) fn local_tool_calls(tool_calls: &[GenerateToolCall]) -> Vec<LocalToolCall> {
    tool_calls
        .iter()
        .enumerate()
        .map(|(output_index, call)| LocalToolCall {
            output_index,
            call_id: call.id.clone(),
            provider_call_id: call.provider_call_id.clone(),
            provider_name: call.provider_name.clone(),
            name: call.name.clone(),
            payload: call.payload.clone(),
        })
        .collect()
}

pub(super) fn tool_call_action_item(
    call: &LocalToolCall,
    persisted_payload: Option<Value>,
) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: call.call_id.clone(),
        provider_call_id: call.provider_call_id.clone(),
        provider_name: call.provider_name.clone(),
        name: call.name.clone(),
        payload: persisted_payload.unwrap_or_else(|| serde_json::json!({ "omitted": true })),
    }
}

pub(super) fn single_tool_display_description(
    responses: &[GenerateResponseItem],
    reasoning_items: &[GenerateReasoningItem],
    tool_call_count: usize,
) -> Option<String> {
    if tool_call_count != 1 {
        return None;
    }

    let description = responses
        .iter()
        .filter_map(|response| match response {
            GenerateResponseItem::Text {
                phase: None | Some(AssistantTextPhase::Commentary),
                text,
            } => Some(text.as_str()),
            GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::FinalAnswer),
                ..
            }
            | GenerateResponseItem::MultipleChoice { .. }
            | GenerateResponseItem::Structured { .. } => None,
        })
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ");
    if !description.is_empty() {
        return Some(description);
    }

    reasoning_items.iter().rev().find_map(|reasoning| {
        reasoning
            .summary
            .iter()
            .find_map(|summary| summary.lines().map(str::trim).find(|line| !line.is_empty()))
            .map(|line| {
                line.trim_matches(|character| matches!(character, '#' | '*' | '_' | '`'))
                    .trim()
                    .to_string()
            })
            .filter(|line| !line.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_providers::GenerateToolCall;
    use serde_json::json;

    #[test]
    fn local_tool_calls_preserve_exact_execution_arguments() {
        let tool_calls = vec![GenerateToolCall {
            id: Some("call_1".to_string()),
            provider_call_id: Some("provider_call_1".to_string()),
            provider_name: Some("provider_web_fetch".to_string()),
            name: noema_capabilities::web::fetch::WEB_FETCH_TOOL.to_string(),
            payload: json!({
                "url": "https://user:secret@example.com/private#fragment",
                "reason": "read"
            }),
        }];

        let calls = local_tool_calls(&tool_calls);

        assert_eq!(calls[0].payload, tool_calls[0].payload);
    }

    #[test]
    fn tool_description_requires_one_unambiguous_call() {
        let responses = vec![
            GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::Commentary),
                text: "  Searching memory\nfor the launch date.  ".to_string(),
            },
            GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::FinalAnswer),
                text: "This must not label a pending call.".to_string(),
            },
        ];

        assert_eq!(
            single_tool_display_description(&responses, &[], 1).as_deref(),
            Some("Searching memory for the launch date.")
        );
        assert_eq!(single_tool_display_description(&responses, &[], 2), None);
    }

    #[test]
    fn tool_description_falls_back_to_provider_reasoning_summary() {
        let reasoning_items = vec![GenerateReasoningItem {
            id: Some("reasoning_1".to_string()),
            encrypted_content: Some("opaque".to_string()),
            summary: vec![
                "\n**Searching the connected Notion workspace**\n\nI’ll locate a relevant page."
                    .to_string(),
            ],
        }];

        assert_eq!(
            single_tool_display_description(&[], &reasoning_items, 1).as_deref(),
            Some("Searching the connected Notion workspace")
        );
        assert_eq!(
            single_tool_display_description(&[], &reasoning_items, 2),
            None
        );
    }
}
