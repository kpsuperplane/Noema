fn fake_generate_response(
    (responses, tool_calls): (Vec<GenerateResponseItem>, Vec<GenerateToolCall>),
    provider: &str,
    model: String,
) -> GenerateResponse {
    GenerateResponse {
        responses,
        tool_calls,
        reasoning_items: Vec::new(),
        hosted_web_searches: Vec::new(),
        provider: provider.to_string(),
        model,
        response_id: Some("fake-response".to_string()),
        usage: None,
    }
}

fn assistant_with_no_memories(text: &str) -> (Vec<GenerateResponseItem>, Vec<GenerateToolCall>) {
    assistant_with_tools(text, None, Vec::new())
}

fn assistant_with_tools(
    text: &str,
    phase: Option<AssistantTextPhase>,
    tool_calls: Vec<GenerateToolCall>,
) -> (Vec<GenerateResponseItem>, Vec<GenerateToolCall>) {
    (
        vec![GenerateResponseItem::Text {
            phase,
            text: text.to_string(),
            citations: Vec::new(),
        }],
        tool_calls,
    )
}

fn tool_calls_only(
    tool_calls: Vec<GenerateToolCall>,
) -> (Vec<GenerateResponseItem>, Vec<GenerateToolCall>) {
    (Vec::new(), tool_calls)
}

fn search_memory_tool_call(id: &str, payload: serde_json::Value) -> GenerateToolCall {
    GenerateToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "search_memory".to_string(),
        payload,
    }
}

fn task_delegate_tool_call(id: &str, title: &str, valid: bool) -> GenerateToolCall {
    let arguments = if valid {
        json!({
            "title": title,
            "description": format!("Complete {title} and report the result."),
            "project": {"kind": "none"},
            "execution_intent": {
                "request_markdown": format!("Complete {title} and report the result."),
                "complexity": "simple"
            }
        })
    } else {
        json!({"title": title})
    };
    GenerateToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "task.delegate".to_string(),
        payload: arguments,
    }
}

fn search_memory_action_item(id: &str, payload: serde_json::Value) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "search_memory".to_string(),
        payload,
    }
}

fn update_own_name_tool_call(id: &str, payload: serde_json::Value) -> GenerateToolCall {
    GenerateToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "update_own_name".to_string(),
        payload,
    }
}
