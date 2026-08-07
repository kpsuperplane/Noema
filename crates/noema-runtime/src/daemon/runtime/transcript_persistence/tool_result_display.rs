fn tool_result_display(name: Option<&str>, success: Option<bool>, payload: &Value) -> Value {
    let name = name.unwrap_or("tool");
    let mut display = json!({
        "name": readable_tool_name(name),
        "access": tool_access_label(name),
    });

    if name == "search_memory" {
        insert_display_value(&mut display, "scope", scope_label(payload.get("scope_ids")));
        insert_display_value(
            &mut display,
            "result",
            Some(memory_search_result_label(payload)),
        );
    } else if name == "web.search" {
        insert_display_value(
            &mut display,
            "result",
            Some(web_search_result_label(success, payload)),
        );
        insert_display_value(
            &mut display,
            "provider",
            payload
                .get("provider")
                .and_then(Value::as_str)
                .map(web_search_provider_label),
        );
        insert_display_value(
            &mut display,
            "reliability",
            payload
                .get("provider_contract")
                .and_then(Value::as_str)
                .map(web_search_contract_label),
        );
        insert_display_value(
            &mut display,
            "fallbackFrom",
            payload
                .get("fallback_from")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackReason",
            payload
                .get("fallback_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
    } else if name == "web.fetch" {
        insert_display_value(
            &mut display,
            "result",
            Some(web_fetch_result_label(success, payload)),
        );
        insert_display_value(
            &mut display,
            "model",
            payload
                .get("summary_model")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackFrom",
            payload
                .get("fallback_from")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackReason",
            payload
                .get("fallback_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
    } else if name.starts_with("web.browse.") {
        insert_display_value(
            &mut display,
            "result",
            Some(success_result_label(success, payload)),
        );
        insert_display_value(
            &mut display,
            "provider",
            payload.get("provider").and_then(Value::as_str).map(str::to_string),
        );
    } else if name == "update_own_name" {
        let result = payload
            .get("display_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| success_result_label(success, payload));
        insert_display_value(&mut display, "name", Some("Saved name".to_string()));
        insert_display_value(&mut display, "result", Some(result));
    } else {
        insert_display_value(
            &mut display,
            "result",
            Some(success_result_label(success, payload)),
        );
    }

    display
}
