use crate::provider::GenerateOutputItem;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LocalToolCall {
    pub(super) output_index: usize,
    pub(super) call_id: Option<String>,
    pub(super) name: String,
    pub(super) payload: Value,
}

pub(super) fn local_tool_calls(output: &[GenerateOutputItem]) -> Vec<LocalToolCall> {
    output
        .iter()
        .enumerate()
        .filter_map(|(output_index, item)| match item {
            GenerateOutputItem::ToolCall { id, name, payload } => Some(LocalToolCall {
                output_index,
                call_id: id.clone(),
                name: name.clone(),
                payload: payload.clone(),
            }),
            GenerateOutputItem::AssistantText { .. }
            | GenerateOutputItem::MemoryProposals { .. }
            | GenerateOutputItem::ToolResult { .. }
            | GenerateOutputItem::ApprovalRequest { .. }
            | GenerateOutputItem::ApprovalResult { .. }
            | GenerateOutputItem::Structured { .. } => None,
        })
        .collect()
}

pub(super) fn tool_call_output_item(call: &LocalToolCall) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: call.call_id.clone(),
        name: call.name.clone(),
        payload: call.payload.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{AssistantTextPhase, GenerateOutputItem};
    use serde_json::json;

    #[test]
    fn local_tool_calls_preserve_provider_output_indexes() {
        let output = vec![
            GenerateOutputItem::AssistantText {
                phase: Some(AssistantTextPhase::Commentary),
                text: "Checking memory.".to_string(),
            },
            GenerateOutputItem::ToolCall {
                id: Some("call_1".to_string()),
                name: "search_memory".to_string(),
                payload: json!({
                    "scope_ids": ["human:local"],
                    "query": "",
                    "purpose": "answer_human_question"
                }),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ];

        let calls = local_tool_calls(&output);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].output_index, 1);
        assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(calls[0].name, "search_memory");
    }
}
