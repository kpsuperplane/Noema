fn current_user_input(input: &GenerateInput) -> String {
    match input {
        GenerateInput::Text(text) if has_tool_result_after_last_user_text(text) => {
            format!("NOEMA_LOCAL_TOOL_RESULT\n{text}")
        }
        GenerateInput::Text(text) => text.clone(),
        GenerateInput::Items(items) if has_tool_result_after_last_user(items) => {
            format!(
                "NOEMA_LOCAL_TOOL_RESULT\n{}",
                input.render_for_token_count()
            )
        }
        GenerateInput::Messages(_)
        | GenerateInput::Items(_)
        | GenerateInput::NativeToolResults(_) => input_messages(input)
            .into_iter()
            .rev()
            .find(|message| message.role == noema_providers::GenerateMessageRole::User)
            .map(|message| message.content.clone())
            .unwrap_or_else(|| input.render_for_token_count()),
    }
}

fn input_messages(input: &GenerateInput) -> Vec<&noema_providers::GenerateMessage> {
    match input {
        GenerateInput::Messages(messages) => messages.iter().collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message) => Some(message),
                GenerateInputItem::AssistantText(_)
                | GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::Text(_) | GenerateInput::NativeToolResults(_) => Vec::new(),
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
            .filter(|message| {
                matches!(
                    message.role,
                    noema_providers::GenerateMessageRole::User
                        | noema_providers::GenerateMessageRole::Assistant
                )
            })
            .map(|message| message.content.clone())
            .collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message)
                    if matches!(
                        message.role,
                        noema_providers::GenerateMessageRole::User
                            | noema_providers::GenerateMessageRole::Assistant
                    ) => {
                    Some(message.content.clone())
                }
                GenerateInputItem::Message(_) => None,
                GenerateInputItem::AssistantText(message) => Some(message.content.clone()),
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(_) => Vec::new(),
    }
}

fn latest_model_context_section(input: &GenerateInput, section_id: &str) -> Option<String> {
    let mut latest = None;
    for message in input_messages(input) {
        if !matches!(
            message.role,
            noema_providers::GenerateMessageRole::System
                | noema_providers::GenerateMessageRole::Developer
        ) {
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

fn input_tool_results(input: &GenerateInput) -> Vec<&noema_providers::GenerateToolResultInput> {
    match input {
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::ToolResult(result) => Some(result),
                GenerateInputItem::Message(_)
                | GenerateInputItem::AssistantText(_)
                | GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(results) => results.iter().collect(),
        GenerateInput::Text(_) | GenerateInput::Messages(_) => Vec::new(),
    }
}

fn has_current_tool_results(input: &GenerateInput) -> bool {
    match input {
        GenerateInput::Text(text) => has_tool_result_after_last_user_text(text),
        GenerateInput::Items(items) => has_tool_result_after_last_user(items),
        GenerateInput::NativeToolResults(results) => !results.is_empty(),
        GenerateInput::Messages(_) => false,
    }
}
