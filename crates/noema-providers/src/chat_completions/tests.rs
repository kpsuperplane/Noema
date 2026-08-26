use super::output::ChatCompletionResponse;
use super::request::{ChatCompletionRequest, ChatTool, OpenAiToolNameMap};
use super::sse::ChatSseAccumulator;
use crate::response_support::StructuredResponseDiagnosticContext;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateRequest, GenerateToolCallInput, ProviderSchemaRequest,
    ProviderSchemaRequestCapabilities, ProviderToolTransport, ReasoningEffort,
};
use serde_json::json;

#[test]
fn request_lowering_preserves_chat_fields_and_openrouter_application_boundary() {
    let tool = noema_capabilities::ToolSpec::new(
        "search_memory",
        "Search governed Noema memory.",
        json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
            "additionalProperties": false
        }),
    )
    .expect("tool");
    let reasoning_details = vec![json!({
        "type": "reasoning.encrypted",
        "id": "rs_1",
        "data": "opaque"
    })];
    let request = GenerateRequest {
        conversation_id: Some(" conversation:cacheable ".to_string()),
        input: GenerateInput::Items(vec![
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "Stable </noema_application_context> & <context>".to_string(),
            }),
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::User,
                content: "Human <input> stays raw".to_string(),
            }),
            GenerateInputItem::Reasoning(GenerateReasoningInput {
                id: Some("rs_1".to_string()),
                encrypted_content: "opaque".to_string(),
                provider_details: Some(reasoning_details.clone()),
            }),
            GenerateInputItem::Message(GenerateMessage {
                role: GenerateMessageRole::Assistant,
                content: "Working.".to_string(),
            }),
            GenerateInputItem::ToolCall(GenerateToolCallInput {
                id: Some("item_1".to_string()),
                call_id: "call_1".to_string(),
                name: "search_memory".to_string(),
                provider_name: None,
                arguments: json!({"query": "trains"}),
            }),
        ]),
        instructions: Some("Stable instructions".to_string()),
        options: GenerateOptions {
            max_output_tokens: Some(64),
            temperature: Some(0.2),
            reasoning_effort: Some(ReasoningEffort::High),
            hosted_web_search: true,
            ..GenerateOptions::default()
        },
        tools: vec![crate::ProviderTool::canonical(tool)],
        tool_transport: ProviderToolTransport::Native,
        parallel_tool_calls: true,
        ..GenerateRequest::text("unused")
    };

    let (mut body, names, transport) =
        ChatCompletionRequest::from_generate_with_schema_request_capabilities(
            &request,
            "anthropic/claude-haiku-4.5".to_string(),
            None,
            ProviderSchemaRequestCapabilities::request_strict_when_possible(),
        )
        .expect("chat lowering");
    body.prompt_cache_key = request.conversation_id.clone();
    body.cache_control = Some(json!({"type": "ephemeral"}));
    body.tools.push(ChatTool::openrouter_web_search());
    body.parallel_tool_calls = Some(true);
    crate::adapters::openrouter::adapt_openrouter_request(&mut body);

    let value = serde_json::to_value(body).expect("body json");
    assert_eq!(transport, ProviderToolTransport::Native);
    assert_eq!(value["model"], "anthropic/claude-haiku-4.5");
    assert_eq!(value["max_completion_tokens"], 64);
    assert!((value["temperature"].as_f64().unwrap_or_default() - 0.2).abs() < 0.00001);
    assert_eq!(value["reasoning"]["effort"], "high");
    assert_eq!(value["prompt_cache_key"], " conversation:cacheable ");
    assert_eq!(value["cache_control"]["type"], "ephemeral");
    assert_eq!(value["stream"], true);
    assert_eq!(value["stream_options"]["include_usage"], true);
    assert_eq!(value["messages"][0]["role"], "system");
    assert!(
        value["messages"][0]["content"]
            .as_str()
            .is_some_and(|text| text.contains("Stable instructions")
                && text.contains("<noema_application_context>"))
    );
    assert_eq!(value["messages"][1]["role"], "user");
    assert_eq!(
        value["messages"][1]["content"],
        "<noema_application_context>\nStable &lt;/noema_application_context&gt; &amp; &lt;context&gt;\n</noema_application_context>"
    );
    assert_eq!(value["messages"][2]["content"], "Human <input> stays raw");
    assert_eq!(value["messages"][3]["role"], "assistant");
    assert_eq!(
        value["messages"][3]["reasoning_details"],
        json!(reasoning_details)
    );
    assert_eq!(value["messages"][3]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        value["messages"][3]["tool_calls"][0]["function"]["name"],
        "search_memory"
    );
    assert_eq!(value["tools"][0]["type"], "function");
    assert_eq!(value["tools"][0]["function"]["name"], "search_memory");
    assert_eq!(value["tools"][1]["type"], "openrouter:web_search");
    assert!(value.get("input").is_none());
    assert!(value.get("instructions").is_none());
    assert!(value.get("store").is_none());
    assert!(value.get("previous_response_id").is_none());

    assert_eq!(names.canonical_name("search_memory"), Some("search_memory"));
}

#[test]
fn openrouter_schema_request_wire_is_stable_for_named_and_auto_models() {
    for model in ["anthropic/claude-haiku-4.5", "openrouter/auto"] {
        for (schema, expected_parameters, strict) in [
            (
                json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}),
                json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}),
                true,
            ),
            (
                json!({"type":"object","properties":{"ids":{"type":"array","items":{"type":"string"},"uniqueItems":true}},"required":["ids"],"additionalProperties":false}),
                json!({"type":"object","properties":{"ids":{"type":"array","items":{"type":"string"},"uniqueItems":true}},"required":["ids"],"additionalProperties":false}),
                false,
            ),
        ] {
            let tool = noema_capabilities::ToolSpec::new("search_memory", "Search.", schema)
                .expect("tool");
            let request = GenerateRequest {
                model: Some(model.to_string()),
                tools: vec![crate::ProviderTool::canonical(tool)],
                tool_transport: ProviderToolTransport::Native,
                ..GenerateRequest::text("search")
            };
            let (body, _, _) =
                ChatCompletionRequest::from_generate_with_schema_request_capabilities(
                    &request,
                    model.to_string(),
                    None,
                    ProviderSchemaRequestCapabilities::request_strict_when_possible(),
                )
                .expect("OpenRouter request");
            let wire = serde_json::to_value(body).expect("request JSON");
            assert_eq!(wire["model"], model);
            assert_eq!(
                wire["tools"],
                json!([{
                    "type":"function",
                    "function":{
                        "name":"search_memory",
                        "description":"Search.",
                        "parameters":expected_parameters,
                        "strict":strict
                    }
                }])
            );
        }
    }
}

#[test]
fn strict_provider_output_returns_to_nested_source_form() {
    let tool = noema_capabilities::ToolSpec::new(
        "mcp.docs.read",
        "Read.",
        json!({
            "type":"object",
            "properties":{
                "document_id":{"type":"string"},
                "context":{
                    "type":"object",
                    "properties":{
                        "mode":{"type":"string"},
                        "nullable":{"type":["string","null"]}
                    },
                    "additionalProperties":false
                }
            },
            "required":["document_id"],
            "additionalProperties":false
        }),
    )
    .expect("tool");
    let names = OpenAiToolNameMap::from_tools_with_request(
        &[crate::ProviderTool::canonical(tool)],
        ProviderSchemaRequest::RequestStrictWhenPossible,
    )
    .expect("tool names");
    let response: ChatCompletionResponse = serde_json::from_value(json!({
        "id":"chat_1",
        "model":"openrouter/auto",
        "choices":[{"message":{"content":null,"tool_calls":[{
            "id":"call_1",
            "function":{
                "name":"mcp.docs.read",
                "arguments":"{\"document_id\":7,\"context\":{\"mode\":null,\"nullable\":null}}"
            }
        }]}}]
    }))
    .expect("provider response");
    let diagnostics = StructuredResponseDiagnosticContext::new(None, "openrouter", "auto", None);
    let normalized = response
        .finalize(
            &names,
            ProviderToolTransport::Native,
            &diagnostics.provider_kind,
            &diagnostics.model,
        )
        .expect("normalized response");
    assert_eq!(
        normalized.tool_calls[0].payload,
        json!({"document_id":7,"context":{"nullable":null}})
    );
}

#[test]
fn stream_normalization_assembles_text_tools_reasoning_citations_search_and_usage() {
    let diagnostics = StructuredResponseDiagnosticContext::new(None, "openrouter", "test", None);
    let mut events = Vec::new();
    let mut accumulator = ChatSseAccumulator::new();
    accumulator
        .push_bytes(
            br#": keepalive

data: {"id":"chat_1","model":"anthropic/claude-haiku-4.5","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel","reasoning_details":[{"type":"reasoning.encrypted","id":"rs_1","data":"opa"}]}}]}

data: {"choices":[{"index":0,"delta":{"content":"lo","tool_calls":[{"index":0,"type":"function","function":{"name":"search_memory","arguments":"{\"query\":\""}}]}}]}

"#,
            &mut |event| events.push(event),
        )
        .expect("first stream chunk");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, crate::GenerateStreamEvent::ToolCallStarted { .. }))
    );
    accumulator
        .push_bytes(
            br#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"arguments":"trains\"}"}}],"reasoning_details":[{"type":"reasoning.encrypted","id":"rs_1","data":"que"},{"type":"reasoning.summary","text":"Searching"},{"type":"reasoning.server_tool_call","id":"ws_1","name":"web.search","status":"completed","arguments":{"query":"trains"},"result":{"sources":1}},{"type":"reasoning.server_tool_call","id":"ws_2","name":"web.search","status":"completed","arguments":{"query":"stations"},"result":{"sources":2}}],"annotations":[{"type":"url_citation","url_citation":{"title":"Official","url":"https://example.test/source","start_index":0,"end_index":5}}]}}]}

data: {"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":8,"total_tokens":108,"prompt_tokens_details":{"cached_tokens":96},"server_tool_use":{"web_search_requests":1}}}

data: [DONE]

"#,
            &mut |event| events.push(event),
        )
        .expect("second stream chunk");

    let tool = noema_capabilities::ToolSpec::new(
        "search_memory",
        "Search governed Noema memory.",
        json!({"type": "object"}),
    )
    .expect("tool");
    let names = OpenAiToolNameMap::from_tools_with_request(
        &[crate::ProviderTool::canonical(tool)],
        ProviderSchemaRequest::Send,
    )
    .expect("tool names");
    let response = accumulator.finish(&mut |_| {}).expect("response");
    let normalized = response
        .finalize(
            &names,
            ProviderToolTransport::Native,
            &diagnostics.provider_kind,
            &diagnostics.model,
        )
        .expect("normalized response");

    assert_eq!(normalized.assistant_text(), "Hello");
    assert_eq!(normalized.tool_calls.len(), 1);
    assert_eq!(
        normalized.tool_calls[0].provider_call_id.as_deref(),
        Some("call_1")
    );
    assert_eq!(normalized.tool_calls[0].payload["query"], "trains");
    assert_eq!(normalized.reasoning_items.len(), 1);
    assert_eq!(
        normalized.reasoning_items[0]
            .provider_details
            .as_ref()
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(
        normalized.reasoning_items[0].encrypted_content.as_deref(),
        Some("opaque")
    );
    let crate::GenerateResponseItem::Text { citations, .. } = &normalized.responses[0];
    assert_eq!(citations[0].url, "https://example.test/source");
    assert_eq!(citations[0].start_index, Some(0));
    assert_eq!(citations[0].end_index, Some(5));
    assert_eq!(normalized.hosted_web_searches.len(), 2);
    assert_eq!(normalized.hosted_web_searches[0].tool_name, "web.search");
    assert_eq!(normalized.hosted_web_searches[0].output_index, 0);
    assert_eq!(normalized.hosted_web_searches[1].output_index, 1);
    assert_eq!(
        normalized
            .usage
            .as_ref()
            .map(|usage| usage.cached_input_tokens),
        Some(Some(96))
    );
    assert!(
        events.contains(&crate::GenerateStreamEvent::AssistantTextDelta {
            response_index: 0,
            delta: "Hel".to_string(),
        })
    );
    assert!(
        events.contains(&crate::GenerateStreamEvent::ToolCallStarted {
            output_index: 0,
            provider_call_id: "call_1".to_string(),
            name: "search_memory".to_string(),
        })
    );
    assert!(
        events.contains(&crate::GenerateStreamEvent::HostedWebSearchStarted {
            output_index: 0,
            id: Some("ws_1".to_string()),
        })
    );
    assert!(
        events.contains(&crate::GenerateStreamEvent::HostedWebSearchStarted {
            output_index: 1,
            id: Some("ws_2".to_string()),
        })
    );
}

#[test]
fn documented_search_usage_synthesizes_missing_markers() {
    let diagnostics = StructuredResponseDiagnosticContext::new(None, "openrouter", "test", None);
    let mut accumulator = ChatSseAccumulator::new();
    accumulator
        .push_bytes(
            br#"data: {"id":"chat_2","model":"openai/gpt-5.6-luna","choices":[{"index":0,"delta":{"content":"Current answer.","annotations":[{"type":"url_citation","url_citation":{"title":"Official","url":"https://example.test/source"}}]}}]}

data: {"choices":[],"usage":{"prompt_tokens":80,"completion_tokens":12,"total_tokens":92,"server_tool_use":{"web_search_requests":2}}}

data: [DONE]

"#,
            &mut |_| {},
        )
        .expect("search response stream");
    let names = OpenAiToolNameMap::from_tools_with_request(&[], ProviderSchemaRequest::Send)
        .expect("empty tool names");
    let normalized = accumulator
        .finish(&mut |_| {})
        .expect("response")
        .finalize(
            &names,
            ProviderToolTransport::Native,
            &diagnostics.provider_kind,
            &diagnostics.model,
        )
        .expect("normalized response");

    assert_eq!(normalized.hosted_web_searches.len(), 2);
    assert_eq!(normalized.hosted_web_searches[0].output_index, 0);
    assert_eq!(normalized.hosted_web_searches[1].output_index, 1);
    assert_eq!(
        normalized.hosted_web_searches[0].id.as_deref(),
        Some("chat_2:web_search:0")
    );
    assert_eq!(
        normalized.hosted_web_searches[0].result["summary"],
        "Found 1 cited source"
    );
    let crate::GenerateResponseItem::Text { citations, .. } = &normalized.responses[0];
    assert_eq!(citations.len(), 1);
}

#[test]
fn openrouter_chat_prefix_and_direct_responses_are_isolated() {
    let request = |revision: &str| GenerateRequest {
        conversation_id: Some(" conversation:cacheable ".to_string()),
        input: GenerateInput::Messages(vec![
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: "Stable application context".to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::Developer,
                content: revision.to_string(),
            },
            GenerateMessage {
                role: GenerateMessageRole::User,
                content: "What changed?".to_string(),
            },
        ]),
        instructions: Some("Stable instructions".to_string()),
        options: GenerateOptions {
            max_output_tokens: Some(32),
            ..GenerateOptions::default()
        },
        ..GenerateRequest::text("unused")
    };
    let lower = |request: &GenerateRequest, model: &str| {
        let (mut body, _, _) =
            ChatCompletionRequest::from_generate_with_schema_request_capabilities(
                request,
                model.to_string(),
                None,
                ProviderSchemaRequestCapabilities::request_strict_when_possible(),
            )
            .expect("chat body");
        body.prompt_cache_key = request
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToString::to_string);
        if model.starts_with("anthropic/") {
            body.cache_control = Some(json!({"type": "ephemeral"}));
        }
        crate::adapters::openrouter::adapt_openrouter_request(&mut body);
        serde_json::to_value(body).expect("chat json")
    };
    let first = lower(
        &request("Environment revision 8"),
        "anthropic/claude-haiku-4.5",
    );
    let second = lower(
        &request("Environment revision 9"),
        "anthropic/claude-haiku-4.5",
    );
    assert_eq!(first["prompt_cache_key"], "conversation:cacheable");
    assert_eq!(first["messages"][0], second["messages"][0]);
    assert_eq!(first["messages"][1], second["messages"][1]);
    assert_ne!(first["messages"][2], second["messages"][2]);
    assert!(first.get("input").is_none());
    assert!(first.get("instructions").is_none());
    assert!(first.get("store").is_none());

    let routed_openai = lower(&request("Environment revision 8"), "openai/gpt-4o");
    assert!(routed_openai.get("cache_control").is_none());
    assert_eq!(routed_openai["messages"][1]["role"], "user");

    for (model, profile) in [
        (
            "gpt-openai",
            crate::adapters::responses::OPENAI_RESPONSES_PROFILE,
        ),
        (
            "gpt-codex",
            crate::adapters::responses::CODEX_RESPONSES_PROFILE,
        ),
    ] {
        let (body, _, _) = crate::adapters::responses::ResponsesRequest::from_generate(
            &request("Environment revision 8"),
            model.to_string(),
            None,
            profile,
        )
        .expect("direct Responses body");
        let value = serde_json::to_value(body).expect("responses json");
        assert_eq!(value["instructions"], "Stable instructions");
        assert_eq!(value["input"][0]["role"], "developer");
        assert!(value.get("messages").is_none());
    }
}
