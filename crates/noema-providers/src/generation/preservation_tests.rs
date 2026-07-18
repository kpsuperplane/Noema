use serde_json::json;

use super::*;

#[test]
fn reasoning_effort_wire_and_persistence_codecs_are_exact() {
    for (effort, stored) in [
        (ReasoningEffort::None, "none"),
        (ReasoningEffort::Minimal, "minimal"),
        (ReasoningEffort::Low, "low"),
        (ReasoningEffort::Medium, "medium"),
        (ReasoningEffort::High, "high"),
        (ReasoningEffort::XHigh, "xhigh"),
    ] {
        assert_eq!(effort.as_persistence_str(), stored);
        assert_eq!(ReasoningEffort::from_persistence_str(stored), Some(effort));
        let encoded = serde_json::to_string(&effort).unwrap();
        assert_eq!(encoded, format!("\"{stored}\""));
        assert_eq!(
            serde_json::from_str::<ReasoningEffort>(&encoded).unwrap(),
            effort
        );
    }
    assert_eq!(ReasoningEffort::from_persistence_str("extreme"), None);
    assert!(serde_json::from_str::<ReasoningEffort>("\"extreme\"").is_err());
}

#[test]
fn output_items_from_text_treats_unrelated_json_as_plain_text() {
    let text =
        r#"{"output":[{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
            .to_string();

    assert_eq!(
        output_items_from_text(text.clone()).unwrap(),
        vec![GenerateResponseItem::Text { phase: None, text }]
    );
}

#[test]
fn required_noema_response_accepts_object_contract_final_text() {
    let response = required_noema_response_from_text(final_response("Done.")).unwrap();

    assert_eq!(response.response_status, GenerateResponseStatus::Final);
    assert_eq!(response.assistant_text(), "Done.");
    assert!(response.tool_calls.is_empty());
}

#[test]
fn required_noema_response_accepts_multiple_silent_tool_calls() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"id":"call_2","name":"mcp.docs.read","payload":{"document_id":"doc_1"}}]}"#
            .to_string(),
    )
    .unwrap();

    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert!(response.responses.is_empty());
    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(response.tool_calls[0].name, "search_memory");
    assert_eq!(response.tool_calls[1].name, "mcp.docs.read");
}

#[test]
fn required_noema_response_accepts_multiple_choice_response() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}],"tool_calls":[]}"#
            .to_string(),
    )
    .unwrap();

    assert!(matches!(
        &response.responses[0],
        GenerateResponseItem::MultipleChoice {
            selection_mode: MultipleChoiceSelectionMode::PickOne,
            options,
            ..
        } if options.len() == 2
    ));
}

#[test]
fn required_noema_response_rejects_invalid_contracts() {
    for (input, message) in [
        (
            r#"{"response_status":"final","responses":[],"tool_calls":[]}"#,
            "Noema final response did not include any response items",
        ),
        (
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
            "Noema final response cannot include tool_calls",
        ),
        (
            r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"a","label":"Also A"}]}],"tool_calls":[]}"#,
            "Noema multiple_choice response option ids must be unique",
        ),
        (
            r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"}]}],"tool_calls":[]}"#,
            "Noema multiple_choice response must include at least two options",
        ),
        (
            r#"{"response_status":"needs_tools","responses":[{"kind":"multiple_choice","phase":"commentary","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
            "Noema needs_tools response cannot include multiple_choice items",
        ),
        (
            r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
            "Noema needs_tools response cannot include final_answer text",
        ),
        (
            "No. I did not call the tool.",
            "provider did not return a Noema structured response object",
        ),
    ] {
        assert_malformed_message(input, message);
    }

    assert!(
        required_noema_response_from_text(
            r#"{"output":[{"kind":"assistant_text","text":"Hello"}]}"#.to_string(),
        )
        .is_err()
    );
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#
            .to_string(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("memory_proposals"));
}

#[test]
fn required_noema_response_recovers_one_embedded_object() {
    let response = required_noema_response_from_text(
        r#"Searching now.{"response_status":"needs_tools","responses":[{"kind":"text","phase":"commentary","text":"Searching now."}],"tool_calls":[{"id":"call_1","name":"mcp.dex.search","payload":{"query":"Gautam"}}]}"#
            .to_string(),
    )
    .unwrap();

    assert_eq!(response.assistant_text(), "Searching now.");
    assert_eq!(response.tool_calls[0].name, "mcp.dex.search");
    let response =
        required_noema_response_from_text(format!("{} trailing prose", final_response("Hello")))
            .unwrap();

    assert_eq!(response.assistant_text(), "Hello");
}

#[test]
fn required_noema_response_deduplicates_exact_stream_objects_and_rejects_conflicts() {
    let duplicate = final_response("Same.");
    let response = required_noema_response_from_text(format!("{duplicate}{duplicate}")).unwrap();

    assert_eq!(response.assistant_text(), "Same.");
    let error = required_noema_response_from_text(format!(
        "{}{}",
        final_response("One."),
        final_response("Two.")
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "provider returned multiple Noema structured response objects"
    ));
}

#[test]
fn action_items_serialize_tool_result_shape() {
    let item = GenerateActionItem::ToolResult {
        call_id: Some("call_1".to_string()),
        provider_call_id: Some("provider_call_1".to_string()),
        provider_name: Some("search_memory".to_string()),
        name: Some("search_memory".to_string()),
        success: Some(true),
        payload: json!({"ok": true}),
    };

    assert_eq!(
        serde_json::to_value(item).unwrap(),
        json!({
            "kind": "tool_result",
            "call_id": "call_1",
            "provider_call_id": "provider_call_1",
            "provider_name": "search_memory",
            "name": "search_memory",
            "success": true,
            "payload": {"ok": true}
        })
    );
}

fn final_response(text: &str) -> String {
    json!({
        "response_status": "final",
        "responses": [{"kind": "text", "phase": "final_answer", "text": text}],
        "tool_calls": []
    })
    .to_string()
}

fn assert_malformed_message(input: &str, expected: &str) {
    let error = required_noema_response_from_text(input.to_string()).unwrap_err();
    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message } if message == expected
    ));
}
