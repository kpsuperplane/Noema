use serde_json::json;

use super::*;

struct EchoProvider;

impl ModelProvider for EchoProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        Ok(GenerateResponse {
            responses: vec![GenerateResponseItem::Text {
                phase: None,
                text: request.input.render_for_token_count(),
            }],
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::Final,
            provider: "mock".to_string(),
            model: request.model.unwrap_or_else(|| "mock-model".to_string()),
            response_id: Some("mock-response".to_string()),
            usage: None,
        })
    }
}

#[test]
fn reasoning_effort_serializes_lowercase_api_values() {
    for (effort, expected) in [
        (ReasoningEffort::None, "\"none\""),
        (ReasoningEffort::Minimal, "\"minimal\""),
        (ReasoningEffort::Low, "\"low\""),
        (ReasoningEffort::Medium, "\"medium\""),
        (ReasoningEffort::High, "\"high\""),
        (ReasoningEffort::XHigh, "\"xhigh\""),
    ] {
        assert_eq!(serde_json::to_string(&effort).unwrap(), expected);
    }
}

#[test]
fn reasoning_effort_deserializes_lowercase_api_values() {
    assert_eq!(
        serde_json::from_str::<ReasoningEffort>("\"xhigh\"").unwrap(),
        ReasoningEffort::XHigh
    );
    assert!(serde_json::from_str::<ReasoningEffort>("\"extreme\"").is_err());
}

#[test]
fn reasoning_effort_persistence_values_round_trip_exactly() {
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
    }
    assert_eq!(ReasoningEffort::from_persistence_str("extreme"), None);
}

#[tokio::test]
async fn model_provider_default_streaming_delegates_to_generate_without_events() {
    let provider = EchoProvider;
    let mut events = Vec::new();
    let response = {
        let mut on_event = |event| events.push(event);
        provider
            .generate_streaming(GenerateRequest::text("hello stream"), &mut on_event)
            .await
            .unwrap()
    };

    assert_eq!(response.assistant_text(), "hello stream");
    assert_eq!(events, Vec::<GenerateStreamEvent>::new());
}

#[test]
fn default_provider_context_metadata_is_unknown() {
    let provider = EchoProvider;

    assert_eq!(provider.context_metadata(None).context_window_tokens, None);
    assert_eq!(
        provider
            .context_metadata(Some("mock"))
            .default_output_reserve_tokens,
        None
    );
}

#[test]
fn generate_tool_call_preserves_provider_call_id_separately() {
    let call = GenerateToolCall {
        id: Some("item_1".to_string()),
        provider_call_id: Some("call_1".to_string()),
        provider_name: Some("search_memory".to_string()),
        name: "search_memory".to_string(),
        payload: json!({"query": "trains"}),
    };

    assert_eq!(call.id.as_deref(), Some("item_1"));
    assert_eq!(call.provider_call_id.as_deref(), Some("call_1"));
}

#[test]
fn output_items_from_text_parses_response_object_responses() {
    let output = output_items_from_text(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[]}"#
            .to_string(),
    )
    .unwrap();

    assert_eq!(
        output,
        vec![GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            text: "Hello".to_string(),
        }]
    );
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
fn required_noema_response_rejects_old_output_array_contract() {
    assert!(
        required_noema_response_from_text(
            r#"{"output":[{"kind":"assistant_text","text":"Hello"}]}"#.to_string(),
        )
        .is_err()
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
fn noema_response_no_longer_requires_memory_proposals() {
    let response = required_noema_response_from_text(final_response("ok")).unwrap();

    assert_eq!(response.responses.len(), 1);
    assert!(response.tool_calls.is_empty());
}

#[test]
fn noema_response_rejects_memory_proposals_field() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(error.to_string().contains("memory_proposals"));
}

#[test]
fn required_noema_response_accepts_silent_tool_calls() {
    let response = required_noema_response_from_text(single_tool_response()).unwrap();

    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert!(response.responses.is_empty());
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(response.tool_calls[0].name, "search_memory");
}

#[test]
fn required_noema_response_accepts_multiple_silent_tool_calls() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"id":"call_2","name":"mcp.docs.read","payload":{"document_id":"doc_1"}}]}"#
            .to_string(),
    )
    .unwrap();

    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[1].name, "mcp.docs.read");
}

#[test]
fn required_noema_response_rejects_memory_proposals() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[],"memory_proposals":[{"content":"secret"}]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(error.to_string().contains("memory_proposals"));
}

#[test]
fn assistant_text_phase_defaults_to_final_without_runtime_tools() {
    assert_eq!(
        AssistantTextPhase::effective_for_response_item(
            &GenerateResponseItem::Text {
                phase: None,
                text: "Done.".to_string(),
            },
            false,
        ),
        AssistantTextPhase::FinalAnswer
    );
}

#[test]
fn assistant_text_phase_defaults_to_commentary_with_runtime_tools() {
    assert_eq!(
        AssistantTextPhase::effective_for_response_item(
            &GenerateResponseItem::Text {
                phase: None,
                text: "Checking that now.".to_string(),
            },
            true,
        ),
        AssistantTextPhase::Commentary
    );
}

#[test]
fn required_noema_response_rejects_final_without_responses() {
    assert_malformed_message(
        r#"{"response_status":"final","responses":[],"tool_calls":[]}"#,
        "Noema final response did not include any response items",
    );
}

#[test]
fn required_noema_response_rejects_tool_calls_in_final_response() {
    assert_malformed_message(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
        "Noema final response cannot include tool_calls",
    );
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
fn required_noema_response_rejects_duplicate_multiple_choice_option_ids() {
    assert_malformed_message(
        r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"a","label":"Also A"}]}],"tool_calls":[]}"#,
        "Noema multiple_choice response option ids must be unique",
    );
}

#[test]
fn required_noema_response_rejects_multiple_choice_with_too_few_options() {
    assert_malformed_message(
        r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"}]}],"tool_calls":[]}"#,
        "Noema multiple_choice response must include at least two options",
    );
}

#[test]
fn required_noema_response_rejects_multiple_choice_in_needs_tools_response() {
    assert_malformed_message(
        r#"{"response_status":"needs_tools","responses":[{"kind":"multiple_choice","phase":"commentary","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
        "Noema needs_tools response cannot include multiple_choice items",
    );
}

#[test]
fn required_noema_response_rejects_final_answer_in_needs_tools_response() {
    assert_malformed_message(
        r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#,
        "Noema needs_tools response cannot include final_answer text",
    );
}

#[test]
fn required_noema_response_rejects_plain_text() {
    assert_malformed_message(
        "No. I did not call the tool.",
        "provider did not return a Noema structured response object",
    );
}

#[test]
fn required_noema_response_recovers_object_after_leading_prose() {
    let response = required_noema_response_from_text(
        r#"Searching now.{"response_status":"needs_tools","responses":[{"kind":"text","phase":"commentary","text":"Searching now."}],"tool_calls":[{"id":"call_1","name":"mcp.dex.search","payload":{"query":"Gautam"}}]}"#
            .to_string(),
    )
    .unwrap();

    assert_eq!(response.assistant_text(), "Searching now.");
    assert_eq!(response.tool_calls[0].name, "mcp.dex.search");
}

#[test]
fn required_noema_response_recovers_object_before_trailing_prose() {
    let response =
        required_noema_response_from_text(format!("{} trailing prose", final_response("Hello")))
            .unwrap();

    assert_eq!(response.assistant_text(), "Hello");
}

#[test]
fn required_noema_response_accepts_exact_concatenated_stream_duplicate() {
    let duplicate = final_response("Same.");
    let response = required_noema_response_from_text(format!("{duplicate}{duplicate}")).unwrap();

    assert_eq!(response.assistant_text(), "Same.");
}

#[test]
fn required_noema_response_rejects_conflicting_concatenated_stream_responses() {
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

fn single_tool_response() -> String {
    json!({
        "response_status": "needs_tools",
        "responses": [],
        "tool_calls": [{
            "id": "call_1",
            "name": "search_memory",
            "payload": {"query": "trains"}
        }]
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
