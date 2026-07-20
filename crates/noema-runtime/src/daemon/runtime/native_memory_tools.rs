use noema_memory::{NATIVE_SEARCH_MEMORY_TOOL_NAME, NativeMemory, READ_MEMORY_PAGE_TOOL_NAME};
use serde_json::{Value, json};

use super::super::{
    local_tool_results::{LocalToolKind, LocalToolResult},
    tool_lifecycle::LocalToolCall,
};

pub(super) fn execute_native_memory_tool(
    memory: Option<&NativeMemory>,
    call: &LocalToolCall,
) -> Option<LocalToolResult> {
    if call.name == READ_MEMORY_PAGE_TOOL_NAME {
        let page = call
            .payload
            .get("arguments")
            .and_then(Value::as_object)
            .and_then(|arguments| arguments.get("page"))
            .and_then(Value::as_str)
            .or_else(|| call.payload.get("page").and_then(Value::as_str));
        let (success, payload) = match (memory, page) {
            (Some(memory), Some(page)) => match memory.read_page(page) {
                Ok(page) => (true, json!({"page": page})),
                Err(error) => (false, json!({"error": error.to_string()})),
            },
            _ => (false, json!({"error": "native memory is unavailable"})),
        };
        Some(LocalToolResult::from_call(
            call,
            LocalToolKind::Memory,
            success,
            payload,
            true,
        ))
    } else if call.name == NATIVE_SEARCH_MEMORY_TOOL_NAME {
        let Some(memory) = memory else {
            return Some(LocalToolResult::from_call(
                call,
                LocalToolKind::Memory,
                false,
                json!({"error": "native memory is unavailable"}),
                true,
            ));
        };
        let arguments = call.payload.get("arguments").unwrap_or(&call.payload);
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(8)
            .clamp(1, 16) as usize;
        let (success, payload) = match memory.search(query, limit) {
            Ok(results) => (true, json!({"scope_id": "human:local", "pages": results})),
            Err(error) => (false, json!({"error": error.to_string()})),
        };
        Some(LocalToolResult::from_call(
            call,
            LocalToolKind::Memory,
            success,
            payload,
            true,
        ))
    } else {
        None
    }
}
