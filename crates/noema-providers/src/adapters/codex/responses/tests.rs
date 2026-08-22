use super::*;
use crate::adapters::{
    codex::oauth::CodexTokenStore,
    test_support::{
        spawn_blocking_websocket_server, spawn_scripted_server, spawn_server,
        spawn_websocket_server, static_codex_credentials,
    },
};
use crate::{CodexOAuthTokens, GenerateInput, ProviderSessionInput, ProviderToolTransport};
use noema_capabilities::ToolSpec;
use serde_json::Value;
use tempfile::TempDir;

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
fn provider_debug_preserves_account_and_oauth_configuration() {
    let provider = provider_from_config(CodexProviderConfig {
        oauth: crate::CodexOAuthConfig {
            client_id: "codex-oauth-client-public".to_string(),
            ..crate::CodexOAuthConfig::default()
        },
        ..CodexProviderConfig::default()
    })
    .expect("provider");

    let debug = format!("{provider:?}");
    assert!(debug.contains("provider_account:codex:default"));
    assert!(debug.contains("codex-oauth-client-public"));
    assert!(debug.contains(crate::DEFAULT_CODEX_BASE_URL));
}

#[tokio::test]
async fn advertises_a_bounded_context_for_runtime_compaction() {
    let provider = provider_from_config(CodexProviderConfig::default()).expect("provider");

    assert_eq!(
        provider.context_metadata(Some("gpt-5.6-luna")).await,
        ProviderContextMetadata {
            context_window_tokens: Some(128_000),
            default_output_reserve_tokens: Some(8_000),
            compact_summary_target_tokens: Some(2_048),
        }
    );
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
    let provider = provider_from_config(CodexProviderConfig {
        base_url,
        default_model: Some("gpt-test".to_string()),
        client_version: Some("0.144.0".to_string()),
        ..CodexProviderConfig::default()
    })
    .expect("provider");
    let mut request = GenerateRequest::text("Hello?");
    request.options.fast_mode = true;
    let mut events = Vec::new();
    let response = provider
        .generate_streaming(request, &mut |event| {
            events.push(event);
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/responses");
    let body: Value = serde_json::from_str(&captured.body).expect("request body");
    assert_eq!(body["service_tier"], "priority");
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
    events.drain(..2).for_each(drop);
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
async fn codex_sse_mixed_streamed_text_and_function_call_preserves_both() {
    let response_body = "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Checking.\"}\n\
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
            tools: vec![search_memory_tool().into()],
            tool_transport: ProviderToolTransport::Native,
            ..GenerateRequest::text("Search memory")
        })
        .await
        .expect("response");

    let body: Value = serde_json::from_str(&request_rx.await.expect("request").body).expect("body");
    assert_eq!(body["tools"][0]["name"], "search_memory");
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
async fn generate_streaming_plain_text_preserves_provider_activity() {
    let response_body = format!(
        "{}{}{}{}",
        "event: response.output_item.added\n\
         data: {\"type\":\"response.output_item.added\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"in_progress\"}}\n\
         \n",
        sse_delta("Hel"),
        sse_delta("lo"),
        sse_completed(),
    );
    let (base_url, request_rx) = spawn_server(200, response_body).await;
    let provider = provider_with_token(base_url);

    let request = GenerateRequest::text("Hello?");
    let mut events = Vec::new();
    let response = provider
        .generate_streaming(request, &mut |event| {
            events.push(event);
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert!(body.get("text").is_none());

    assert_eq!(response.assistant_text(), "Hello");
    events.drain(..2).for_each(drop);
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
async fn websocket_session_uses_complete_then_incremental_requests_and_replays_changes() {
    let (base_url, requests_rx) = spawn_websocket_server(vec![
        vec![websocket_completed("resp_1", "one")],
        vec![websocket_completed("resp_2", "two")],
        vec![websocket_completed("resp_3", "three")],
        vec![websocket_completed("resp_4", "four")],
    ])
    .await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    let mut request = GenerateRequest::text("complete one");
    session
        .generate(
            request.clone(),
            ProviderSessionInput::initial(request.input.clone()),
            &mut |_| {},
        )
        .await
        .expect("initial response");
    request.input = GenerateInput::Text("ignored by session input".to_string());
    session
        .generate(
            request.clone(),
            ProviderSessionInput {
                replay: GenerateInput::Text("complete two".to_string()),
                incremental: Some(GenerateInput::Text("incremental two".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect("incremental response");
    request.instructions = Some("changed rules".to_string());
    session
        .generate(
            request.clone(),
            ProviderSessionInput {
                replay: GenerateInput::Text("complete three".to_string()),
                incremental: Some(GenerateInput::Text("unsafe incremental".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect("changed request response");
    session
        .generate(
            request,
            ProviderSessionInput {
                replay: GenerateInput::Text("compacted complete four".to_string()),
                incremental: None,
            },
            &mut |_| {},
        )
        .await
        .expect("compacted response");

    let requests = requests_rx.await.expect("WebSocket requests");
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0]["type"], "response.create");
    assert_eq!(requests[0]["store"], false);
    assert!(requests[0].get("previous_response_id").is_none());
    assert_eq!(codex_request_text(&requests[0]), "complete one");
    assert_eq!(requests[1]["previous_response_id"], "resp_1");
    assert_eq!(codex_request_text(&requests[1]), "incremental two");
    assert!(requests[2].get("previous_response_id").is_none());
    assert_eq!(codex_request_text(&requests[2]), "complete three");
    assert!(requests[3].get("previous_response_id").is_none());
    assert_eq!(codex_request_text(&requests[3]), "compacted complete four");
}

#[tokio::test]
async fn missing_previous_response_replays_complete_input_once() {
    let (base_url, requests_rx) = spawn_websocket_server(vec![
        vec![websocket_completed("resp_1", "one")],
        vec![serde_json::json!({
            "type": "error",
            "error": {"code": "previous_response_not_found", "message": "gone"}
        })],
        vec![websocket_completed("resp_2", "two")],
    ])
    .await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    session
        .generate(
            GenerateRequest::text("one"),
            ProviderSessionInput::initial(GenerateInput::Text("one".to_string())),
            &mut |_| {},
        )
        .await
        .expect("initial response");
    session
        .generate(
            GenerateRequest::text("two"),
            ProviderSessionInput {
                replay: GenerateInput::Text("complete two".to_string()),
                incremental: Some(GenerateInput::Text("incremental two".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect("replayed response");

    let requests = requests_rx.await.expect("WebSocket requests");
    assert_eq!(requests[1]["previous_response_id"], "resp_1");
    assert_eq!(codex_request_text(&requests[1]), "incremental two");
    assert!(requests[2].get("previous_response_id").is_none());
    assert_eq!(codex_request_text(&requests[2]), "complete two");
}

#[tokio::test]
async fn missing_previous_response_does_not_replay_hosted_web_state() {
    let (base_url, requests_rx) = spawn_websocket_server(vec![
        vec![websocket_completed_with_search("resp_1")],
        vec![serde_json::json!({
            "type": "error",
            "error": {"code": "previous_response_not_found", "message": "gone"}
        })],
    ])
    .await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    session
        .generate(
            GenerateRequest::text("one"),
            ProviderSessionInput::initial(GenerateInput::Text("one".to_string())),
            &mut |_| {},
        )
        .await
        .expect("initial response");
    let error = session
        .generate(
            GenerateRequest::text("two"),
            ProviderSessionInput {
                replay: GenerateInput::Text("complete two".to_string()),
                incremental: Some(GenerateInput::Text("incremental two".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect_err("provider-only state must fail closed");

    assert!(matches!(error, ProviderError::ProtocolError { .. }));
    assert_eq!(requests_rx.await.expect("WebSocket requests").len(), 2);
}

#[tokio::test]
async fn hosted_web_state_reports_changed_request_fields() {
    let (base_url, requests_rx) =
        spawn_websocket_server(vec![vec![websocket_completed_with_search("resp_1")]]).await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    let mut request = GenerateRequest::text("one");
    session
        .generate(
            request.clone(),
            ProviderSessionInput::initial(GenerateInput::Text("one".to_string())),
            &mut |_| {},
        )
        .await
        .expect("initial response");
    request.instructions = Some("changed instructions".to_string());
    let error = session
        .generate(
            request,
            ProviderSessionInput {
                replay: GenerateInput::Text("complete two".to_string()),
                incremental: Some(GenerateInput::Text("incremental two".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect_err("changed settings must fail closed");

    assert!(matches!(
        error,
        ProviderError::ProtocolError { message, .. }
            if message.ends_with("request settings changed: instructions")
    ));
    assert_eq!(requests_rx.await.expect("WebSocket requests").len(), 1);
}

#[tokio::test]
async fn websocket_generation_honors_provider_timeout() {
    let (base_url, request_rx) = spawn_blocking_websocket_server().await;
    let provider = provider_from_config(CodexProviderConfig {
        base_url,
        timeout_seconds: 1,
        ..CodexProviderConfig::default()
    })
    .expect("provider");
    let mut session = provider.open_generation_session();
    let error = session
        .generate(
            GenerateRequest::text("wait"),
            ProviderSessionInput::initial(GenerateInput::Text("wait".to_string())),
            &mut |_| {},
        )
        .await
        .expect_err("request must time out");

    request_rx.await.expect("WebSocket request");
    assert!(matches!(
        error,
        ProviderError::Timeout {
            operation,
            seconds: 1,
            ..
        } if operation == "responses_websocket"
    ));
}

#[tokio::test]
async fn websocket_failure_after_output_does_not_replay() {
    let (base_url, requests_rx) = spawn_websocket_server(vec![
        vec![websocket_completed("resp_1", "one")],
        vec![serde_json::json!({
            "type": "response.output_text.delta",
            "output_index": 0,
            "delta": "started"
        })],
    ])
    .await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    session
        .generate(
            GenerateRequest::text("one"),
            ProviderSessionInput::initial(GenerateInput::Text("one".to_string())),
            &mut |_| {},
        )
        .await
        .expect("initial response");
    let error = session
        .generate(
            GenerateRequest::text("two"),
            ProviderSessionInput {
                replay: GenerateInput::Text("complete two".to_string()),
                incremental: Some(GenerateInput::Text("incremental two".to_string())),
            },
            &mut |_| {},
        )
        .await
        .expect_err("interrupted response");
    assert!(matches!(error, ProviderError::TransportFailure { .. }));
    assert_eq!(requests_rx.await.expect("WebSocket requests").len(), 2);
}

#[tokio::test]
async fn unsupported_websocket_uses_stateless_sse() {
    let (base_url, requests_rx) = spawn_scripted_server(vec![
        (404, "{}".to_string()),
        (200, sse_delta("fallback") + &sse_completed()),
    ])
    .await;
    let provider = provider_with_token(base_url);
    let mut session = provider.open_generation_session();
    session
        .generate(
            GenerateRequest::text("fallback"),
            ProviderSessionInput::initial(GenerateInput::Text("fallback".to_string())),
            &mut |_| {},
        )
        .await
        .expect("SSE response");
    let requests = requests_rx.await.expect("requests");
    assert_eq!(requests[0].method, "GET");
    let body: Value = serde_json::from_str(&requests[1].body).expect("SSE body");
    assert!(body.get("previous_response_id").is_none());
    assert_eq!(body["store"], false);
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

fn websocket_completed(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "model": "gpt-test",
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": text}]
            }]
        }
    })
}

fn websocket_completed_with_search(id: &str) -> Value {
    serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "model": "gpt-test",
            "status": "completed",
            "output": [
                {
                    "type": "web_search_call",
                    "id": "search_1",
                    "status": "completed",
                    "action": {"type": "search", "query": "current facts"}
                },
                {
                    "type": "message",
                    "content": [{"type": "output_text", "text": "found it"}]
                }
            ]
        }
    })
}

fn codex_request_text(request: &Value) -> &str {
    request["input"][0]["content"]
        .as_str()
        .or_else(|| request["input"][0]["content"][0]["text"].as_str())
        .expect("request text")
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
