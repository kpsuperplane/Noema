use super::request::{ResponsesReasoning, ResponsesRequestProfile};
use super::tools::ResponsesToolNameMap;
use super::*;
use crate::response_support::noema_response_text_format;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateRequest, GenerateToolCallInput, GenerateToolResultInput,
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, PromptCacheMode, PromptCacheOptions,
    PromptCacheRetention, PromptCacheTtl, ProviderError, ReasoningEffort,
};
use serde_json::{Value, json};

#[test]
fn responses_request_profiles_preserve_provider_wire_differences() {
    let request = GenerateRequest {
        conversation_id: Some(" conversation:cacheable ".to_string()),
        instructions: Some("Be brief.".to_string()),
        options: GenerateOptions {
            max_output_tokens: Some(32),
            prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
            require_noema_response: true,
            ..GenerateOptions::default()
        },
        tools: vec![test_tool()],
        tool_choice: NoemaToolChoice::Required,
        parallel_tool_calls: true,
        ..GenerateRequest::text("hi")
    };

    let openai = lowered_json(
        &request,
        "gpt-openai",
        Some(ReasoningEffort::Low),
        OPENAI_RESPONSES_PROFILE,
    );
    let codex = lowered_json(
        &request,
        "gpt-codex",
        Some(ReasoningEffort::High),
        CODEX_RESPONSES_PROFILE,
    );

    assert_eq!(openai["input"], "hi");
    assert_eq!(openai["max_output_tokens"], 32);
    assert_eq!(openai["prompt_cache_retention"], "24h");
    assert_eq!(openai["include"][0], "reasoning.encrypted_content");
    assert!(openai.get("stream").is_none());
    assert_eq!(openai["reasoning"]["effort"], "low");

    assert_eq!(codex["input"][0]["role"], "user");
    assert_eq!(codex["input"][0]["content"], "hi");
    assert!(codex.get("max_output_tokens").is_none());
    assert!(codex.get("prompt_cache_retention").is_none());
    assert!(codex.get("include").is_none());
    assert_eq!(codex["stream"], true);
    assert_eq!(codex["reasoning"]["effort"], "high");

    for value in [&openai, &codex] {
        assert_eq!(value["instructions"], "Be brief.");
        assert_eq!(value["text"]["format"]["name"], "noema_response");
        assert_eq!(value["tools"][0]["name"], "search_memory");
        assert_eq!(value["tool_choice"], "required");
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["prompt_cache_key"], "conversation:cacheable");
        assert_eq!(value["store"], false);
    }
}

#[test]
fn openai_profile_serializes_allowed_tools_with_provider_safe_names() {
    let request = GenerateRequest {
        tools: vec![test_tool(), test_tool_named("mcp.docs:read")],
        tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Required,
            tools: vec![noema_capabilities::ToolName::new("mcp.docs:read").expect("tool name")],
        }),
        ..GenerateRequest::text("hi")
    };

    let value = lowered_json(&request, "gpt-openai", None, OPENAI_RESPONSES_PROFILE);
    assert_eq!(value["tools"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["tool_choice"]["type"], "allowed_tools");
    assert_eq!(value["tool_choice"]["mode"], "required");
    assert_eq!(value["tool_choice"]["tools"][0]["type"], "function");
    assert_eq!(
        value["tool_choice"]["tools"][0]["name"],
        "mcp_x2e_docs_x3a_read"
    );
}

#[test]
fn allowed_tools_are_profile_gated_and_must_reference_the_catalog() {
    let mut request = GenerateRequest {
        tools: vec![test_tool()],
        tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Auto,
            tools: vec![noema_capabilities::ToolName::new("search_memory").expect("tool name")],
        }),
        ..GenerateRequest::text("hi")
    };

    let unsupported = ResponsesRequest::from_generate(
        &request,
        "gpt-codex".into(),
        None,
        CODEX_RESPONSES_PROFILE,
    )
    .expect_err("Codex profile rejects allowed tools");
    assert!(
        unsupported
            .to_string()
            .contains("not supported by this provider request profile")
    );

    request.tool_choice = NoemaToolChoice::Allowed(NoemaAllowedTools {
        mode: NoemaAllowedToolsMode::Auto,
        tools: vec![noema_capabilities::ToolName::new("update_own_name").expect("tool name")],
    });
    let missing = ResponsesRequest::from_generate(
        &request,
        "gpt-openai".into(),
        None,
        OPENAI_RESPONSES_PROFILE,
    )
    .expect_err("unknown allowed tool rejected");
    assert!(
        missing
            .to_string()
            .contains("not present in the request tool catalog")
    );
}

#[test]
fn openai_profile_serializes_cache_options_and_developer_message_breakpoints() {
    let request = GenerateRequest {
        input: GenerateInput::Messages(vec![
            GenerateMessage {
                role: GenerateMessageRole::System,
                content: "   ".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "Environment revision 8".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::User,
                content: "What changed?".to_string(),
            },
        ]),
        options: GenerateOptions {
            prompt_cache_options: Some(PromptCacheOptions {
                mode: PromptCacheMode::Explicit,
                ttl: PromptCacheTtl::ThirtyMinutes,
            }),
            prompt_cache_breakpoints: vec![0],
            ..GenerateOptions::default()
        },
        ..GenerateRequest::text("unused")
    };

    let openai = lowered_json(&request, "gpt-openai", None, OPENAI_RESPONSES_PROFILE);
    let codex = lowered_json(&request, "gpt-codex", None, CODEX_RESPONSES_PROFILE);
    assert_eq!(openai["prompt_cache_options"]["mode"], "explicit");
    assert_eq!(openai["prompt_cache_options"]["ttl"], "30m");
    assert_eq!(openai["input"][0]["role"], "developer");
    assert_eq!(openai["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(
        openai["input"][0]["content"][0]["prompt_cache_breakpoint"]["mode"],
        "explicit"
    );
    assert_eq!(openai["input"][1]["content"], "What changed?");
    assert!(codex.get("prompt_cache_options").is_none());
    assert_eq!(codex["input"][0]["role"], "developer");
    assert_eq!(codex["input"][0]["content"], "Environment revision 8");
}

#[test]
fn prompt_cache_breakpoints_reject_invalid_filtered_message_indices() {
    let request = GenerateRequest {
        input: GenerateInput::Messages(vec![GenerateMessage {
            role: GenerateMessageRole::Developer,
            content: "Environment revision 8".to_string(),
        }]),
        options: GenerateOptions {
            prompt_cache_breakpoints: vec![1],
            ..GenerateOptions::default()
        },
        ..GenerateRequest::text("unused")
    };

    let error = ResponsesRequest::from_generate(
        &request,
        "gpt-openai".into(),
        None,
        OPENAI_RESPONSES_PROFILE,
    )
    .expect_err("out-of-range breakpoint rejected");
    assert!(
        error
            .to_string()
            .contains("out of range for 1 filtered messages")
    );
}

#[test]
fn responses_request_reasoning_precedence_and_input_validation_are_shared() {
    let mut request = GenerateRequest::text("hi");
    request.options.reasoning_effort = Some(ReasoningEffort::Medium);
    let (body, _) = ResponsesRequest::from_generate(
        &request,
        "gpt-test".into(),
        Some(ReasoningEffort::Low),
        OPENAI_RESPONSES_PROFILE,
    )
    .expect("request reasoning wins");
    assert!(matches!(
        body.reasoning,
        Some(ResponsesReasoning {
            effort: ReasoningEffort::Medium
        })
    ));

    let error = ResponsesRequest::from_generate(
        &GenerateRequest::text(""),
        "gpt-test".into(),
        None,
        OPENAI_RESPONSES_PROFILE,
    )
    .expect_err("empty input rejected");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
}

#[test]
fn whole_catalog_lowering_and_native_envelope_reconciliation_are_stable() {
    let request = GenerateRequest {
        tools: vec![
            noema_capabilities::web::search::tool_spec().expect("search spec"),
            noema_capabilities::web::fetch::tool_spec().expect("fetch spec"),
            noema_capabilities::ToolSpec::new(
                "mcp.mcp:docs.read",
                "Read a document.",
                json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
            )
            .expect("MCP spec"),
        ],
        tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Required,
            tools: vec![noema_capabilities::ToolName::new("mcp.mcp:docs.read").expect("tool name")],
        }),
        parallel_tool_calls: true,
        ..GenerateRequest::text("fixture input")
    };
    let expected: Value = serde_json::from_str(include_str!(
        "whole_capability_catalog_lowering_fixture.json"
    ))
    .expect("fixture JSON");

    assert_eq!(
        lowered_json(&request, "gpt-fixture", None, OPENAI_RESPONSES_PROFILE),
        expected
    );

    let response = crate::required_noema_response_from_text_with_native_tool_calls(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"commentary","text":"Searching."}],"tool_calls":[]}"#
            .to_string(),
        vec![crate::GenerateToolCall {
            id: Some("item_1".to_string()),
            provider_call_id: Some("call_1".to_string()),
            provider_name: Some("search_memory".to_string()),
            name: "search_memory".to_string(),
            payload: json!({"query": "trains"}),
        }],
    )
    .expect("native tool response");
    assert_eq!(
        (
            response.response_status,
            response.tool_calls[0].provider_call_id.as_deref()
        ),
        (crate::GenerateResponseStatus::NeedsTools, Some("call_1"))
    );
}

#[test]
fn valid_unknown_provider_tool_name_is_rejected_without_fallback() {
    let tool_names = ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names");
    for (label, call_id, arguments) in [
        ("valid unknown", Some("call_1"), "{}"),
        ("missing call id precedence", None, "{}"),
        (
            "malformed arguments precedence",
            Some("call_1"),
            "{\"query\":",
        ),
        ("non-object arguments precedence", Some("call_1"), "[]"),
    ] {
        let error = response_with_call(call_id, "unadvertised_valid_name", arguments)
            .native_tool_calls_with_names(&tool_names)
            .expect_err(label);
        assert!(
            matches!(error, ProviderError::MalformedResponse { ref message }
                if message == "provider returned an unadvertised tool name"),
            "{label}"
        );
    }
}

#[test]
fn noema_response_text_format_covers_text_and_multiple_choice_contracts() {
    let value = noema_response_text_format();
    let one_of = value["format"]["schema"]["properties"]["responses"]["items"]["oneOf"]
        .as_array()
        .expect("responses items oneOf");
    let text_schema = one_of
        .iter()
        .find(|schema| schema["properties"]["kind"]["enum"] == json!(["text"]))
        .expect("text response schema");

    assert_eq!(text_schema["required"], json!(["kind", "phase", "text"]));
    assert_eq!(text_schema["additionalProperties"], false);
    let schema = one_of
        .iter()
        .find(|schema| schema["properties"]["kind"]["enum"] == json!(["multiple_choice"]))
        .expect("multiple choice response schema");

    assert_eq!(
        schema["required"],
        json!(["kind", "phase", "prompt", "selection_mode", "options"])
    );
    assert_eq!(
        schema["properties"]["selection_mode"]["enum"],
        json!(["pick_one", "pick_many"])
    );
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn responses_response_parses_function_call_output_items() {
    let tool_names =
        ResponsesToolNameMap::from_tools(&[test_tool_named("mcp.docs:read")]).expect("tool names");
    let response = response_with_call(
        Some("call_1"),
        "mcp_x2e_docs_x3a_read",
        "{\"document_id\":\"doc_1\"}",
    );
    let calls = response
        .native_tool_calls_with_names(&tool_names)
        .expect("tool calls");

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id.as_deref(), Some("item_1"));
    assert_eq!(calls[0].provider_call_id.as_deref(), Some("call_1"));
    assert_eq!(
        calls[0].provider_name.as_deref(),
        Some("mcp_x2e_docs_x3a_read")
    );
    assert_eq!(calls[0].name, "mcp.docs:read");
    assert_eq!(calls[0].payload["document_id"], "doc_1");
}

#[test]
fn responses_response_rejects_malformed_native_tool_calls() {
    for (call_id, arguments, message) in [
        (
            Some("call_1"),
            "{\"query\":",
            "failed to parse native tool call arguments for search_memory",
        ),
        (
            Some("call_1"),
            "[]",
            "native tool call arguments for search_memory must be a JSON object",
        ),
        (
            None,
            "{\"query\":\"trains\"}",
            "native tool call search_memory is missing call_id",
        ),
    ] {
        let error = response_with_call(call_id, "search_memory", arguments)
            .native_tool_calls_with_names(&test_tool_names())
            .expect_err("malformed call rejected");
        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        assert!(error.to_string().contains(message));
    }
}

#[test]
fn responses_input_serializes_native_tool_results() {
    let input = GenerateInput::NativeToolResults(vec![GenerateToolResultInput {
        id: Some("item_1".to_string()),
        call_id: "call_1".to_string(),
        name: "mcp.docs:read".to_string(),
        provider_name: Some("mcp_x2e_docs_x3a_read".to_string()),
        arguments: json!({"document_id": "doc_1"}),
        success: true,
        payload: json!({"title": "Docs"}),
    }]);

    let value = serde_json::to_value(ResponsesInput::from(&input)).expect("serialize");
    assert_eq!(value[0]["type"], "function_call");
    assert_eq!(value[0]["id"], "item_1");
    assert_eq!(value[0]["call_id"], "call_1");
    assert_eq!(value[0]["name"], "mcp_x2e_docs_x3a_read");
    let arguments: Value =
        serde_json::from_str(value[0]["arguments"].as_str().expect("arguments string"))
            .expect("arguments json");
    assert_eq!(arguments["document_id"], "doc_1");
    assert_eq!(value[1]["type"], "function_call_output");
    let output: Value = serde_json::from_str(value[1]["output"].as_str().expect("output string"))
        .expect("output json");
    assert_eq!(output["name"], "mcp.docs:read");
    assert_eq!(output["provider_name"], "mcp_x2e_docs_x3a_read");
    assert_eq!(output["success"], true);
    assert_eq!(output["payload"]["title"], "Docs");
}

#[test]
fn chained_response_sends_only_new_tool_outputs() {
    let mut request = GenerateRequest {
        input: GenerateInput::NativeToolResults(vec![GenerateToolResultInput {
            id: Some("item_1".to_string()),
            call_id: "call_1".to_string(),
            name: "search_memory".to_string(),
            provider_name: None,
            arguments: json!({"query": "trains"}),
            success: true,
            payload: json!({"matches": []}),
        }]),
        ..GenerateRequest::text("unused")
    };
    request.options.previous_response_id = Some("resp_previous".to_string());
    request.options.store_response = true;

    let value = lowered_json(&request, "gpt-test", None, OPENAI_RESPONSES_PROFILE);
    assert_eq!(value["previous_response_id"], "resp_previous");
    assert_eq!(value["store"], true);
    assert_eq!(value["input"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["input"][0]["type"], "function_call_output");
    assert_eq!(value["input"][0]["call_id"], "call_1");
}

#[test]
fn serializes_reasoning_history_item_for_replay() {
    let input = GenerateInput::Items(vec![GenerateInputItem::Reasoning(GenerateReasoningInput {
        id: Some("rs_1".to_string()),
        encrypted_content: "opaque-openai-reasoning".to_string(),
    })]);

    let ResponsesInput::Items(items) = ResponsesInput::from(&input) else {
        panic!("expected items");
    };
    let value = serde_json::to_value(&items[0]).expect("json");
    assert_eq!(value["type"], "reasoning");
    assert_eq!(value["id"], "rs_1");
    assert_eq!(value["encrypted_content"], "opaque-openai-reasoning");
}

#[test]
fn responses_input_encodes_unsafe_typed_history_tool_names() {
    let input = GenerateInput::Items(vec![GenerateInputItem::ToolCall(GenerateToolCallInput {
        id: None,
        call_id: "call_1".to_string(),
        name: "mcp.dex:search contacts".to_string(),
        provider_name: None,
        arguments: json!({"query": "Gautam"}),
    })]);
    let value = serde_json::to_value(ResponsesInput::from(&input)).expect("serialize");
    assert_eq!(value[0]["type"], "function_call");
    assert_eq!(value[0]["name"], "mcp_x2e_dex_x3a_search_x20_contacts");
}

#[test]
fn responses_tool_name_map_rejects_provider_safe_name_collisions() {
    let first = test_tool_named("mcp.docs");
    let second = test_tool_named("mcp_x2e_docs");
    let error = ResponsesToolNameMap::from_tools(&[first, second]).expect_err("collision rejected");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(
        error
            .to_string()
            .contains("provider-safe tool name collision")
    );
}

#[test]
fn prompt_cache_key_uses_non_empty_conversation_id() {
    for (conversation_id, expected) in [
        (
            Some(" conversation:cacheable "),
            Some("conversation:cacheable"),
        ),
        (Some("  "), None),
        (None, None),
    ] {
        let request = GenerateRequest {
            conversation_id: conversation_id.map(str::to_string),
            ..GenerateRequest::text("hi")
        };
        let value = lowered_json(&request, "gpt-test", None, OPENAI_RESPONSES_PROFILE);
        assert_eq!(
            value.get("prompt_cache_key").and_then(Value::as_str),
            expected
        );
    }
}

fn lowered_json(
    request: &GenerateRequest,
    model: &str,
    reasoning: Option<ReasoningEffort>,
    profile: ResponsesRequestProfile,
) -> Value {
    let (body, _) = ResponsesRequest::from_generate(request, model.to_string(), reasoning, profile)
        .expect("lowering");
    serde_json::to_value(body).expect("request JSON")
}

fn test_tool_names() -> ResponsesToolNameMap {
    ResponsesToolNameMap::from_tools(&[test_tool()]).expect("tool names")
}

fn test_tool() -> noema_capabilities::ToolSpec {
    test_tool_named("search_memory")
}

fn test_tool_named(name: &str) -> noema_capabilities::ToolSpec {
    noema_capabilities::ToolSpec::new(
        name,
        "Search governed Noema memory.",
        json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
            "additionalProperties": false
        }),
    )
    .expect("tool")
}

fn response_with_call(call_id: Option<&str>, name: &str, arguments: &str) -> ResponsesResponse {
    serde_json::from_value(json!({
        "id": "resp_1",
        "model": "gpt-test",
        "output": [{
            "type": "function_call",
            "id": "item_1",
            "call_id": call_id,
            "name": name,
            "arguments": arguments
        }]
    }))
    .expect("response")
}
