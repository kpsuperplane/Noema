use crate::provider::{GenerateActionItem, GenerateToolCall};
use crate::web_fetch::tool::{WEB_FETCH_TOOL, sanitize_web_fetch_payload_for_storage};
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
                sanitize_web_fetch_payload_for_storage(&call.payload)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::GenerateToolCall;
    use serde_json::json;

    #[test]
    fn local_tool_calls_preserve_provider_output_indexes() {
        let tool_calls = vec![
            GenerateToolCall {
                id: Some("call_1".to_string()),
                provider_call_id: Some("provider_call_1".to_string()),
                provider_name: Some("provider_search_memory".to_string()),
                name: "search_memory".to_string(),
                payload: json!({
                    "scope_ids": ["human:local"],
                    "query": "",
                    "purpose": "answer_human_question"
                }),
            },
            GenerateToolCall {
                id: Some("call_2".to_string()),
                provider_call_id: None,
                provider_name: None,
                name: "mcp.web.search".to_string(),
                payload: json!({"query": "second"}),
            },
        ];

        let calls = local_tool_calls(&tool_calls);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].output_index, 0);
        assert_eq!(calls[1].output_index, 1);
        assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(
            calls[0].provider_call_id.as_deref(),
            Some("provider_call_1")
        );
        assert_eq!(
            calls[0].provider_name.as_deref(),
            Some("provider_search_memory")
        );
        assert_eq!(calls[1].name, "mcp.web.search");
    }

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
            crate::web_fetch::tool::REDACTED_SENSITIVE_WEB_FETCH_URL
        );
        assert_eq!(calls[0].payload["__noema_rejected_sensitive_url"], true);
    }
}
