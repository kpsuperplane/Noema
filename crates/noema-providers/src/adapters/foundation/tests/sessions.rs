use crate::{
    FoundationLocalProviderConfig, GenerateInput, GenerateMessage, GenerateMessageRole,
    GenerateOptions, GenerateRequest, GenerateToolResultInput, ModelProvider, ProviderError,
    ProviderTool, ProviderToolTransport,
};
use noema_capabilities::ToolSpec;

use super::{
    super::FoundationLocalProvider,
    support::{bridge_script, healthy_bridge_script},
};

#[tokio::test]
async fn generate_reuses_bridge_session_for_plain_text() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(&format!(
        r#"
    *'"id":"create_session"'*) printf '%s\n' "create_session" >> "{}"; printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
"#,
        log_path
    )));
    let provider = provider(bridge_path);

    for input in [
        GenerateInput::Text("first".to_string()),
        GenerateInput::Messages(vec![
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::User, "second"),
        ]),
    ] {
        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:stable".to_string()),
                model: Some("default".to_string()),
                input,
                instructions: Some("be concise".to_string()),
                options: GenerateOptions::default(),
                tools: Vec::new(),
                tool_transport: ProviderToolTransport::None,
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("generate");
    }

    let create_session_count = std::fs::read_to_string(log.path())
        .expect("log read")
        .lines()
        .filter(|line| *line == "create_session")
        .count();
    assert_eq!(create_session_count, 1);
}

#[tokio::test]
async fn native_tool_continuation_reuses_origin_session_and_catalog() {
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-1","tool_name":"search_memory","arguments":"{\"query\":\"first\"}"}}' ;;
    *'"type":"tool_result"'*'"call_id":"call-1"'*) printf '%s\n' '{"id":"tool_result:call-1","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-2","tool_name":"search_memory","arguments":"{\"query\":\"second\"}"}}' ;;
    *'"type":"tool_result"'*'"call_id":"call-2"'*) printf '%s\n' '{"id":"tool_result:call-2","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"continued answer"}}' ;;
"#,
    ));
    let provider = provider(bridge_path);
    let tools = vec![search_tool()];

    let first = provider
        .generate(native_request(
            GenerateInput::Text("find something".to_string()),
            "initial instructions",
            tools.clone(),
        ))
        .await
        .expect("initial native call");
    assert_eq!(first.tool_calls.len(), 1);
    assert_eq!(
        first.tool_calls[0].provider_call_id.as_deref(),
        Some("call-1")
    );

    let unrelated = provider
        .generate(native_request(
            GenerateInput::Text("unrelated".to_string()),
            "unrelated instructions",
            Vec::new(),
        ))
        .await
        .expect_err("fresh generation must not bypass a pending native call");
    assert!(matches!(unrelated, ProviderError::InvalidRequest { .. }));

    let second = provider
        .generate(native_request(
            GenerateInput::NativeToolResults(vec![tool_result("call-1", "first")]),
            "changed continuation instructions",
            Vec::new(),
        ))
        .await
        .expect("continuation should use the origin session");
    assert_eq!(second.tool_calls.len(), 1);
    assert_eq!(
        second.tool_calls[0].provider_call_id.as_deref(),
        Some("call-2")
    );

    let final_response = provider
        .generate(native_request(
            GenerateInput::NativeToolResults(vec![tool_result("call-2", "second")]),
            "changed again",
            Vec::new(),
        ))
        .await
        .expect("second continuation should complete the origin session");
    assert_eq!(final_response.assistant_text(), "continued answer");
}

#[tokio::test]
async fn generate_recreates_bridge_session_when_static_instructions_change() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(&format!(
        r#"
    *'"id":"create_session"'*) printf '%s\n' "$line" >> "{}"; printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"close_session"'*) printf '%s\n' '{{"id":"close_session","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
"#,
        log_path
    )));
    let provider = provider(bridge_path);

    for instructions in ["be concise", "be expansive"] {
        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:stable".to_string()),
                model: Some("default".to_string()),
                input: GenerateInput::Text("hello".to_string()),
                instructions: Some(instructions.to_string()),
                options: GenerateOptions::default(),
                tools: Vec::new(),
                tool_transport: ProviderToolTransport::None,
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("generate");
    }

    let create_session_requests = std::fs::read_to_string(log.path()).expect("log read");
    assert_eq!(create_session_requests.lines().count(), 2);
    assert!(create_session_requests.contains("be concise"));
    assert!(create_session_requests.contains("be expansive"));
}

#[tokio::test]
async fn generate_reuses_exact_history_and_resets_on_divergence() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(&format!(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"close_session"'*) printf '%s\n' '{{"id":"close_session","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"replay_turns"'*) printf '%s\n' "$line" >> "{}"; printf '%s\n' '{{"id":"replay_turns","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
"#,
        log_path
    )));
    let provider = provider(bridge_path);

    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
        ]))
        .await
        .expect("first generate");
    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::Developer, "environment@2"),
            message(GenerateMessageRole::User, "second"),
        ]))
        .await
        .expect("second generate");
    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "divergent answer"),
            message(GenerateMessageRole::Developer, "environment@2"),
            message(GenerateMessageRole::User, "second"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::User, "third"),
        ]))
        .await
        .expect("divergent history should recreate the session");

    let replay_requests = std::fs::read_to_string(log.path()).expect("log read");
    let mut replay_requests = replay_requests.lines();
    let first = replay_requests.next().expect("initial context replay");
    let second = replay_requests.next().expect("context update replay");
    let third = replay_requests
        .next()
        .expect("divergent full-history replay");
    assert!(replay_requests.next().is_none());
    assert!(first.contains("environment@1"));
    assert!(second.contains("environment@2"));
    assert!(!second.contains("environment@1"));
    assert!(second.contains("application_context"));
    assert!(third.contains("environment@1"));
    assert!(third.contains("divergent answer"));
}

fn provider(bridge_path: std::path::PathBuf) -> FoundationLocalProvider {
    FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: "default".to_string(),
        bridge_path: Some(bridge_path),
        system_errors: None,
    })
    .expect("provider")
}

fn search_tool() -> ProviderTool {
    ProviderTool::canonical(
        ToolSpec::new(
            "search_memory",
            "Search memory.",
            serde_json::json!({ "type": "object" }),
        )
        .expect("tool"),
    )
}

fn native_request(
    input: GenerateInput,
    instructions: &str,
    tools: Vec<ProviderTool>,
) -> GenerateRequest {
    GenerateRequest {
        conversation_id: Some("conversation:stable".to_string()),
        model: Some("default".to_string()),
        input,
        instructions: Some(instructions.to_string()),
        options: GenerateOptions::default(),
        tools,
        tool_transport: ProviderToolTransport::Native,
        tool_choice: Default::default(),
        parallel_tool_calls: false,
    }
}

fn tool_result(call_id: &str, query: &str) -> GenerateToolResultInput {
    GenerateToolResultInput {
        id: None,
        call_id: call_id.to_string(),
        name: "search_memory".to_string(),
        provider_name: Some("search_memory".to_string()),
        arguments: serde_json::json!({ "query": query }),
        success: true,
        payload: serde_json::json!({ "matches": [] }),
    }
}

fn session_request(messages: Vec<GenerateMessage>) -> GenerateRequest {
    GenerateRequest {
        conversation_id: Some("conversation:stable".to_string()),
        model: Some("default".to_string()),
        input: GenerateInput::Messages(messages),
        instructions: Some("stable kernel".to_string()),
        options: GenerateOptions::default(),
        tools: Vec::new(),
        tool_transport: ProviderToolTransport::None,
        tool_choice: Default::default(),
        parallel_tool_calls: false,
    }
}

fn message(role: GenerateMessageRole, content: &str) -> GenerateMessage {
    GenerateMessage {
        role,
        content: content.to_string(),
    }
}
