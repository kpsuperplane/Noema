use super::request::{ResponsesReasoning, ResponsesRequestProfile};
use super::tools::ResponsesToolNameMap;
use super::*;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateRequest, GenerateToolCallInput, GenerateToolResultInput,
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, PromptCacheMode, PromptCacheOptions,
    PromptCacheRetention, PromptCacheTtl, ProviderError, ProviderToolTransport, ReasoningEffort,
    SchemaEnforcement,
};
use serde_json::{Value, json};

mod tool_transport;

#[test]
fn responses_request_profiles_preserve_provider_wire_differences() {
    let request = GenerateRequest {
        conversation_id: Some(" conversation:cacheable ".to_string()),
        instructions: Some("Be brief.".to_string()),
        options: GenerateOptions {
            max_output_tokens: Some(32),
            prompt_cache_retention: Some(PromptCacheRetention::TwentyFourHours),
            ..GenerateOptions::default()
        },
        tools: vec![test_tool().into()],
        tool_transport: ProviderToolTransport::Native,
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
    assert_eq!(openai["reasoning"]["summary"], "auto");

    assert_eq!(codex["input"][0]["role"], "user");
    assert_eq!(codex["input"][0]["content"], "hi");
    assert!(codex.get("max_output_tokens").is_none());
    assert!(codex.get("prompt_cache_retention").is_none());
    assert!(codex.get("include").is_none());
    assert_eq!(codex["stream"], true);
    assert_eq!(codex["reasoning"]["effort"], "high");
    assert_eq!(codex["reasoning"]["summary"], "auto");

    assert!(openai.get("text").is_none());
    assert!(codex.get("text").is_none());

    for value in [&openai, &codex] {
        assert_eq!(value["instructions"], "Be brief.");
        assert_eq!(value["tools"][0]["name"], "search_memory");
        assert_eq!(value["tool_choice"], "required");
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["prompt_cache_key"], "conversation:cacheable");
        assert_eq!(value["store"], false);
    }
}

#[test]
fn hosted_web_search_serializes_beside_configured_functions() {
    let request = GenerateRequest {
        options: GenerateOptions {
            hosted_web_search: true,
            ..GenerateOptions::default()
        },
        tools: vec![test_tool().into()],
        tool_transport: ProviderToolTransport::Native,
        tool_choice: NoemaToolChoice::Allowed(NoemaAllowedTools {
            mode: NoemaAllowedToolsMode::Auto,
            tools: vec![noema_capabilities::ToolName::new("search_memory").expect("tool name")],
        }),
        ..GenerateRequest::text("research")
    };

    for profile in [OPENAI_RESPONSES_PROFILE, CODEX_RESPONSES_PROFILE] {
        let value = lowered_json(&request, "gpt-test", None, profile);
        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["name"], "search_memory");
        assert_eq!(value["tools"][1]["type"], "web_search");
        assert_eq!(value["tools"][1]["external_web_access"], true);
        assert_eq!(value["tool_choice"], "auto");
    }
}

#[test]
fn responses_tools_omit_lookaround_patterns_without_relaxing_other_patterns() {
    let tool = noema_capabilities::ToolSpec::new(
        "mcp.dex.create_calendar_event",
        "Create a calendar event.",
        json!({
            "type": "object",
            "properties": {
                "attendees": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "pattern": r"^(?!\.)(?!.*\.\.)([A-Za-z0-9_'+\-\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$"
                    }
                },
                "title": {"type": "string", "pattern": r".*\S.*"}
            }
        }),
    )
    .expect("tool");
    let request = GenerateRequest {
        tools: vec![tool.into()],
        tool_transport: ProviderToolTransport::Native,
        ..GenerateRequest::text("hi")
    };

    let value = lowered_json(&request, "gpt-codex", None, CODEX_RESPONSES_PROFILE);
    let properties = &value["tools"][0]["parameters"]["properties"];
    assert!(properties["attendees"]["items"].get("pattern").is_none());
    assert_eq!(properties["title"]["pattern"], r".*\S.*");
}

#[test]
fn openai_profile_serializes_allowed_tools_with_provider_safe_names() {
    let tools = crate::expose_provider_tools(
        vec![test_tool(), test_tool_named("mcp.docs:read")],
        ProviderToolTransport::Native,
        crate::ProviderToolSchemaDialect::OpenAiResponses,
    );
    let request = GenerateRequest {
        tools,
        tool_transport: ProviderToolTransport::Native,
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
    assert_eq!(value["tool_choice"]["tools"][0]["name"], "docs_x3a_read");
}

#[test]
fn allowed_tools_are_profile_gated_and_must_reference_the_catalog() {
    let mut request = GenerateRequest {
        tools: vec![test_tool().into()],
        tool_transport: ProviderToolTransport::Native,
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
fn direct_responses_profiles_preserve_cache_controls() {
    let request = GenerateRequest {
        input: GenerateInput::Messages(vec![
            GenerateMessage {
                role: GenerateMessageRole::System,
                content: "   ".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "Stable </noema_application_context> & <context>".to_string(),
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
    assert_eq!(openai["input"][1]["content"], "Environment revision 8");
    assert_eq!(openai["input"][2]["content"], "What changed?");
    assert!(codex.get("prompt_cache_options").is_none());
    assert_eq!(codex["input"][0]["role"], "developer");
    assert_eq!(
        codex["input"][0]["content"],
        "Stable </noema_application_context> & <context>"
    );
    assert_eq!(codex["input"][1]["content"], "Environment revision 8");
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
    let (body, _, _) = ResponsesRequest::from_generate(
        &request,
        "gpt-test".into(),
        Some(ReasoningEffort::Low),
        OPENAI_RESPONSES_PROFILE,
    )
    .expect("request reasoning wins");
    assert!(matches!(
        body.reasoning,
        Some(ResponsesReasoning {
            effort: ReasoningEffort::Medium,
            summary: Some("auto"),
        })
    ));

    request.options.reasoning_effort = Some(ReasoningEffort::None);
    let value = lowered_json(&request, "gpt-test", None, OPENAI_RESPONSES_PROFILE);
    assert!(value["reasoning"].get("summary").is_none());

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
fn whole_catalog_lowering_is_stable() {
    let tools = crate::expose_provider_tools(
        vec![
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
        ProviderToolTransport::Native,
        crate::ProviderToolSchemaDialect::OpenAiResponses,
    );
    let request = GenerateRequest {
        tools,
        tool_transport: ProviderToolTransport::Native,
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
}

#[test]
fn responses_finalize_native_final_without_calls_accepts_plain_text() {
    let response: ResponsesResponse = serde_json::from_value(json!({
        "id": "resp_1",
        "model": "gpt-test",
        "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "Done."}]
        }]
    }))
    .expect("response");
    let diagnostics = ResponsesDiagnosticContext::new(None, "test", "gpt-test", None);
    let response = response
        .finalize(
            &test_tool_names(),
            ProviderToolTransport::Native,
            &diagnostics,
        )
        .expect("native-capable final response");

    assert_eq!(response.assistant_text(), "Done.");
    assert!(response.tool_calls.is_empty());
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
fn responses_response_parses_function_call_output_items() {
    let tools = crate::expose_provider_tools(
        vec![test_tool_named("mcp.docs:read")],
        ProviderToolTransport::Native,
        crate::ProviderToolSchemaDialect::OpenAiResponses,
    );
    let tool_names =
        ResponsesToolNameMap::from_tools_with_enforcement(&tools, SchemaEnforcement::BestEffort)
            .expect("tool names");
    let response = response_with_call(
        Some("call_1"),
        "docs_x3a_read",
        "{\"document_id\":\"doc_1\"}",
    );
    let calls = response
        .native_tool_calls_with_names(&tool_names)
        .expect("tool calls");

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id.as_deref(), Some("item_1"));
    assert_eq!(calls[0].provider_call_id.as_deref(), Some("call_1"));
    assert_eq!(calls[0].provider_name.as_deref(), Some("docs_x3a_read"));
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
        provider_details: None,
    })]);

    let ResponsesInput::Items(items) = ResponsesInput::from(&input) else {
        panic!("expected items");
    };
    let value = serde_json::to_value(&items[0]).expect("json");
    assert_eq!(value["type"], "reasoning");
    assert_eq!(value["id"], "rs_1");
    assert_eq!(value["summary"], json!([]));
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
    assert_eq!(value[0]["name"], "dex_x3a_search_x20_contacts");
}

#[test]
fn responses_tool_name_map_disambiguates_only_actual_alias_collisions() {
    let tools = crate::expose_provider_tools(
        vec![
            test_tool_named("mcp.mcp_server:alpha.dex_list_contacts"),
            test_tool_named("mcp.mcp_server:bravo.dex_list_contacts"),
        ],
        ProviderToolTransport::Native,
        crate::ProviderToolSchemaDialect::OpenAiResponses,
    );
    let names =
        ResponsesToolNameMap::from_tools_with_enforcement(&tools, SchemaEnforcement::BestEffort)
            .expect("colliding leaf names are disambiguated");
    let wire_names = names
        .tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .collect::<Vec<_>>();

    assert_ne!(wire_names[0], wire_names[1]);
    assert!(
        wire_names
            .iter()
            .all(|name| name.starts_with("dex_list_contacts_") && name.len() == 19)
    );
}

#[test]
fn long_mcp_tool_names_keep_the_callable_name_on_the_provider_wire() {
    let canonical = "mcp.mcp_server:5d8a417997dd2905574a27fe7c3a3afa.dex_list_contacts";
    let request = GenerateRequest {
        tools: crate::expose_provider_tools(
            vec![test_tool_named(canonical)],
            ProviderToolTransport::Native,
            crate::ProviderToolSchemaDialect::OpenAiResponses,
        ),
        tool_transport: ProviderToolTransport::Native,
        ..GenerateRequest::text("hi")
    };

    let value = lowered_json(&request, "gpt-codex", None, CODEX_RESPONSES_PROFILE);
    let provider_name = value["tools"][0]["name"]
        .as_str()
        .expect("provider tool name");

    assert_eq!(provider_name, "dex_list_contacts");
}

#[test]
fn strict_tool_lowering_closes_optional_fields_and_marks_nullable() {
    let tool = noema_capabilities::ToolSpec::new(
        "mcp.docs.read",
        "Read a document.",
        json!({
            "type": "object",
            "properties": {
                "document_id": {"type": "string"},
                "context": {
                    "type": "object",
                    "properties": {"mode": {"type": "string"}},
                    "required": [],
                    "additionalProperties": false
                }
            },
            "required": ["document_id"],
            "additionalProperties": false
        }),
    )
    .expect("tool");
    let names = ResponsesToolNameMap::from_tools_with_enforcement(
        &[tool.into()],
        SchemaEnforcement::Strict,
    )
    .expect("strict lowering");
    let wire = serde_json::to_value(&names.tools[0]).expect("tool wire");
    assert_eq!(wire["strict"], true);
    assert_eq!(
        wire["parameters"]["required"],
        json!(["context", "document_id"])
    );
    assert_eq!(
        wire["parameters"]["properties"]["context"]["type"],
        json!(["object", "null"])
    );
    assert_eq!(
        wire["parameters"]["properties"]["context"]["properties"]["mode"]["type"],
        json!(["string", "null"])
    );
}

#[test]
fn strict_tool_lowering_falls_back_for_unsupported_unique_items() {
    let tool = noema_capabilities::ToolSpec::new(
        "mcp.docs.read",
        "Read a document.",
        json!({
            "type": "object",
            "properties": {"ids": {"type": "array", "uniqueItems": true}},
            "required": ["ids"],
            "additionalProperties": false
        }),
    )
    .expect("tool");
    let names = ResponsesToolNameMap::from_tools_with_enforcement(
        &[tool.into()],
        SchemaEnforcement::Strict,
    )
    .expect("best-effort fallback");
    let wire = serde_json::to_value(&names.tools[0]).expect("tool wire");
    assert_eq!(wire["strict"], false);
    assert_eq!(names.strict_fallbacks.len(), 1);
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
    let (body, _, _) =
        ResponsesRequest::from_generate(request, model.to_string(), reasoning, profile)
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
