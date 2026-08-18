fn tool_call_display(name: &str, payload: &Value) -> Value {
    let arguments = tool_arguments(payload);
    let mut display = json!({
        "name": readable_tool_name(name),
        "access": tool_access_label(name),
    });

    if name == "search_memory" {
        insert_display_value(
            &mut display,
            "purpose",
            purpose_label(arguments.get("purpose").and_then(Value::as_str)),
        );
        insert_display_value(
            &mut display,
            "scope",
            scope_label(arguments.get("scope_ids")),
        );
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        insert_display_value(
            &mut display,
            "target",
            Some(query.map_or_else(
                || "Scoped memories".to_string(),
                |query| format!("Memory search: {query}"),
            )),
        );
    } else if name == "web.search" {
        insert_display_value(
            &mut display,
            "purpose",
            arguments
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        insert_display_value(&mut display, "target", query.map(ToString::to_string));
    } else if matches!(name, "web.fetch" | "file.download") {
        insert_display_value(
            &mut display,
            "purpose",
            arguments
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(noema_capabilities::web::fetch::sanitized_display_url);
        insert_display_value(&mut display, "target", url);
    } else if name.starts_with("web.browse.") {
        let target = arguments
            .get("url")
            .and_then(Value::as_str)
            .map(noema_capabilities::web::fetch::sanitized_display_url);
        insert_display_value(&mut display, "target", target);
        insert_display_value(
            &mut display,
            "purpose",
            arguments.get("action").and_then(Value::as_str).map(str::to_string),
        );
    } else if name == "update_own_name" {
        insert_display_value(
            &mut display,
            "purpose",
            Some("Save the agent name you requested".to_string()),
        );
        if let Some(agent_name) = arguments.get("name").and_then(Value::as_str) {
            insert_display_value(&mut display, "target", Some(agent_name.to_string()));
        }
    } else {
        insert_display_value(
            &mut display,
            "purpose",
            Some("Use an enabled connected tool".to_string()),
        );
    }

    display
}
