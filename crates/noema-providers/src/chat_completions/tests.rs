use super::request::{
    ChatCompletionRequest, ChatMessage, ChatMessageContent, ChatTextBlock, ChatTool,
    OpenAiToolNameMap,
};
use super::sse::ChatSseAccumulator;
use crate::response_support::StructuredResponseDiagnosticContext;
use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateRequest, GenerateToolCallInput, ProviderSchemaCapabilities,
    ProviderToolTransport, ReasoningEffort, SchemaEnforcement,
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
        ChatCompletionRequest::from_generate_with_schema_capabilities(
            &request,
            "anthropic/claude-haiku-4.5".to_string(),
            None,
            ProviderSchemaCapabilities::strict(),
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

    let mut structured = ChatMessage {
        role: "developer".to_string(),
        content: Some(ChatMessageContent::Blocks(vec![ChatTextBlock {
            kind: "text",
            text: "& <unsafe>".to_string(),
        }])),
        tool_calls: Vec::new(),
        tool_call_id: None,
        reasoning_details: Vec::new(),
    };
    structured
        .content
        .as_mut()
        .expect("content")
        .wrap_application_context();
    assert_eq!(
        serde_json::to_value(structured).expect("structured json")["content"][0]["text"],
        "<noema_application_context>\n&amp; &lt;unsafe&gt;\n</noema_application_context>"
    );
    assert_eq!(names.canonical_name("search_memory"), Some("search_memory"));
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

data: {"choices":[{"index":0,"delta":{"content":"lo","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"search_memory","arguments":"{\"query\":\""}}]}}]}

"#,
            &mut |event| events.push(event),
        )
        .expect("first stream chunk");
    accumulator
        .push_bytes(
            br#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"trains\"}"}}],"reasoning_details":[{"type":"reasoning.encrypted","id":"rs_1","data":"que"},{"type":"reasoning.summary","text":"Searching"},{"type":"reasoning.server_tool_call","id":"ws_1","name":"web.search","status":"completed","arguments":{"query":"trains"},"result":{"sources":1}}],"annotations":[{"type":"url_citation","url_citation":{"title":"Official","url":"https://example.test/source","start_index":0,"end_index":5}}]}}]}

data: {"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":8,"total_tokens":108,"prompt_tokens_details":{"cached_tokens":96}}}

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
    let names = OpenAiToolNameMap::from_tools_with_enforcement(
        &[crate::ProviderTool::canonical(tool)],
        SchemaEnforcement::BestEffort,
    )
    .expect("tool names");
    let response = accumulator.finish(&mut |_| {}).expect("response");
    let normalized = response
        .finalize(&names, ProviderToolTransport::Native, &diagnostics)
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
        Some(3)
    );
    assert_eq!(
        normalized.reasoning_items[0].encrypted_content.as_deref(),
        Some("opaque")
    );
    assert_eq!(normalized.citations[0].url, "https://example.test/source");
    assert_eq!(normalized.citations[0].start_index, Some(0));
    assert_eq!(normalized.citations[0].end_index, Some(5));
    assert_eq!(normalized.hosted_web_searches.len(), 1);
    assert_eq!(normalized.hosted_web_searches[0].tool_name, "web.search");
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
            name: "search_memory".to_string(),
        })
    );
    assert!(
        events.contains(&crate::GenerateStreamEvent::HostedWebSearchStarted {
            output_index: 0,
            id: Some("ws_1".to_string()),
        })
    );
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
        let (mut body, _, _) = ChatCompletionRequest::from_generate_with_schema_capabilities(
            request,
            model.to_string(),
            None,
            ProviderSchemaCapabilities::strict(),
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
