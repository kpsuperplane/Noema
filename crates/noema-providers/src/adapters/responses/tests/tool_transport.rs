use super::*;
use crate::{GenerateRequest, ProviderToolTransport};
use serde_json::json;

#[test]
fn hosted_and_native_requests_omit_legacy_text_format() {
    let native_request = GenerateRequest {
        options: crate::GenerateOptions {
            hosted_web_search: true,
            ..crate::GenerateOptions::default()
        },
        tools: vec![super::test_tool().into()],
        tool_transport: ProviderToolTransport::Native,
        ..GenerateRequest::text("hi")
    };
    let native = super::lowered_json(&native_request, "gpt-test", None, OPENAI_RESPONSES_PROFILE);
    assert!(native.get("text").is_none());
    assert!(native.get("format").is_none());
    assert_eq!(native["tools"][0]["type"], "function");
    assert_eq!(native["tools"][1]["type"], "web_search");

    let plain = super::lowered_json(
        &GenerateRequest::text("hi"),
        "gpt-test",
        None,
        OPENAI_RESPONSES_PROFILE,
    );
    assert!(plain.get("text").is_none());
    assert!(plain.get("format").is_none());
}

#[test]
fn disabled_transport_rejects_an_advertised_tool_catalog() {
    let request = GenerateRequest {
        tools: vec![super::test_tool().into()],
        tool_transport: ProviderToolTransport::None,
        ..GenerateRequest::text("hi")
    };
    let error = ResponsesRequest::from_generate(
        &request,
        "gpt-test".into(),
        None,
        OPENAI_RESPONSES_PROFILE,
    )
    .expect_err("disabled transport rejects an advertised tool catalog");
    assert!(error.to_string().contains("tool transport is disabled"));
}

#[test]
fn native_call_only_response_is_normalized_without_assistant_text() {
    let diagnostics = ResponsesDiagnosticContext::new(None, "test", "gpt-test", None);
    let response = super::response_with_call(Some("call_1"), "search_memory", "{}");
    let response = response
        .finalize(
            &super::test_tool_names(),
            ProviderToolTransport::Native,
            &diagnostics,
        )
        .expect("native call-only response");
    assert!(response.responses.is_empty());
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(
        response.tool_calls[0].provider_call_id.as_deref(),
        Some("call_1")
    );
}

#[test]
fn native_tool_response_uses_native_calls_with_plain_text() {
    let diagnostics = ResponsesDiagnosticContext::new(None, "test", "gpt-test", None);
    let response: ResponsesResponse = serde_json::from_value(json!({
        "id": "resp_1",
        "model": "gpt-test",
        "output": [
            {
                "type": "message",
                "content": [{"type": "output_text", "text": "Checking."}]
            },
            {
                "type": "function_call",
                "id": "item_1",
                "call_id": "call_1",
                "name": "search_memory",
                "arguments": "{}"
            }
        ]
    }))
    .expect("response");

    let response = response
        .finalize(
            &super::test_tool_names(),
            ProviderToolTransport::Native,
            &diagnostics,
        )
        .expect("native response");
    assert_eq!(response.assistant_text(), "Checking.");
    assert_eq!(response.tool_calls.len(), 1);
}

#[test]
fn native_parallel_calls_reject_duplicate_provider_call_ids() {
    let diagnostics = ResponsesDiagnosticContext::new(None, "test", "gpt-test", None);
    let response: ResponsesResponse = serde_json::from_value(json!({
        "id": "resp_1",
        "model": "gpt-test",
        "output": [
            {
                "type": "function_call",
                "id": "item_1",
                "call_id": "call_duplicate",
                "name": "search_memory",
                "arguments": "{}"
            },
            {
                "type": "function_call",
                "id": "item_2",
                "call_id": "call_duplicate",
                "name": "search_memory",
                "arguments": "{}"
            }
        ]
    }))
    .expect("response");

    let error = response
        .finalize(
            &super::test_tool_names(),
            ProviderToolTransport::Native,
            &diagnostics,
        )
        .expect_err("duplicate provider call ids must fail closed");
    assert!(error.to_string().contains("duplicate provider_call_id"));
}
