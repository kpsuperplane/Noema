use noema_capabilities::web::fetch::{WEB_FETCH_TOOL, sanitize_payload_for_storage};
use noema_providers::{
    AssistantTextPhase, GenerateActionItem, GenerateResponseItem, GenerateToolCall,
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
            payload: if call.name == WEB_FETCH_TOOL {
                sanitize_payload_for_storage(&call.payload)
            } else {
                call.payload.clone()
            },
        })
        .collect()
}

pub(super) fn tool_call_action_item(call: &LocalToolCall) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: call.call_id.clone(),
        provider_call_id: call.provider_call_id.clone(),
        provider_name: call.provider_name.clone(),
        name: call.name.clone(),
        payload: call.payload.clone(),
    }
}

pub(super) fn single_tool_display_description(
    responses: &[GenerateResponseItem],
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
    (!description.is_empty()).then_some(description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_providers::GenerateToolCall;
    use serde_json::json;

    #[test]
    fn local_tool_calls_redact_sensitive_web_fetch_urls_before_persistence() {
        let tool_calls = vec![GenerateToolCall {
            id: Some("call_1".to_string()),
            provider_call_id: Some("provider_call_1".to_string()),
            provider_name: Some("provider_web_fetch".to_string()),
            name: WEB_FETCH_TOOL.to_string(),
            payload: json!({
                "url": "https://user:secret@example.com/private#fragment",
                "reason": "read"
            }),
        }];

        let calls = local_tool_calls(&tool_calls);

        assert_eq!(
            calls[0].payload["url"],
            noema_capabilities::web::fetch::REDACTED_SENSITIVE_URL
        );
        assert_eq!(calls[0].payload["__noema_rejected_sensitive_url"], true);
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
            single_tool_display_description(&responses, 1).as_deref(),
            Some("Searching memory for the launch date.")
        );
        assert_eq!(single_tool_display_description(&responses, 2), None);
    }
}
