use super::*;
use crate::{GenerateOptions, GenerateRequest, GenerateResponseStatus, ProviderToolTransport};
use serde_json::json;

#[test]
fn responses_schema_and_parser_follow_explicit_tool_transport() {
    let native_request = GenerateRequest {
        options: GenerateOptions {
            require_noema_response: true,
            ..GenerateOptions::default()
        },
        tools: vec![super::test_tool()],
        tool_transport: ProviderToolTransport::Native,
        ..GenerateRequest::text("hi")
    };
    let envelope_request = GenerateRequest {
        tool_transport: ProviderToolTransport::NoemaEnvelope,
        ..native_request.clone()
    };

    let native = super::lowered_json(&native_request, "gpt-test", None, OPENAI_RESPONSES_PROFILE);
    let envelope = super::lowered_json(
        &envelope_request,
        "gpt-test",
        None,
        OPENAI_RESPONSES_PROFILE,
    );
    assert!(
        native["text"]["format"]["schema"]["properties"]
            .get("tool_calls")
            .is_none()
    );
    assert!(
        envelope["text"]["format"]["schema"]["properties"]
            .get("tool_calls")
            .is_some()
    );
    assert_eq!(
        envelope["text"]["format"]["schema"]["required"],
        json!(["response_status", "responses", "tool_calls"])
    );

    let disabled = GenerateRequest {
        tool_transport: ProviderToolTransport::None,
        ..native_request
    };
    let error = ResponsesRequest::from_generate(
        &disabled,
        "gpt-test".into(),
        None,
        OPENAI_RESPONSES_PROFILE,
    )
    .expect_err("disabled transport rejects an advertised tool catalog");
    assert!(error.to_string().contains("tool transport is disabled"));
}

#[test]
fn responses_finalize_without_text_still_honors_tool_transport() {
    let diagnostics = ResponsesDiagnosticContext::new(None, "test", "gpt-test", None);
    let native = super::response_with_call(Some("call_1"), "search_memory", "{}");
    let native = native
        .finalize(
            &super::test_tool_names(),
            ProviderToolTransport::Native,
            true,
            &diagnostics,
        )
        .expect("native call-only response");
    assert_eq!(native.response_status, GenerateResponseStatus::NeedsTools);
    assert_eq!(native.tool_calls.len(), 1);

    let envelope = super::response_with_call(Some("call_1"), "search_memory", "{}");
    let error = envelope
        .finalize(
            &super::test_tool_names(),
            ProviderToolTransport::NoemaEnvelope,
            false,
            &diagnostics,
        )
        .expect_err("envelope transport rejects provider-native calls");
    assert!(
        error
            .to_string()
            .contains("Noema envelope response cannot include provider-native tool calls")
    );
}
