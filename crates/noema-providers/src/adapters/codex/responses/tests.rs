use super::*;
use crate::adapters::{
    codex::oauth::CodexTokenStore,
    test_support::{spawn_server, static_codex_credentials},
};
use crate::{
    CodexOAuthTokens, GenerateInput, GenerateOptions, GenerateResponseStatus,
    ProviderToolTransport, response_support::SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
};
use noema_capabilities::ToolSpec;
use serde_json::Value;
use tempfile::TempDir;

fn read_system_error_events(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("system error log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("system error event"))
        .collect()
}

#[test]
fn rejects_missing_account_home() {
    let error = CodexResponsesProvider::with_client_and_credentials(
        reqwest::Client::new(),
        CodexProviderConfig::default(),
        "  ",
        static_codex_credentials("access", "refresh"),
    )
    .expect_err("blank durable account identity");

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));

    let error = CodexResponsesProvider::with_client_and_credentials(
        reqwest::Client::new(),
        CodexProviderConfig {
            base_url: "https://user:password@example.test/codex?token=secret".to_string(),
            ..CodexProviderConfig::default()
        },
        "provider_account:codex:default",
        static_codex_credentials("access", "refresh"),
    )
    .expect_err("credential-bearing base URL");
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(!error.to_string().contains("password"));
    assert!(!error.to_string().contains("token=secret"));
}

#[test]
fn accepts_noema_owned_token_store() {
    let dir = TempDir::new().expect("temp dir");
    let store = CodexTokenStore::new(dir.path().join("providers/codex/default"));
    store
        .write(&CodexOAuthTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            last_refresh: 123,
        })
        .expect("write Noema tokens");

    assert!(store.has_usable_tokens());
    provider_from_config(CodexProviderConfig::default()).expect("provider credential boundary");
}

#[test]
fn provider_debug_redacts_account_and_oauth_configuration() {
    let provider = provider_from_config(CodexProviderConfig {
        oauth: crate::CodexOAuthConfig {
            client_id: "codex-oauth-client-secret".to_string(),
            ..crate::CodexOAuthConfig::default()
        },
        ..CodexProviderConfig::default()
    })
    .expect("provider");

    let debug = format!("{provider:?}");
    assert!(!debug.contains("private-codex-account"));
    assert!(!debug.contains("codex-oauth-client-secret"));
    assert!(debug.contains("[REDACTED]"));
    assert!(debug.contains("[REDACTED URL]"));
}

#[tokio::test]
async fn sends_codex_input_as_response_message_list() {
    let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3}}}\n\
             \n",
        )
        .await;
    let provider = provider_with_token(base_url);

    let mut events = Vec::new();
    let response = provider
        .generate_streaming(GenerateRequest::text("Hello?"), &mut |event| {
            events.push(event);
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/responses");
    assert_eq!(
        captured.headers.get("authorization"),
        Some(&format!("Bearer {}", test_access_token()))
    );
    assert_eq!(
        captured
            .headers
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some("workspace-test")
    );
    assert_eq!(
        captured.headers.get("originator").map(String::as_str),
        Some(CODEX_ORIGINATOR)
    );
    assert_eq!(
        captured.headers.get("version").map(String::as_str),
        Some("0.144.0")
    );
    assert_eq!(
        captured.headers.get("user-agent").map(String::as_str),
        Some("codex_cli_rs/0.144.0 (Noema)")
    );
    assert_eq!(
        captured.headers.get("accept").map(String::as_str),
        Some("text/event-stream")
    );
    assert_eq!(response.assistant_text(), "Hello");
    assert_eq!(
        events,
        vec![
            GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "Hel".to_string()
            },
            GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "lo".to_string()
            }
        ]
    );
}

#[tokio::test]
async fn codex_sse_mixed_streamed_text_and_function_call_returns_needs_tools() {
    let response_body = "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"response_status\\\":\\\"needs_tools\\\",\\\"responses\\\":[{\\\"kind\\\":\\\"text\\\",\\\"phase\\\":\\\"commentary\\\",\\\"text\\\":\\\"Checking.\\\"}],\\\"tool_calls\\\":[]}\"}\n\
             \n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":null}}\n\
             \n";
    let (base_url, request_rx) = spawn_server(200, response_body).await;
    let provider = provider_with_token(base_url);

    let response = provider
        .generate(GenerateRequest {
            options: GenerateOptions {
                require_noema_response: true,
                ..GenerateOptions::default()
            },
            tools: vec![search_memory_tool().into()],
            tool_transport: ProviderToolTransport::Native,
            ..GenerateRequest::text("Search memory")
        })
        .await
        .expect("response");

    let body: Value = serde_json::from_str(&request_rx.await.expect("request").body).expect("body");
    assert_eq!(body["tools"][0]["name"], "search_memory");
    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert_eq!(response.assistant_text(), "Checking.");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("item_1"));
    assert_eq!(
        response.tool_calls[0].provider_call_id.as_deref(),
        Some("call_1")
    );
    assert_eq!(response.tool_calls[0].name, "search_memory");
    assert_eq!(response.tool_calls[0].payload["query"], "trains");
}

#[tokio::test]
async fn codex_parses_encrypted_reasoning_items_when_returned() {
    let (base_url, _request_rx) = spawn_server(
        200,
        "event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":[{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"opaque-codex-reasoning\"},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Done\"}]}]}}\n\n",
    )
    .await;
    let response = provider_with_token(base_url)
        .generate(GenerateRequest::text("Hello?"))
        .await
        .expect("response");

    assert_eq!(response.assistant_text(), "Done");
    assert_eq!(response.reasoning_items.len(), 1);
    assert_eq!(
        response.reasoning_items[0].encrypted_content.as_deref(),
        Some("opaque-codex-reasoning")
    );
}

#[tokio::test]
async fn generate_streaming_required_noema_response_preserves_provider_activity() {
    let response_body = format!(
        "{}{}{}{}",
        "event: response.output_item.added\n\
         data: {\"type\":\"response.output_item.added\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"in_progress\"}}\n\
         \n",
        sse_delta(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hel"#
        ),
        sse_delta(r#"lo"}],"tool_calls":[]}"#),
        sse_completed(),
    );
    let (base_url, request_rx) = spawn_server(200, response_body).await;
    let provider = provider_with_token(base_url);

    let request = GenerateRequest {
        options: GenerateOptions {
            require_noema_response: true,
            ..GenerateOptions::default()
        },
        ..GenerateRequest::text("Hello?")
    };
    let mut events = Vec::new();
    let response = provider
        .generate_streaming(request, &mut |event| {
            events.push(event);
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(body["text"]["format"]["name"], "noema_response");
    assert_eq!(
        body["text"]["format"]["schema"]["properties"]["response_status"]["enum"][1],
        "final"
    );

    assert_eq!(response.assistant_text(), "Hello");
    assert_eq!(
        events,
        vec![
            GenerateStreamEvent::HostedWebSearchStarted {
                output_index: 2,
                id: Some("ws_1".to_string()),
            },
            GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "Hel".to_string()
            },
            GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "lo".to_string()
            }
        ]
    );
}

#[tokio::test]
async fn logs_required_noema_response_parse_failure() {
    let response_body = "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"output\\\":[]}\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_bad\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n";
    let (base_url, _request_rx) = spawn_server(200, response_body).await;
    let dir = TempDir::new().expect("temp dir");
    let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
    let mut provider = provider_with_token(base_url);
    provider.system_errors = Some(logger.clone());

    let error = provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:test".to_string()),
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("hello".to_string()),
            instructions: None,
            options: GenerateOptions {
                require_noema_response: true,
                ..GenerateOptions::default()
            },
            tools: Vec::new(),
            tool_transport: ProviderToolTransport::Native,
            tool_choice: Default::default(),
            parallel_tool_calls: false,
        })
        .await
        .expect_err("malformed response");

    assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    let events = read_system_error_events(logger.path());
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0]["category"],
        SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE
    );
    assert_eq!(events[0]["context"]["conversation_id"], "conversation:test");
    assert_eq!(events[0]["raw"]["provider_text"], "{\"output\":[]}");
}

fn provider_with_token(base_url: String) -> CodexResponsesProvider {
    provider_from_config(CodexProviderConfig {
        base_url,
        default_model: Some("gpt-test".to_string()),
        client_version: Some("0.144.0".to_string()),
        ..CodexProviderConfig::default()
    })
    .expect("provider")
}

fn provider_from_config(
    config: CodexProviderConfig,
) -> Result<CodexResponsesProvider, ProviderError> {
    CodexResponsesProvider::with_client_and_credentials(
        reqwest::Client::new(),
        config,
        "provider_account:codex:default",
        static_codex_credentials(test_access_token(), test_access_token()),
    )
}

fn test_access_token() -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "workspace-test"
            },
            "exp": 4_102_444_800_u64
        })
        .to_string(),
    );
    format!("header.{claims}.signature")
}

fn sse_delta(delta: &str) -> String {
    format!(
        "event: response.output_text.delta\n\
             data: {}\n\
             \n",
        serde_json::json!({
            "type": "response.output_text.delta",
            "delta": delta,
        })
    )
}

fn sse_completed() -> String {
    "event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
        \n"
        .to_string()
}

fn search_memory_tool() -> ToolSpec {
    ToolSpec::new(
        "search_memory",
        "Search memory.",
        serde_json::json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
            "additionalProperties": false
        }),
    )
    .expect("tool")
}
