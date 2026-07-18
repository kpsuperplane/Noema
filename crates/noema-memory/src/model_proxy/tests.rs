//! Focused model proxy tests.

use std::{sync::Arc, time::Duration};

use noema_providers::{
    GenerateInput, GenerateInputItem, GenerateResponse, GenerateResponseStatus, GenerateToolCall,
    GenerationPriority, NoemaToolChoice, ProviderRegistry, RegistryProviderRouteResolver,
};
use serde_json::{Value, json};

use super::test_support::{
    BlockingProvider, CapturingProvider, FakeSelectionRepository, instance_key, post_chat,
    proxy_config, selection,
};

#[tokio::test]
async fn chat_completions_translate_configured_model_tools_and_history_ids() {
    let provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
        "stored",
        "fake",
        "memory-model",
    )));
    let route = proxy_config(provider, "memory-model")
        .route_resolver
        .resolve_route()
        .await
        .expect("route");
    let request =
        serde_json::from_value::<super::translation::OpenAiChatCompletionRequest>(json!({
            "model": "memory-model",
            "messages": [
                {"role": "system", "content": "extract useful memories"},
                {"role": "user", "content": "Kevin likes local-first tools"}
            ],
            "max_tokens": 123,
            "temperature": 0.2,
            "tool_choice": "required",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "CreateMemory",
                    "description": "Create a memory",
                    "parameters": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"],
                        "additionalProperties": false
                    }
                }
            }]
        }))
        .expect("OpenAI request")
        .into_generate_request(&route)
        .expect("generate request");
    assert_eq!(request.model.as_deref(), Some("memory-model"));
    assert_eq!(
        request.instructions.as_deref(),
        Some("extract useful memories")
    );
    assert_eq!(request.options.max_output_tokens, Some(123));
    assert_eq!(request.options.temperature, None);
    assert_eq!(
        request.options.generation_priority,
        GenerationPriority::Background
    );
    assert!(matches!(request.tool_choice, NoemaToolChoice::Required));
    assert_eq!(request.tools[0].name.as_str(), "CreateMemory");
    assert_eq!(request.tools[0].description, "Create a memory");
    let GenerateInput::Items(items) = &request.input else {
        panic!("expected itemized input");
    };
    assert_eq!(items.len(), 1);
    assert!(
        request
            .input
            .render_for_token_count()
            .contains("Kevin likes")
    );

    let request =
        serde_json::from_value::<super::translation::OpenAiChatCompletionRequest>(json!({
            "model": "memory-model",
            "messages": [
                {"role": "user", "content": "remember this"},
                {"role": "assistant", "content": null, "tool_calls": [{
                    "id": "call_5FiAc5MZsQF2jiDkEJxu5IrB", "type": "function",
                    "function": {"name": "CreateMemory", "arguments": "{\"text\":\"remember\"}"}
                }]},
                {"role": "tool", "tool_call_id": "call_5FiAc5MZsQF2jiDkEJxu5IrB",
                 "name": "CreateMemory", "content": "{\"ok\":true}"}
            ]
        }))
        .expect("history request")
        .into_generate_request(&route)
        .expect("generate request");
    let GenerateInput::Items(items) = &request.input else {
        panic!("expected itemized input");
    };
    let GenerateInputItem::ToolCall(call) = &items[1] else {
        panic!("expected tool call item");
    };
    assert_eq!(call.id, None);
    assert_eq!(call.call_id, "call_5FiAc5MZsQF2jiDkEJxu5IrB");
    let GenerateInputItem::ToolResult(result) = &items[2] else {
        panic!("expected tool result item");
    };
    assert_eq!(result.call_id, call.call_id);
}

#[tokio::test]
async fn refreshed_selection_preserves_old_lease_and_routes_later_requests_to_replacement() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let old_provider = Arc::new(BlockingProvider {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    });
    let new_provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
        "new",
        "local_models",
        "replacement-model",
    )));
    let old_key = instance_key("codex:memory-proxy:old");
    let new_key = instance_key("local_models:memory-proxy:new");
    let registry = Arc::new(ProviderRegistry::new());
    registry
        .register(
            old_key.clone(),
            old_provider as noema_providers::ProviderHandle,
        )
        .expect("register old provider");
    let repository = Arc::new(FakeSelectionRepository::new(selection(
        "codex",
        "provider_account:codex:default",
        old_key,
        "initial-model",
    )));
    let proxy = super::MemoryModelProxy::start(super::MemoryModelProxyConfig {
        route_resolver: Arc::new(RegistryProviderRouteResolver::new(
            repository.clone(),
            registry.clone(),
        )),
        api_key: "secret".to_string(),
        model_profile: "initial-model".to_string(),
        system_errors: None,
    })
    .await
    .expect("start proxy");

    let client = reqwest::Client::new();
    let first_started = started.notified();
    let first_url = format!("{}/chat/completions", proxy.openai_base_url());
    let first_api_key = proxy.api_key().to_string();
    let first_client = client.clone();
    let first = tokio::spawn(async move {
        first_client
            .post(first_url)
            .bearer_auth(first_api_key)
            .json(&json!({
                "model": "initial-model",
                "messages": [{"role": "user", "content": "first request"}]
            }))
            .send()
            .await
            .expect("first request")
            .json::<Value>()
            .await
            .expect("first json")
    });
    first_started.await;

    registry
        .register(
            new_key.clone(),
            new_provider.clone() as noema_providers::ProviderHandle,
        )
        .expect("register replacement provider");
    repository.save(selection(
        "local_models",
        "provider_account:local_models:default",
        new_key,
        "replacement-model",
    ));

    let models: Value = client
        .get(format!("{}/models", proxy.openai_base_url()))
        .send()
        .await
        .expect("model list")
        .json()
        .await
        .expect("model list json");
    assert_eq!(models["data"][0]["id"], "replacement-model");

    release.notify_one();
    let first_body = first.await.expect("first task");
    assert_eq!(first_body["choices"][0]["message"]["content"], "old");

    let second_body: Value = post_chat(
        &proxy,
        json!({
            "model": "initial-model",
            "messages": [{"role": "user", "content": "second request"}]
        }),
    )
    .await
    .json()
    .await
    .expect("second json");
    assert_eq!(second_body["choices"][0]["message"]["content"], "new");

    let requests = new_provider.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].model.as_deref(), Some("replacement-model"));
}

#[tokio::test]
async fn shutdown_drains_in_flight_connections_before_returning() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let provider = Arc::new(BlockingProvider {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    });
    let proxy = super::MemoryModelProxy::start(proxy_config(provider, "memory-model"))
        .await
        .expect("start proxy");
    let request_started = started.notified();
    let url = format!("{}/chat/completions", proxy.openai_base_url());
    let api_key = proxy.api_key().to_string();
    let request = tokio::spawn(async move {
        reqwest::Client::new()
            .post(url)
            .bearer_auth(api_key)
            .json(&json!({
                "model": "memory-model",
                "messages": [{"role": "user", "content": "finish during shutdown"}]
            }))
            .send()
            .await
            .expect("request")
            .json::<Value>()
            .await
            .expect("json")
    });
    request_started.await;

    let mut shutdown = tokio::spawn(proxy.shutdown());
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut shutdown)
            .await
            .is_err(),
        "shutdown returned before the active connection completed"
    );

    release.notify_one();
    let body = request.await.expect("request task");
    assert_eq!(body["choices"][0]["message"]["content"], "old");
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown should finish after the connection drains")
        .expect("shutdown task");
}

#[test]
fn chat_completions_return_provider_tool_calls() {
    let body = super::translation::openai_response_from_generate_response(GenerateResponse {
        responses: Vec::new(),
        tool_calls: vec![GenerateToolCall {
            id: Some("item_1".to_string()),
            provider_call_id: Some("call_1".to_string()),
            provider_name: Some("CreateMemory".to_string()),
            name: "CreateMemory".to_string(),
            payload: json!({"text": "Kevin likes local-first tools"}),
        }],
        reasoning_items: Vec::new(),
        response_status: GenerateResponseStatus::NeedsTools,
        provider: "fake".to_string(),
        model: "memory-model".to_string(),
        response_id: Some("resp_1".to_string()),
        usage: None,
    });

    assert_eq!(body["id"], "resp_1");
    assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["id"],
        "call_1"
    );
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "CreateMemory"
    );
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{\"text\":\"Kevin likes local-first tools\"}"
    );
}

#[tokio::test]
async fn chat_completions_reject_streaming_requests() {
    let provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
        "unused",
        "fake",
        "memory-model",
    )));
    let proxy = super::MemoryModelProxy::start(proxy_config(provider.clone(), "memory-model"))
        .await
        .expect("start proxy");

    let response = post_chat(
        &proxy,
        json!({
            "model": "memory-model",
            "stream": true,
            "messages": [{"role": "user", "content": "hello"}]
        }),
    )
    .await;

    assert_eq!(response.status(), reqwest::StatusCode::NOT_IMPLEMENTED);
    assert!(provider.requests().is_empty());
}
