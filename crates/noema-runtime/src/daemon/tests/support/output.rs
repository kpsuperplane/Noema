fn fake_generate_response(
    output: Vec<GenerateOutputItem>,
    provider: &str,
    model: String,
) -> GenerateResponse {
    let mut responses = Vec::new();
    let mut tool_calls = Vec::new();

    for item in output {
        match item {
            GenerateOutputItem::AssistantText { phase, text } => {
                responses.push(GenerateResponseItem::Text { phase, text });
            }
            GenerateOutputItem::MultipleChoice {
                phase,
                prompt,
                selection_mode,
                options,
            } => {
                responses.push(GenerateResponseItem::MultipleChoice {
                    phase,
                    prompt,
                    selection_mode,
                    options,
                });
            }
            GenerateOutputItem::ToolCall {
                id,
                provider_call_id,
                provider_name,
                name,
                payload,
            } => {
                tool_calls.push(GenerateToolCall {
                    id,
                    provider_call_id,
                    provider_name,
                    name,
                    payload,
                });
            }
        }
    }

    let response_status = if tool_calls.is_empty() {
        GenerateResponseStatus::Final
    } else {
        GenerateResponseStatus::NeedsTools
    };

    GenerateResponse {
        responses,
        tool_calls,
        reasoning_items: Vec::new(),
        response_status,
        provider: provider.to_string(),
        model,
        response_id: Some("fake-response".to_string()),
        usage: None,
    }
}

fn assistant_with_no_memories(text: &str) -> Vec<GenerateOutputItem> {
    assistant_items_with_no_memories(&[text])
}

fn assistant_items_with_no_memories(texts: &[&str]) -> Vec<GenerateOutputItem> {
    texts
        .iter()
        .map(|text| GenerateOutputItem::AssistantText {
            phase: None,
            text: (*text).to_string(),
        })
        .collect()
}

fn search_memory_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "search_memory".to_string(),
        payload,
    }
}

fn web_search_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: Some(id.to_string()),
        provider_name: Some("web.search".to_string()),
        name: "web.search".to_string(),
        payload,
    }
}

fn web_fetch_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: Some(id.to_string()),
        provider_name: Some("web.fetch".to_string()),
        name: "web.fetch".to_string(),
        payload,
    }
}

fn task_delegate_tool_call(id: &str, title: &str, valid: bool) -> GenerateOutputItem {
    let arguments = if valid {
        json!({
            "title": title,
            "request": format!("Complete {title} and report the result."),
            "complexity": "simple",
            "executor_model_pool_entry_id": "task_pool:setting:simple",
            "validation_criteria": [{
                "description": format!("{title} is complete"),
                "evidence_required": "A concise sourced result"
            }]
        })
    } else {
        json!({"title": title})
    };
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "task.delegate".to_string(),
        payload: json!({"arguments": arguments}),
    }
}

fn artifact_create_local_file_tool_call(
    id: &str,
    payload: serde_json::Value,
) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: Some(id.to_string()),
        provider_name: Some("artifact.create_local_file".to_string()),
        name: "artifact.create_local_file".to_string(),
        payload,
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

fn update_own_name_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "update_own_name".to_string(),
        payload,
    }
}
