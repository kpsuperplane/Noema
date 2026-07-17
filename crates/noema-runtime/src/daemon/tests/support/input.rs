fn current_user_input(input: &GenerateInput) -> String {
    match input {
        GenerateInput::Text(text) if has_tool_result_after_last_user_text(text) => {
            format!("NOEMA_LOCAL_TOOL_RESULT\n{text}")
        }
        GenerateInput::Text(text) => text.clone(),
        GenerateInput::Messages(messages) => messages
            .iter()
            .rev()
            .find(|message| message.role == noema_providers::GenerateMessageRole::User)
            .map_or_else(
                || input.render_for_token_count(),
                |message| message.content.clone(),
            ),
        GenerateInput::Items(items) if has_tool_result_after_last_user(items) => {
            format!(
                "NOEMA_LOCAL_TOOL_RESULT\n{}",
                input.render_for_token_count()
            )
        }
        GenerateInput::Items(items) => items
            .iter()
            .rev()
            .find_map(|item| match item {
                noema_providers::GenerateInputItem::Message(message)
                    if message.role == noema_providers::GenerateMessageRole::User =>
                {
                    Some(message.content.clone())
                }
                noema_providers::GenerateInputItem::Message(_)
                | noema_providers::GenerateInputItem::Reasoning(_)
                | noema_providers::GenerateInputItem::ToolCall(_)
                | noema_providers::GenerateInputItem::ToolResult(_) => None,
            })
            .unwrap_or_else(|| input.render_for_token_count()),
        GenerateInput::NativeToolResults(_) => input.render_for_token_count(),
    }
}

fn has_tool_result_after_last_user(items: &[GenerateInputItem]) -> bool {
    let last_user = items.iter().rposition(|item| {
        matches!(
            item,
            GenerateInputItem::Message(message)
                if message.role == noema_providers::GenerateMessageRole::User
        )
    });
    items.iter().enumerate().any(|(index, item)| {
        matches!(item, GenerateInputItem::ToolResult(_))
            && last_user.is_none_or(|user| index > user)
    })
}

fn has_tool_result_after_last_user_text(text: &str) -> bool {
    let Some(tool_result) = text.rfind("\"type\":\"function_call_output\"") else {
        return false;
    };
    text.rfind("user:").is_none_or(|user| tool_result > user)
}

fn input_message_texts(input: &GenerateInput) -> Vec<String> {
    match input {
        GenerateInput::Text(text) => vec![text.clone()],
        GenerateInput::Messages(messages) => messages
            .iter()
            .filter(|message| message.role != noema_providers::GenerateMessageRole::Developer)
            .map(|message| message.content.clone())
            .collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message)
                    if message.role != noema_providers::GenerateMessageRole::Developer =>
                {
                    Some(message.content.clone())
                }
                GenerateInputItem::Message(_) => None,
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(_) => Vec::new(),
    }
}

fn latest_model_context_section(input: &GenerateInput, section_id: &str) -> Option<String> {
    let messages = match input {
        GenerateInput::Messages(messages) => messages.iter().collect::<Vec<_>>(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message) => Some(message),
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::Text(_) | GenerateInput::NativeToolResults(_) => Vec::new(),
    };
    let mut latest = None;
    for message in messages {
        if message.role != noema_providers::GenerateMessageRole::Developer {
            continue;
        }
        let Some(envelope) = message
            .content
            .strip_prefix("NOEMA_MODEL_CONTEXT_UPDATE\n")
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        else {
            continue;
        };
        if envelope["section_id"].as_str() != Some(section_id) {
            continue;
        }
        latest = envelope["content"].as_str().map(str::to_string);
    }
    latest
}

fn model_context_section_update_count(input: &GenerateInput, section_id: &str) -> usize {
    let messages = match input {
        GenerateInput::Messages(messages) => messages.iter().collect::<Vec<_>>(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message) => Some(message),
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::Text(_) | GenerateInput::NativeToolResults(_) => Vec::new(),
    };
    messages
        .into_iter()
        .filter(|message| message.role == noema_providers::GenerateMessageRole::Developer)
        .filter_map(|message| {
            message
                .content
                .strip_prefix("NOEMA_MODEL_CONTEXT_UPDATE\n")
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        })
        .filter(|envelope| envelope["section_id"].as_str() == Some(section_id))
        .count()
}

fn input_tool_results(input: &GenerateInput) -> Vec<&noema_providers::GenerateToolResultInput> {
    match input {
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::ToolResult(result) => Some(result),
                GenerateInputItem::Message(_)
                | GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(results) => results.iter().collect(),
        GenerateInput::Text(_) | GenerateInput::Messages(_) => Vec::new(),
    }
}
