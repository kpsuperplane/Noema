use super::*;

#[test]
fn assistant_delta_stream_id_includes_prior_output_offset() {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let context = ConversationMemoryContext {
        turn_index: 1,
        conversation_id: "conversation:1".to_string(),
        turn_id: "turn:1".to_string(),
        user_item_id: "user:1".to_string(),
        assistant_item_id: None,
    };

    handle_provider_stream_event(
        GenerateStreamEvent::AssistantTextDelta {
            response_index: 1,
            delta: "Done".to_string(),
        },
        &item_tx,
        &context,
        "assistant_stream:turn:1:continuation",
        2,
    );

    let event = item_rx.try_recv().expect("assistant delta event");
    let TurnStreamEvent::AssistantTextDelta {
        stream_id,
        response_index,
        delta,
        ..
    } = event
    else {
        panic!("expected assistant delta event");
    };
    assert_eq!(stream_id, "assistant_stream:turn:1:continuation:response:3");
    assert_eq!(response_index, 1);
    assert_eq!(delta, "Done");
}

#[test]
fn hosted_web_search_stream_event_emits_canonical_tool_call() {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let context = ConversationMemoryContext {
        turn_index: 4,
        conversation_id: "conversation:1".to_string(),
        turn_id: "turn:1".to_string(),
        user_item_id: "user:1".to_string(),
        assistant_item_id: None,
    };

    handle_provider_stream_event(
        GenerateStreamEvent::HostedWebSearchStarted {
            output_index: 2,
            id: Some("ws_1".to_string()),
        },
        &item_tx,
        &context,
        "assistant_stream:turn:1:initial",
        3,
    );

    let event = item_rx.try_recv().expect("hosted search activity");
    let TurnStreamEvent::ConversationItem {
        item_id,
        cursor,
        item,
        ..
    } = event
    else {
        panic!("expected conversation item");
    };
    assert_eq!(item_id, "transient:tool_call:conversation:1:4:5");
    assert_eq!(cursor, None);
    let TurnTranscriptItem::Activity {
        id,
        activity_kind,
        status,
        title,
        ..
    } = *item
    else {
        panic!("expected activity");
    };
    assert_eq!(id, "tool_call:conversation:1:4:5");
    assert_eq!(activity_kind, "tool_call");
    assert_eq!(status, TurnActivityStatus::Started);
    assert_eq!(title, "Tool call: web.search");
}

#[test]
fn web_search_tool_call_display_shows_visible_query() {
    let display = tool_call_display(
        "web.search",
        &json!({
            "query": "rust language",
            "reason": "answer current question",
            "max_results": 3
        }),
    );

    assert_eq!(display["name"], "Web Search");
    assert_eq!(display["access"], "Searches public web");
    assert_eq!(display["target"], "rust language");
    assert_eq!(display["purpose"], "answer current question");
}

#[test]
fn memory_search_display_describes_lexical_page_matches() {
    let empty = tool_result_display(Some("search_memory"), Some(true), &json!({"pages": []}));
    let matched = tool_result_display(
        Some("search_memory"),
        Some(true),
        &json!({"pages": [{"path": "health-and-lifestyle.md"}]}),
    );

    assert_eq!(empty["result"], "No lexical matches");
    assert_eq!(matched["result"], "Found 1 matching page");
}

#[test]
fn web_search_display_shows_provider_fallback_without_raw_payload() {
    let display = tool_result_display(
        Some("web.search"),
        Some(true),
        &json!({
            "provider": "duckduckgo_public",
            "provider_contract": "best_effort_public",
            "fallback_from": "openai",
            "fallback_reason": "provider account unauthenticated",
            "query": "rust learn",
            "results": [],
            "summary": "No web results found",
            "raw_provider_payload": "secret",
        }),
    );

    assert_eq!(display["provider"], "DuckDuckGo public search");
    assert_eq!(display["fallbackFrom"], "openai");
    assert_eq!(
        display["fallbackReason"],
        "provider account unauthenticated"
    );
    assert!(!display.to_string().contains("secret"));
}

#[test]
fn web_fetch_tool_call_display_shows_visible_url() {
    let display = tool_call_display(
        "web.fetch",
        &json!({
            "url": "https://example.com/page",
            "reason": "read public documentation",
            "max_chars": 4000
        }),
    );

    assert_eq!(display["name"], "Fetched Web Page");
    assert_eq!(display["access"], "Fetches public web pages");
    assert_eq!(display["target"], "https://example.com/page");
    assert_eq!(display["purpose"], "read public documentation");
}

#[test]
fn web_fetch_tool_call_display_removes_credentials_and_preserves_ordinary_components() {
    let display = tool_call_display(
        "web.fetch",
        &json!({
            "url": "https://user:secret@example.com/page#token",
            "reason": "read public documentation"
        }),
    );

    assert_eq!(display["target"], "https://example.com/page#token");
    assert!(!display.to_string().contains("secret"));
}

#[test]
fn web_fetch_tool_result_display_shows_raw_and_summary_counts() {
    let raw_display = tool_result_display(
        Some("web.fetch"),
        Some(true),
        &json!({
            "provider": "direct_http",
            "fallback_from": "provider_account:codex:test",
            "fallback_reason": "bound provider account does not declare web.fetch",
            "url": "https://example.com/page",
            "final_url": "https://example.com/page",
            "title": "Example",
            "format": "markdown",
            "extraction": "readability_rs",
            "content_kind": "raw_markdown",
            "content": "secret raw fetched body",
            "raw_excerpt": "secret raw excerpt",
            "raw_chars": 183421,
            "returned_chars": 12840,
            "summary_model": null,
            "summary_strategy": "not_summarized",
            "truncated": true
        }),
    );
    assert_eq!(raw_display["name"], "Fetched Web Page");
    assert_eq!(raw_display["access"], "Fetches public web pages");
    assert_eq!(raw_display["result"], "Fetched 12,840 chars");
    assert_eq!(raw_display["fallbackFrom"], "provider_account:codex:test");
    assert_eq!(
        raw_display["fallbackReason"],
        "bound provider account does not declare web.fetch"
    );
    assert!(raw_display.get("model").is_none());
    assert!(!raw_display.to_string().contains("secret raw"));

    let saved_name_display = tool_result_display(
        Some("update_own_name"),
        Some(true),
        &json!({
            "display_name": "Momo",
        }),
    );
    assert_eq!(saved_name_display["name"], "Saved name");
    assert_eq!(saved_name_display["result"], "Momo");

    let summary_display = tool_result_display(
        Some("web.fetch"),
        Some(true),
        &json!({
            "provider": "direct_http",
            "url": "https://example.com/long",
            "final_url": "https://example.com/long",
            "title": "Long Example",
            "format": "markdown",
            "extraction": "readability_rs",
            "content_kind": "summary",
            "content": "secret summary text",
            "raw_excerpt": "secret raw excerpt",
            "raw_chars": 183421,
            "returned_chars": 4972,
            "summary_model": "gpt-5.4-mini",
            "summary_strategy": "single_pass",
            "truncated": false
        }),
    );
    assert_eq!(
        summary_display["result"],
        "Summarized 183,421 chars to 4,972 chars"
    );
    assert_eq!(summary_display["model"], "gpt-5.4-mini");
    assert!(!summary_display.to_string().contains("secret"));
}

#[test]
fn provider_usage_metadata_projects_reported_zero_and_missing_cache_usage() {
    let usage = |input_tokens, cached_input_tokens| noema_providers::TokenUsage {
        input_tokens,
        output_tokens: 10,
        total_tokens: input_tokens + 10,
        cached_input_tokens,
    };
    let metadata = |provider, cached_input_tokens| {
        provider_usage_metadata(
            provider,
            "gpt-test",
            "initial",
            ProviderResponsePosition {
                response_index: 0,
                output_index: Some(0),
            },
            Some(&usage(12_000, cached_input_tokens)),
        )
    };

    let reported = metadata("codex", Some(9_600));

    assert_eq!(reported["provider_usage"]["provider"], "codex");
    assert_eq!(reported["provider_usage"]["model"], "gpt-test");
    assert_eq!(reported["provider_usage"]["phase"], "initial");
    assert_eq!(reported["provider_usage"]["response_index"], 0);
    assert_eq!(reported["provider_usage"]["output_index"], 0);
    assert_eq!(reported["provider_usage"]["input_tokens"], 12_000);
    assert_eq!(reported["provider_usage"]["cached_input_tokens"], 9_600);
    assert_eq!(reported["provider_usage"]["cache_hit_ratio"], 0.8);

    let zero = metadata("codex", Some(0));
    assert_eq!(zero["provider_usage"]["cached_input_tokens"], 0);
    assert_eq!(zero["provider_usage"]["cache_hit_ratio"], 0.0);

    let missing = metadata("foundation_local", None);
    assert!(
        missing["provider_usage"]
            .get("cached_input_tokens")
            .is_none()
    );
    assert!(missing["provider_usage"].get("cache_hit_ratio").is_none());
}

#[test]
fn progress_audit_display_labels_running_and_completed_states() {
    let running = progress_audit_display("Checking progress", "running", None);
    assert_eq!(running["name"], "Checking progress");
    assert_eq!(running["status"], "running");

    let completed = progress_audit_display(
        "Still making progress",
        "completed",
        Some("Found new sources and is preparing the write step."),
    );
    assert_eq!(completed["name"], "Still making progress");
    assert_eq!(completed["status"], "completed");
    assert_eq!(
        completed["summary"],
        "Found new sources and is preparing the write step."
    );
}
