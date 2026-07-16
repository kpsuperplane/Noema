use serde_json::json;

use super::*;

struct EchoProvider;

impl ModelProvider for EchoProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let text = request.input.render_for_token_count();
        Ok(GenerateResponse {
            responses: vec![GenerateResponseItem::Text { phase: None, text }],
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::Final,
            provider: "mock".to_string(),
            model: request.model.unwrap_or_else(|| "mock-model".to_string()),
            response_id: Some("mock-response".to_string()),
            usage: Some(TokenUsage {
                input_tokens: 1,
                output_tokens: 1,
                total_tokens: 2,
                cached_input_tokens: None,
            }),
        })
    }
}

#[tokio::test]
async fn model_provider_contract_can_be_implemented_by_a_mock() {
    let provider = EchoProvider;
    let response = provider
        .generate(GenerateRequest::text("hello").with_model("mock-1"))
        .await
        .expect("mock provider should return a response");

    assert_eq!(response.assistant_text(), "hello");
    assert_eq!(response.provider, "mock");
    assert_eq!(response.model, "mock-1");
}

#[tokio::test]
async fn default_streaming_delegates_to_generate_without_events() {
    let provider = EchoProvider;
    let mut events = Vec::new();
    let response = {
        let mut on_event = |event| events.push(event);
        provider
            .generate_streaming(GenerateRequest::text("hello stream"), &mut on_event)
            .await
            .expect("mock provider should return a streaming response")
    };

    assert_eq!(response.assistant_text(), "hello stream");
    assert!(events.is_empty());
}

#[test]
fn reasoning_effort_codecs_round_trip_exactly() {
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
        assert_eq!(
            serde_json::from_str::<ReasoningEffort>(
                &serde_json::to_string(&effort).expect("serialize effort")
            )
            .expect("deserialize effort"),
            effort
        );
    }
    assert_eq!(ReasoningEffort::from_persistence_str("future"), None);
}

#[test]
fn output_items_from_text_preserves_unrelated_json_as_text() {
    let text = r#"{"output":[{"kind":"tool_call","name":"search_memory"}]}"#.to_string();
    let output = output_items_from_text(text.clone()).expect("plain assistant text");

    assert_eq!(
        output,
        vec![GenerateResponseItem::Text { phase: None, text }]
    );
}

#[test]
fn required_response_accepts_final_text() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[]}"#
            .to_string(),
    )
    .expect("required structured response");

    assert_eq!(response.response_status, GenerateResponseStatus::Final);
    assert_eq!(response.assistant_text(), "Done.");
    assert!(response.tool_calls.is_empty());
}

#[test]
fn required_response_accepts_silent_tool_calls() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
            .to_string(),
    )
    .expect("silent tool call response");

    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert!(response.responses.is_empty());
    assert_eq!(response.tool_calls[0].name, "search_memory");
}

#[test]
fn required_response_rejects_legacy_memory_proposals() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#
            .to_string(),
    )
    .expect_err("legacy field rejected");

    assert!(error.to_string().contains("memory_proposals"));
}

#[test]
fn final_response_requires_non_empty_response_items() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[],"tool_calls":[]}"#.to_string(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "Noema final response did not include any response items"
    ));
}

#[test]
fn needs_tools_rejects_final_answer_text() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "Noema needs_tools response cannot include final_answer text"
    ));
}

#[test]
fn multiple_choice_contract_validates_option_identity() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}],"tool_calls":[]}"#
            .to_string(),
    )
    .expect("multiple choice response");
    assert!(matches!(
        &response.responses[0],
        GenerateResponseItem::MultipleChoice { options, .. } if options.len() == 2
    ));

    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"a","label":"Again"}]}],"tool_calls":[]}"#
            .to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "Noema multiple_choice response option ids must be unique"
    ));
}

#[test]
fn parser_recovers_one_embedded_response_and_rejects_conflicts() {
    let response = required_noema_response_from_text(
        r#"leading {"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[]} trailing"#
            .to_string(),
    )
    .expect("embedded response");
    assert_eq!(response.assistant_text(), "Hello");

    let first = r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"One."}],"tool_calls":[]}"#;
    let second = r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Two."}],"tool_calls":[]}"#;
    let error = required_noema_response_from_text(format!("{first}{second}")).unwrap_err();
    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "provider returned multiple Noema structured response objects"
    ));
}

#[test]
fn native_tool_calls_replace_an_empty_envelope_channel() {
    let native_call = GenerateToolCall {
        id: Some("item_1".to_string()),
        provider_call_id: Some("call_1".to_string()),
        provider_name: Some("search_memory".to_string()),
        name: "search_memory".to_string(),
        payload: json!({"query": "trains"}),
    };
    let response = required_noema_response_from_text_with_native_tool_calls(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"commentary","text":"Searching."}],"tool_calls":[]}"#
            .to_string(),
        vec![native_call],
    )
    .expect("native tool response");

    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert_eq!(
        response.tool_calls[0].provider_call_id.as_deref(),
        Some("call_1")
    );
}

#[test]
fn action_items_preserve_provider_tool_identity() {
    let item = GenerateActionItem::ToolResult {
        call_id: Some("call_1".to_string()),
        provider_call_id: Some("provider_call_1".to_string()),
        provider_name: Some("search_memory".to_string()),
        name: Some("search_memory".to_string()),
        success: Some(true),
        payload: json!({"ok": true}),
    };

    assert_eq!(
        serde_json::to_value(item).expect("json"),
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
