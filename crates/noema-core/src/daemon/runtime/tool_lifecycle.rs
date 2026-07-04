use crate::provider::{GenerateActionItem, GenerateToolCall};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LocalToolCall {
    pub(super) output_index: usize,
    pub(super) call_id: Option<String>,
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
            name: call.name.clone(),
            payload: call.payload.clone(),
        })
        .collect()
}

pub(super) fn tool_call_action_item(call: &LocalToolCall) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: call.call_id.clone(),
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
                name: "search_memory".to_string(),
                payload: json!({
                    "scope_ids": ["human:local"],
                    "query": "",
                    "purpose": "answer_human_question"
                }),
            },
            GenerateToolCall {
                id: Some("call_2".to_string()),
                name: "mcp.web.search".to_string(),
                payload: json!({"query": "second"}),
            },
        ];

        let calls = local_tool_calls(&tool_calls);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].output_index, 0);
        assert_eq!(calls[1].output_index, 1);
        assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(calls[1].name, "mcp.web.search");
    }
}
