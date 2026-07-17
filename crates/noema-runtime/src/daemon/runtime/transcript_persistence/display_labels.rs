fn progress_audit_display(label: &str, status: &str, summary: Option<&str>) -> Value {
    let mut display = json!({
        "name": label,
        "access": "Reviews tool progress",
        "status": status,
    });
    insert_display_value(&mut display, "summary", summary.map(str::to_string));
    display
}

fn display_summary(display: &Value, key: &str) -> Option<String> {
    display
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn tool_arguments(payload: &Value) -> &Value {
    payload.get("arguments").unwrap_or(payload)
}

fn readable_tool_name(name: &str) -> String {
    match name {
        "search_memory" => "Search memory".to_string(),
        "update_own_name" => "Save name".to_string(),
        "web.search" => "Web Search".to_string(),
        "web.fetch" => "Fetched Web Page".to_string(),
        other => other
            .split('.')
            .next_back()
            .unwrap_or(other)
            .split(['_', '-'])
            .filter(|part| !part.is_empty())
            .enumerate()
            .map(|(index, part)| {
                let lower = part.to_ascii_lowercase();
                if index == 0 {
                    let mut chars = lower.chars();
                    chars
                        .next()
                        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                        .unwrap_or(lower)
                } else {
                    lower
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn tool_access_label(name: &str) -> &'static str {
    match name {
        "search_memory" => "Reads memory",
        "update_own_name" => "Updates agent profile",
        "web.search" => "Searches public web",
        "web.fetch" => "Fetches public web pages",
        _ => "Uses a connected tool",
    }
}

fn purpose_label(purpose: Option<&str>) -> Option<String> {
    match purpose {
        Some("answer_human_question") => {
            Some("Answer the question from saved memories".to_string())
        }
        Some("personalize_response") => {
            Some("Personalize this response from saved memories".to_string())
        }
        Some("continue_task") => Some("Continue the current task with saved context".to_string()),
        Some("use_tool") => Some("Use saved context before a tool action".to_string()),
        Some(other) => Some(other.replace('_', " ")),
        None => None,
    }
}

fn scope_label(scope_ids: Option<&Value>) -> Option<String> {
    let scopes = scope_ids?.as_array()?;
    let labels = scopes
        .iter()
        .filter_map(Value::as_str)
        .filter(|scope| !scope.trim().is_empty())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        None
    } else {
        Some(labels.join(", "))
    }
}

fn memory_search_result_label(payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    let memory_count = payload
        .get("memories")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut result = match memory_count {
        0 => "Found no memories".to_string(),
        1 => "Found 1 memory".to_string(),
        count => format!("Found {count} memories"),
    };
    let omission_count = payload
        .get("omissions")
        .and_then(Value::as_array)
        .map(|omissions| {
            omissions
                .iter()
                .filter_map(|omission| omission.get("count").and_then(Value::as_u64))
                .sum::<u64>()
        })
        .unwrap_or(0);
    if omission_count > 0 {
        result.push_str(&format!("; {omission_count} omitted by policy"));
    }
    result
}

fn web_search_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| success_result_label(success, payload))
}

fn web_search_provider_label(provider: &str) -> String {
    match provider {
        "duckduckgo_public" => "DuckDuckGo public search".to_string(),
        other => other.replace('_', " "),
    }
}

fn web_search_contract_label(contract: &str) -> String {
    match contract {
        "best_effort_public" => "Best effort".to_string(),
        other => other.replace('_', " "),
    }
}

fn web_fetch_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }

    let raw_chars = payload.get("raw_chars").and_then(Value::as_u64);
    let returned_chars = payload.get("returned_chars").and_then(Value::as_u64);
    if payload
        .get("content_kind")
        .and_then(Value::as_str)
        .is_some_and(|content_kind| content_kind == "summary")
        && raw_chars.is_some()
        && returned_chars.is_some()
    {
        return format!(
            "Summarized {} chars to {} chars",
            format_count(raw_chars.unwrap_or_default()),
            format_count(returned_chars.unwrap_or_default())
        );
    }

    if let Some(chars) = returned_chars.or(raw_chars) {
        return format!("Fetched {} chars", format_count(chars));
    }

    success_result_label(success, payload)
}

fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted.chars().rev().collect()
}

fn success_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    match success {
        Some(false) => "Failed".to_string(),
        _ => "Completed".to_string(),
    }
}

fn insert_display_value(display: &mut Value, key: &str, value: Option<String>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    if let Some(object) = display.as_object_mut() {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn assistant_stream_id(turn_id: &str, segment: &str) -> String {
    format!("assistant_stream:{turn_id}:{segment}")
}

pub(super) fn assistant_response_stream_id(stream_id: &str, response_index: usize) -> String {
    format!("{stream_id}:response:{response_index}")
}
