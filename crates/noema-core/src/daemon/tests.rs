use super::*;
use super::{protocol::TurnStreamEvent, runtime::CodexRuntimeHandle};
use crate::{
    ActorRef,
    provider::{
        AssistantTextPhase, GenerateActionItem, GenerateInput, GenerateInputItem,
        GenerateReasoningItem, GenerateRequest, GenerateResponse, GenerateResponseItem,
        GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall, MultipleChoiceOption,
        MultipleChoiceSelectionMode, ProviderError, ProviderToolCapabilities,
        ProviderToolFallbackMode, ProviderToolSchemaDialect,
    },
    {ConversationItemKind, ConversationItemStatus, ReplayMode},
};
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    pin::Pin,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex as AsyncMutex, mpsc, oneshot},
};

const RESTART_CONTEXT_TEST_PHASE_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_PHASE";
const RESTART_CONTEXT_TEST_HOME_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_HOME";
const RESTART_CONTEXT_TEST_CONVERSATION_FILE: &str = "restart_context_conversation_id";

#[derive(Debug, Clone)]
enum GenerateOutputItem {
    AssistantText {
        phase: Option<AssistantTextPhase>,
        text: String,
    },
    MultipleChoice {
        phase: Option<AssistantTextPhase>,
        prompt: String,
        selection_mode: MultipleChoiceSelectionMode,
        options: Vec<MultipleChoiceOption>,
    },
    ToolCall {
        id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        name: String,
        payload: serde_json::Value,
    },
}

#[tokio::test]
async fn runtime_actor_allocates_distinct_conversation_ids() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::Simple)).await;

    let first = handle
        .start_conversation(None)
        .await
        .expect("first conversation");
    let second = handle
        .start_conversation(None)
        .await
        .expect("second conversation");

    assert_ne!(first.conversation_id, second.conversation_id);

    let items = collect_turn(&handle, first.conversation_id.clone(), "hello".to_string())
        .await
        .expect("turn response");
    assert_eq!(assistant_text(&items), "fake answer");

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_handle_generate_once_uses_provider_without_conversation() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::Simple)).await;

    let response = handle
        .generate_once(GenerateRequest::text("hello"))
        .await
        .expect("generate once");
    handle.shutdown().await;

    assert_eq!(response.assistant_text(), "fake answer");
}

#[tokio::test]
async fn runtime_handle_generate_once_does_not_block_subsequent_commands() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider = BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    };
    let store = crate::store::tests::test_store().await;
    let handle = CodexRuntimeHandle::spawn_with_provider(Arc::new(provider), store)
        .await
        .expect("runtime");
    let generate_handle = handle.clone();
    let pending_generate = tokio::spawn(async move {
        generate_handle
            .generate_once(GenerateRequest::text("slow"))
            .await
    });

    started_rx.await.expect("provider started");
    let start_result =
        tokio::time::timeout(Duration::from_millis(100), handle.start_conversation(None)).await;
    let _ = release_tx.send(());
    let generated = pending_generate
        .await
        .expect("generate task")
        .expect("generate result");
    handle.shutdown().await;

    let conversation = start_result
        .expect("start_conversation should not wait for generate_once provider completion")
        .expect("conversation");
    assert!(conversation.conversation_id.starts_with("conversation:"));
    assert_eq!(generated.assistant_text(), "slow answer");
}

#[tokio::test]
async fn runtime_turn_streams_durable_assistant_item_and_idle_status() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "hello".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::InputReceived,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Thinking,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Idle,
            } if id == &conversation_id
        )
    }));

    let Some((assistant_item_id, assistant_turn_id)) =
        events.iter().find_map(|event| match event {
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id,
                item,
                ..
            } if id == &conversation_id => match item.as_ref() {
                TurnTranscriptItem::AssistantText { text } if text == "fake answer" => {
                    Some((item_id.clone(), turn_id.clone()))
                }
                _ => None,
            },
            _ => None,
        })
    else {
        panic!("expected durable assistant conversation item, got {events:?}");
    };
    assert!(assistant_item_id.starts_with("item:"));
    assert!(assistant_turn_id.is_some());

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == assistant_item_id
            && item.kind == ConversationItemKind::AssistantText
            && item.status == ConversationItemStatus::Completed
    }));
}

#[tokio::test]
async fn turn_persists_multiple_choice_prompt() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::MultipleChoice)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "choose".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    let prompt_event = events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item_id, item, .. } => match item.as_ref() {
                TurnTranscriptItem::MultipleChoicePrompt {
                    prompt,
                    selection_mode,
                    options,
                } if prompt == "Pick a direction"
                    && selection_mode == &MultipleChoiceSelectionMode::PickOne =>
                {
                    Some((item_id.clone(), options.clone()))
                }
                _ => None,
            },
            TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .expect("multiple choice prompt event");
    assert_eq!(
        prompt_event.1,
        vec![
            MultipleChoiceOption {
                id: "ship".to_string(),
                label: "Ship it".to_string(),
            },
            MultipleChoiceOption {
                id: "polish".to_string(),
                label: "Polish first".to_string(),
            },
        ]
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == prompt_event.0
            && item.kind == ConversationItemKind::MultipleChoicePrompt
            && item.status == ConversationItemStatus::Completed
            && item.content_text.as_deref() == Some("Pick a direction")
            && item.payload_json["selection_mode"] == "pick_one"
            && item.payload_json["options"][0]["id"] == "ship"
    }));
}

#[tokio::test]
async fn slash_remember_is_ordinary_chat_text() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "/remember I like trains".to_string(),
    )
    .await
    .expect("turn response");
    handle.shutdown().await;

    let items = store
        .list_visible_conversation_item_page(&conversation_id, None, 20)
        .await
        .expect("items");
    assert!(
        items
            .items
            .iter()
            .any(|item| item.content_text.as_deref() == Some("/remember I like trains"))
    );
    assert!(!items.items.iter().any(|item| {
        item.payload_json
            .get("activity_kind")
            .and_then(serde_json::Value::as_str)
            == Some("memory_save")
    }));
    assert!(!items.items.iter().any(|item| {
        item.payload_json
            .get("activity_kind")
            .and_then(serde_json::Value::as_str)
            == Some("memory_extraction")
    }));
}

#[tokio::test]
async fn primary_agent_runtime_preference_supplies_turn_model() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect("preference");

    let provider = Arc::new(CapturingProvider::default());
    let runtime =
        CodexRuntimeHandle::spawn_with_provider_kind(provider.clone(), store, "foundation_local")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    assert_eq!(
        requests.last().and_then(|request| request.model.as_deref()),
        Some("default")
    );
}

#[tokio::test]
async fn primary_agent_runtime_preference_supplies_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::High),
        })
        .await
        .expect("preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime =
        CodexRuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(crate::provider::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn primary_agent_codex_preference_sends_reasoning_effort_to_codex_provider_kind() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::High),
        })
        .await
        .expect("preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let openai_provider = Arc::new(CapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_map(
        "openai",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
            (
                "openai".to_string(),
                openai_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
        ],
        store,
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    assert!(openai_provider.requests.lock().expect("openai").is_empty());
    let codex_requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        codex_requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(crate::provider::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn primary_agent_openai_preference_sends_reasoning_effort_to_openai_provider_kind() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider_account_id = "provider_account:openai:runtime_reasoning";
    insert_authenticated_provider_account(
        &store,
        provider_account_id,
        "openai",
        "runtime-reasoning",
    )
    .await;
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "openai".to_string(),
            provider_account_id: provider_account_id.to_string(),
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Medium),
        })
        .await
        .expect("preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let openai_provider = Arc::new(CapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_map(
        "codex",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
            (
                "openai".to_string(),
                openai_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
        ],
        store,
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    assert!(codex_provider.requests.lock().expect("codex").is_empty());
    let openai_requests = openai_provider.requests.lock().expect("openai requests");
    assert_eq!(
        openai_requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(crate::provider::ReasoningEffort::Medium)
    );
}

#[tokio::test]
async fn primary_agent_default_provider_sends_no_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime =
        CodexRuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        None
    );
}

#[tokio::test]
async fn native_provider_turn_request_includes_builtin_tools() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        capabilities: ProviderToolCapabilities {
            native_tools: true,
            parallel_tool_calls: true,
            tool_choice: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: true,
            prompt_cache_key: false,
            encrypted_reasoning: false,
            fallback_mode: ProviderToolFallbackMode::NativeRequired,
        },
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    assert_eq!(
        request.options.prompt_cache_retention,
        Some(crate::PromptCacheRetention::TwentyFourHours)
    );
    let tool_names = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"search_memory"));
    assert!(request.parallel_tool_calls);
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(instructions.contains("Executable tools are provided through the native tool channel"));
    assert!(!instructions.contains("emit the relevant tool_calls item in this response"));
}

#[tokio::test]
async fn normal_turn_instructions_are_stable_across_turns() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id.clone(),
            "first durable question".to_string(),
            first_tx,
        )
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            second_tx,
        )
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_instructions = agent_requests[0]
        .instructions
        .as_deref()
        .expect("first instructions");
    let second_instructions = agent_requests[1]
        .instructions
        .as_deref()
        .expect("second instructions");
    assert_eq!(first_instructions, second_instructions);
    assert!(!first_instructions.contains("conversation_id:"));
    assert!(!first_instructions.contains("turn_index:"));
    assert!(!first_instructions.contains("cwd_project_hint:"));
    assert!(!first_instructions.contains("Recent durable transcript"));
    assert!(!first_instructions.contains("first durable question"));
    assert!(!second_instructions.contains("second durable question"));
}

#[tokio::test]
async fn normal_turn_input_replays_previous_turn_as_prefix() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id.clone(),
            "first durable question".to_string(),
            first_tx,
        )
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            second_tx,
        )
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_items = input_message_texts(&agent_requests[0].input);
    let second_items = input_message_texts(&agent_requests[1].input);
    assert_eq!(first_items, vec!["first durable question".to_string()]);
    assert!(second_items.starts_with(&[
        "first durable question".to_string(),
        "fake answer".to_string(),
    ]));
    assert_eq!(
        second_items.last().map(String::as_str),
        Some("second durable question")
    );
}

#[tokio::test]
async fn runtime_persists_and_replays_encrypted_reasoning_items() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ReasoningReplay)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let first_items = collect_turn(&handle, conversation_id.clone(), "first".to_string())
        .await
        .expect("first turn");
    assert!(first_items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::AssistantText { text, .. } if text == "first answer"
        )
    }));

    let second_items = collect_turn(&handle, conversation_id, "second".to_string())
        .await
        .expect("second turn");
    assert!(second_items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::AssistantText { text, .. } if text == "saw encrypted reasoning"
        )
    }));

    handle.shutdown().await;
}

#[tokio::test]
async fn prompt_context_uses_active_summary_and_post_checkpoint_items() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item(&store, &started.conversation_id, "covered user").await;
    append_test_text_item(&store, &started.conversation_id, "covered assistant").await;
    store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: started.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: None,
            summary_text: "Summary: the user approved rolling durable compaction.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: None,
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");
    append_test_text_item(&store, &started.conversation_id, "post checkpoint user").await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(!instructions.contains("Compacted conversation context:"));
    assert!(!instructions.contains("rolling durable compaction"));
    assert!(!instructions.contains("current turn"));
    assert!(!instructions.contains("covered user"));
    assert!(!instructions.contains("post checkpoint user"));
    let input = request.input.render_for_token_count();
    assert!(input.contains("Compacted conversation context:"));
    assert!(input.contains("rolling durable compaction"));
    assert!(input.contains("post checkpoint user"));
    assert!(input.contains("current turn"));
    assert!(!input.contains("covered user"));
}

#[tokio::test]
async fn compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item(&store, &started.conversation_id, "covered user").await;
    append_test_text_item(&store, &started.conversation_id, "covered assistant").await;
    store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: started.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: None,
            summary_text: "Summary: compacted checkpoint facts.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: None,
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");
    append_test_text_item(&store, &started.conversation_id, "post checkpoint user").await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(!instructions.contains("compacted checkpoint facts"));
    let input_texts = input_message_texts(&request.input);
    assert_eq!(
        input_texts.first().map(String::as_str),
        Some(
            "Compacted conversation context:\nSummary: compacted checkpoint facts.\n\nRecent transcript after this compacted checkpoint follows in subsequent messages."
        )
    );
    assert!(
        input_texts
            .iter()
            .any(|text| text == "post checkpoint user")
    );
    assert!(input_texts.iter().any(|text| text == "current turn"));
    assert!(!input_texts.iter().any(|text| text == "covered user"));
}

#[tokio::test]
async fn prompt_context_keeps_all_post_checkpoint_items_for_budgeting() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    for index in 1..=45 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &format!("post checkpoint item {index}"),
        )
        .await;
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    let input = request.input.render_for_token_count();
    assert!(input.contains("post checkpoint item 1"));
    assert!(input.contains("post checkpoint item 45"));
}

#[tokio::test]
async fn prompt_context_sends_prior_transcript_as_provider_messages() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item_with_kind(
        &store,
        &started.conversation_id,
        ConversationItemKind::UserText,
        "first durable question",
    )
    .await;
    append_test_text_item_with_kind(
        &store,
        &started.conversation_id,
        ConversationItemKind::AssistantText,
        "first durable answer",
    )
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            tx,
        )
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    assert_eq!(request.options.prompt_cache_retention, None);
    let GenerateInput::Messages(messages) = &request.input else {
        panic!("expected transcript messages, got {:?}", request.input);
    };
    let observed = messages
        .iter()
        .map(|message| (message.role, message.content.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            (
                crate::provider::GenerateMessageRole::User,
                "first durable question"
            ),
            (
                crate::provider::GenerateMessageRole::Assistant,
                "first durable answer"
            ),
            (
                crate::provider::GenerateMessageRole::User,
                "second durable question"
            ),
        ]
    );
}

#[tokio::test]
async fn prompt_context_falls_back_to_estimates_when_token_count_fails() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: true,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "hello".to_string(),
    )
    .await;
    result.expect("turn should use fallback token estimate");
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    assert!(
        requests
            .iter()
            .any(|request| request.options.require_noema_response)
    );
}

#[tokio::test]
async fn foreground_context_compaction_runs_before_over_limit_turn() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");
    for _ in 0..4 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &"older context ".repeat(400),
        )
        .await;
    }

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect("turn");
    runtime.shutdown().await;

    {
        let requests = provider.requests.lock().expect("requests");
        let compaction_index = requests
            .iter()
            .position(|request| !request.options.require_noema_response)
            .expect("compaction request");
        let agent_index = requests
            .iter()
            .position(|request| request.options.require_noema_response)
            .expect("agent request");
        assert!(compaction_index < agent_index);
    }
    let summaries = store
        .list_context_summaries_for_conversation(&started.conversation_id)
        .await
        .expect("summaries");
    assert!(summaries.iter().any(|summary| {
        summary.provider_kind == "foundation_local"
            && summary.model_profile.is_none()
            && summary.covered_item_end_sequence >= 2
            && summary.summary_text == "fake answer"
    }));
}

#[tokio::test]
async fn foreground_context_compaction_chunks_backlog_to_fit_provider_window() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 5_500,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: true,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");
    for _ in 0..4 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &"older context ".repeat(400),
        )
        .await;
    }

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect("turn should compact oversized backlog in bounded chunks");
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let compaction_requests = requests
        .iter()
        .filter(|request| !request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert!(
        compaction_requests.len() > 1,
        "expected multiple bounded compaction requests"
    );
    for request in compaction_requests {
        let input = request.input.render_for_token_count();
        let input_tokens = request
            .instructions
            .as_deref()
            .map_or(0, estimated_test_tokens)
            + estimated_test_tokens(&input);
        let available = provider
            .context_window_tokens
            .saturating_sub(request.options.max_output_tokens.unwrap_or(512))
            .saturating_sub(128);
        assert!(
            input_tokens <= available,
            "compaction request used {input_tokens} input tokens with {available} available"
        );
    }
    assert!(
        requests
            .iter()
            .any(|request| request.options.require_noema_response)
    );
}

#[tokio::test]
async fn background_context_compaction_creates_checkpoint_after_large_turn() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 18_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");
    append_test_text_item(
        &store,
        &started.conversation_id,
        &"background context ".repeat(2_000),
    )
    .await;

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect("turn");
    wait_for_context_summary_count(&store, &started.conversation_id, 1).await;
    runtime.shutdown().await;

    {
        let requests = provider.requests.lock().expect("requests");
        let agent_index = requests
            .iter()
            .position(|request| request.options.require_noema_response)
            .expect("agent request");
        let compaction_index = requests
            .iter()
            .position(|request| !request.options.require_noema_response)
            .expect("background compaction request");
        assert!(agent_index < compaction_index);
    }
    let active = store
        .latest_active_context_summary(&started.conversation_id, "foundation_local", None)
        .await
        .expect("active summary")
        .expect("active summary exists");
    assert_eq!(active.summary_text, "fake answer");
}

#[tokio::test]
async fn foreground_context_compaction_failure_blocks_turn_with_recoverable_notice() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 4_096,
        fail_compaction: true,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");
    for _ in 0..4 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &"older context ".repeat(400),
        )
        .await;
    }

    let (result, events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect_err("compaction failure");
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    assert!(
        requests
            .iter()
            .any(|request| !request.options.require_noema_response)
    );
    assert!(
        !requests
            .iter()
            .any(|request| request.options.require_noema_response)
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                item,
                ..
            } if matches!(
                item.as_ref(),
                TurnTranscriptItem::ErrorNotice {
                    message,
                    recoverable: true,
                } if message.contains("Context compaction failed before this turn could run")
            )
        )
    }));
}

#[tokio::test]
async fn primary_agent_runtime_preference_selects_provider_without_restart() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect("preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let foundation_provider = Arc::new(CapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_map(
        "codex",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
            (
                "foundation_local".to_string(),
                foundation_provider.clone()
                    as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
        ],
        store,
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let codex_requests = codex_provider.requests.lock().expect("codex requests");
    assert!(codex_requests.is_empty());
    let foundation_requests = foundation_provider
        .requests
        .lock()
        .expect("foundation requests");
    assert_eq!(
        foundation_requests
            .last()
            .and_then(|request| request.model.as_deref()),
        Some("default")
    );
}

#[tokio::test]
async fn runtime_turn_refreshes_agent_preference_after_conversation_hydration() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .update_agent_display_name("agent:primary", "Noema")
        .await
        .expect("name primary");
    let codex_account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex_account.provider_account_id,
            model_profile: "codex-initial".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect("initial preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let foundation_provider = Arc::new(CapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_map(
        "codex",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
            (
                "foundation_local".to_string(),
                foundation_provider.clone()
                    as Arc<dyn crate::daemon::runtime::RuntimeModelProvider>,
            ),
        ],
        store.clone(),
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let foundation_account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: foundation_account.provider_account_id,
            model_profile: "foundation-live".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect("updated preference");

    let (result, _events) =
        collect_turn_events(&runtime, started.conversation_id, "hello".to_string()).await;
    result.expect("turn");
    runtime.shutdown().await;

    assert!(codex_provider.requests.lock().expect("codex").is_empty());
    let foundation_requests = foundation_provider.requests.lock().expect("foundation");
    assert_eq!(
        foundation_requests
            .last()
            .and_then(|request| request.model.as_deref()),
        Some("foundation-live")
    );
}

#[tokio::test]
async fn runtime_turn_rehydrates_recorded_failure_conversation_for_retry() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::TurnError)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (first_result, first_events) =
        collect_turn_events(&handle, conversation_id.clone(), "first".to_string()).await;
    first_result.expect_err("first turn failure");
    assert!(first_events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                item,
                ..
            } if matches!(item.as_ref(), TurnTranscriptItem::ErrorNotice { .. })
        )
    }));

    let (second_result, _second_events) =
        collect_turn_events(&handle, conversation_id.clone(), "second".to_string()).await;
    second_result.expect_err("second provider failure should not become unknown conversation");
    assert_eq!(
        store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_turn_streams_assistant_text_deltas_before_durable_item() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;
    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "hello".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    let first_delta = events
        .iter()
        .position(|event| matches!(event, TurnStreamEvent::AssistantTextDelta { .. }))
        .expect("assistant delta");
    let durable_assistant = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem { item, .. }
                    if matches!(item.as_ref(), TurnTranscriptItem::AssistantText { .. })
            )
        })
        .expect("durable assistant");
    assert!(first_delta < durable_assistant);

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    let assistant_items = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .collect::<Vec<_>>();
    assert_eq!(assistant_items.len(), 1);
    assert_eq!(
        assistant_items[0].content_text.as_deref(),
        Some("fake answer")
    );
}

#[tokio::test]
async fn runtime_turn_streams_tool_call_started_before_durable_response_items() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::SearchMemoryContinuation))
            .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await;
    result.expect("turn");
    handle.shutdown().await;

    let streamed_text = events
        .iter()
        .position(|event| matches!(event, TurnStreamEvent::AssistantTextDelta { .. }))
        .expect("assistant text delta");
    let streamed_tool_started = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem { item_id, item, .. }
                    if item_id.starts_with("transient:tool_call:")
                        && matches!(
                            item.as_ref(),
                            TurnTranscriptItem::Activity {
                                activity_kind,
                                status: TurnActivityStatus::Started,
                                title,
                                ..
                            } if activity_kind == "tool_call" && title == "Tool call: search_memory"
                        )
            )
        })
        .expect("transient tool call started item");
    let durable_commentary = assistant_text_item_event_index(&events, "Searching memory.")
        .expect("durable commentary item");

    assert!(
        streamed_text < streamed_tool_started,
        "tool marker should not appear before streamed assistant commentary: {events:?}"
    );
    assert!(
        streamed_tool_started < durable_commentary,
        "tool marker should appear while the provider response is still streaming: {events:?}"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let durable_assistant = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::AssistantText)
        .expect("durable assistant item");
    let durable_tool = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("durable tool item");
    assert!(
        durable_assistant < durable_tool,
        "replay should keep durable commentary before durable tool execution: {replay:?}"
    );
}

#[tokio::test]
async fn runtime_primary_conversation_sends_recent_durable_context_after_restart() {
    if let Ok(phase) = std::env::var(RESTART_CONTEXT_TEST_PHASE_ENV) {
        let home = PathBuf::from(
            std::env::var(RESTART_CONTEXT_TEST_HOME_ENV).expect("restart test home env"),
        );
        match phase.as_str() {
            "write" => restart_context_write_phase(&home).await,
            "read" => restart_context_read_phase(&home).await,
            other => panic!("unknown restart context test phase: {other}"),
        }
        return;
    }

    let home = tempfile::tempdir().expect("temp noema home");
    run_restart_context_child_phase("write", home.path());
    run_restart_context_child_phase("read", home.path());
}

#[tokio::test]
async fn runtime_prompt_includes_unnamed_agent_onboarding() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::IdentityPromptCheck)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw unnamed identity"
    )));
}

#[tokio::test]
async fn runtime_provider_prompt_includes_assistant_text_phase_contract() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::PromptPhaseContract)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw phase contract"
    )));
}

#[tokio::test]
async fn runtime_provider_prompt_allows_markdown_in_assistant_text() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::PromptMarkdownContract)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw markdown contract"
    )));
}

#[tokio::test]
async fn start_primary_conversation_generates_initial_name_onboarding_message() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::InitialNameOnboarding))
            .await;

    let conversation_id = handle
        .start_primary_conversation(None)
        .await
        .expect("primary conversation")
        .conversation_id;
    handle.shutdown().await;

    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        !items
            .iter()
            .any(|item| item.kind == ConversationItemKind::UserText),
        "initial onboarding should not fake a user message: {items:?}"
    );
    let assistant_texts: Vec<_> = items
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .filter_map(|item| item.content_text.as_deref())
        .collect();
    assert_eq!(
        assistant_texts,
        vec![
            "hey, i’m glad to be here with you 👋",
            "i can help you think, plan, make, untangle, and keep life moving with a little more ease",
            "what would you like to name me?",
        ]
    );
}

#[tokio::test]
async fn failed_initial_name_onboarding_logs_runtime_invariant() {
    let (handle, store) = test_runtime_handle_with_store(fake_provider(
        FakeCodexScenario::InitialNameOnboardingNoAssistant,
    ))
    .await;

    let error = handle
        .start_primary_conversation(None)
        .await
        .expect_err("onboarding should fail");

    assert!(
        error
            .to_string()
            .contains("initial onboarding response did not include assistant text")
    );
    let logger = store.system_error_logger();
    let events = crate::system_errors::read_system_error_events(logger.path()).expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["category"], crate::SYSTEM_ERROR_RUNTIME_INVARIANT);
    assert_eq!(
        events[0]["message"],
        "initial onboarding response did not include assistant text"
    );
    assert_eq!(events[0]["raw"]["persisted_count"], 0);
    assert_eq!(
        events[0]["raw"]["provider_response"]["responses"],
        json!([])
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn failed_initial_name_onboarding_recomputes_turn_index_on_retry() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::TurnError)).await;

    assert!(handle.start_primary_conversation(None).await.is_err());
    assert!(handle.start_primary_conversation(None).await.is_err());
    handle.shutdown().await;

    let conversation_id = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation")
        .conversation_id;
    assert_eq!(
        store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
}

fn run_restart_context_child_phase(phase: &str, home: &std::path::Path) {
    let output = Command::new(std::env::current_exe().expect("current test binary"))
        .arg("daemon::tests::runtime_primary_conversation_sends_recent_durable_context_after_restart")
        .arg("--exact")
        .env(RESTART_CONTEXT_TEST_PHASE_ENV, phase)
        .env(RESTART_CONTEXT_TEST_HOME_ENV, home)
        .output()
        .expect("run restart context test phase");
    assert!(
        output.status.success(),
        "restart context {phase} phase failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn restart_context_write_phase(home: &std::path::Path) {
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let first_store = crate::NoemaStore::open(&config)
        .await
        .expect("open first store");
    let first_handle = CodexRuntimeHandle::spawn_with_provider(
        Arc::new(fake_provider(FakeCodexScenario::RestartContext)),
        first_store.clone(),
    )
    .await
    .expect("first runtime");
    let first_conversation_id = first_handle
        .start_primary_conversation(None)
        .await
        .expect("first primary conversation")
        .conversation_id;
    let first_items = collect_turn(
        &first_handle,
        first_conversation_id.clone(),
        "first durable question".to_string(),
    )
    .await
    .expect("first turn");
    assert_eq!(assistant_text(&first_items), "fake answer");
    first_handle.shutdown().await;
    std::fs::write(
        home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE),
        &first_conversation_id,
    )
    .expect("write restart conversation id");
    first_store.close().await.expect("close first store");
}

async fn restart_context_read_phase(home: &std::path::Path) {
    let first_conversation_id =
        std::fs::read_to_string(home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE))
            .expect("read restart conversation id");
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let reopened_store = crate::NoemaStore::open(&config)
        .await
        .expect("reopen store");
    let second_handle = CodexRuntimeHandle::spawn_with_provider(
        Arc::new(fake_provider(FakeCodexScenario::RestartContext)),
        reopened_store.clone(),
    )
    .await
    .expect("second runtime");
    let restarted_conversation_id = second_handle
        .start_primary_conversation(None)
        .await
        .expect("restarted primary conversation")
        .conversation_id;
    assert_eq!(first_conversation_id, restarted_conversation_id);

    let second_items = collect_turn(
        &second_handle,
        restarted_conversation_id,
        "second durable question".to_string(),
    )
    .await
    .expect("second turn");
    assert_eq!(assistant_text(&second_items), "saw durable context");
    second_handle.shutdown().await;

    let replay = reopened_store
        .list_conversation_items(&first_conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let user_texts = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::UserText)
        .map(|item| item.content_text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        user_texts,
        vec![
            Some("first durable question"),
            Some("second durable question")
        ]
    );
    reopened_store.close().await.expect("close reopened store");
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_as_action_rows() {
    let (handle, store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::ToolItem),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));

    let started_tool_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_call"
                    && *status == TurnActivityStatus::Started
                    && id.starts_with("tool_call:")
            )
        })
        .expect("started tool call marker");
    let completed_result_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_result"
                    && *status == TurnActivityStatus::Completed
                    && id.starts_with("tool_result:")
            )
        })
        .expect("completed tool result marker");
    assert!(
        started_tool_position < completed_result_position,
        "tool call should appear as started before its result completes"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolCall
                && item.status == ConversationItemStatus::Running
                && item.payload_json["activity_kind"] == "tool_call"
                && item.payload_json["metadata"]["action"]["name"] == "search_memory"
        }),
        "expected replayed tool call item, got {replay:?}"
    );
    let tool_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("tool call item");
    let assistant_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::AssistantText)
        .expect("assistant item");
    assert!(
        assistant_position < tool_position,
        "assistant commentary should replay before runtime tool execution: {replay:?}"
    );
}

#[tokio::test]
async fn runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::ToolCallBeforeCommentary),
        json!({"results": []}),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Check memory.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let commentary_position = items
        .iter()
        .position(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Checking memory.")
        })
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: search_memory"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .expect("tool started item");
    let tool_result_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "tool_result" && title == "Tool result: search_memory"
            )
        })
        .expect("tool result item");

    assert!(
        commentary_position < tool_started_position,
        "commentary should describe intent before runtime tool execution starts: {items:?}"
    );
    assert!(
        tool_started_position < tool_result_position,
        "tool lifecycle should start before its result: {items:?}"
    );
}

#[tokio::test]
async fn runtime_keeps_commentary_before_tool_lifecycle_when_provider_orders_text_first() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::SearchMemoryContinuation)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let commentary_position = items
        .iter()
        .position(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Searching memory.")
        })
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: search_memory"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .expect("tool started item");
    let final_position = items
        .iter()
        .rposition(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "I found your train memory.")
        })
        .expect("final answer item");

    assert!(commentary_position < tool_started_position, "{items:?}");
    assert!(tool_started_position < final_position, "{items:?}");
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_before_turn_failure() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ToolItemThenFailure)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await;
    assert!(matches!(result, Err(DaemonError::Provider(_))));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Error,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id: Some(_),
                item,
                ..
            } if id == &conversation_id
                && item_id.starts_with("item:")
                && matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status: TurnActivityStatus::Completed,
                        title,
                        ..
                    } if activity_kind == "tool_call"
                        && title == "Tool call: search_memory"
                )
        )
    }));
    let items = transcript_items_from_events(events);
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
}

#[tokio::test]
async fn runtime_actor_executes_search_memory_as_local_tool_result() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_train",
                "memory": "Kevin likes trains.",
                "metadata": {"source": "test"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: search_memory"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["memories"]
                .as_array()
                .is_some_and(|memories| memories.iter().any(|memory| {
                    memory["kind"] == "mnemosyne"
                        && memory["memory"] == "Kevin likes trains."
                        && memory["scope_id"]
                            .as_str()
                            .is_some_and(|scope_id| scope_id.starts_with("conversation:"))
                }))
            && metadata["action"]["payload"].get("unavailable").is_none()
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I found your train memory."
    )));
    handle.shutdown().await;
}

#[tokio::test]
async fn user_message_submits_memory_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::Simple),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, _events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "remember this turn".to_string(),
    )
    .await;
    result.expect("turn");
    wait_for_memory_observation_requests(&server, 1).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    assert!(bodies.iter().any(|body| {
        body["user_id"] == "human:local"
            && body["agent_id"] == "agent:local"
            && body["run_id"] == conversation_id
            && body["metadata"]["noemaConversationId"] == conversation_id
            && body["metadata"]["sourceKind"] == "user_message"
            && body["metadata"]["turnId"]
                .as_str()
                .is_some_and(|turn_id| turn_id.starts_with("turn:"))
            && body["metadata"]["userItemId"]
                .as_str()
                .is_some_and(|item_id| item_id.starts_with("item:"))
            && body["messages"].as_array().is_some_and(|messages| {
                messages.as_slice() == [json!({"role": "user", "content": "remember this turn"})]
            })
    }));
}

#[tokio::test]
async fn provider_failure_after_user_message_still_submits_memory_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::TurnError),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, _events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "remember even if generation fails".to_string(),
    )
    .await;
    assert!(result.is_err());
    wait_for_memory_observation_requests(&server, 1).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    assert!(bodies.iter().any(|body| {
        body["messages"]
            == json!([{"role": "user", "content": "remember even if generation fails"}])
    }));
}

#[tokio::test]
async fn slow_memory_ingest_does_not_delay_provider_response() {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    store.ensure_default_actors().await.expect("actors");
    let server = FakeMemoryServer::start_with_add_delay(
        json!({"results": []}),
        1,
        Some(Duration::from_secs(5)),
    )
    .await;
    std::mem::forget(home);
    let connection = crate::MnemosyneConnection::new(server.base_url(), None);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(fake_provider(FakeCodexScenario::Simple)),
        store,
        Some(connection),
    )
    .await
    .expect("runtime");

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let result = tokio::time::timeout(
        Duration::from_millis(500),
        collect_turn_events(
            &handle,
            conversation_id,
            "do not wait for memory indexing".to_string(),
        ),
    )
    .await
    .expect("turn should not wait for Mnemosyne response")
    .0;
    handle.shutdown().await;

    result.expect("turn");
}

#[tokio::test]
async fn memory_observation_uses_distinct_source_ids_and_current_user_source_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::Simple),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "first turn should not repeat".to_string(),
    )
    .await
    .0
    .expect("first turn");
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "second turn should be submitted".to_string(),
    )
    .await
    .0
    .expect("second turn");
    wait_for_memory_observation_requests(&server, 2).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    let observation_bodies = bodies
        .iter()
        .filter(|body| body["metadata"]["noemaConversationId"] == conversation_id)
        .collect::<Vec<_>>();
    assert_eq!(observation_bodies.len(), 2);
    let source_item_ids = observation_bodies
        .iter()
        .filter_map(|body| body["metadata"]["userItemId"].as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(source_item_ids.len(), 2);
    assert!(
        source_item_ids
            .iter()
            .all(|source_item_id| source_item_id.starts_with("item:"))
    );
    assert!(observation_bodies.iter().any(|body| {
        body["metadata"]["sourceObservation"] == "first turn should not repeat"
            && body["messages"]
                == json!([{"role": "user", "content": "first turn should not repeat"}])
    }));
    assert!(observation_bodies.iter().any(|body| {
        body["metadata"]["sourceObservation"] == "second turn should be submitted"
            && body["messages"].as_array().is_some_and(|messages| {
                messages.last()
                    == Some(&json!({"role": "user", "content": "second turn should be submitted"}))
            })
    }));
}

#[tokio::test]
async fn memory_observation_includes_assistant_context_since_previous_user() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::MemoryContextQuestion),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "start memory context test".to_string(),
    )
    .await
    .0
    .expect("first turn");
    collect_turn_events(&handle, conversation_id.clone(), "cars".to_string())
        .await
        .0
        .expect("second turn");
    wait_for_memory_observation_requests(&server, 2).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    let observation = bodies
        .iter()
        .find(|body| body["metadata"]["sourceObservation"] == "cars")
        .expect("cars observation");
    assert_eq!(
        observation["messages"],
        json!([
            {
                "role": "assistant",
                "content": "what are some topics you find interesting?"
            },
            {
                "role": "assistant",
                "content": "short answers are fine too"
            },
            {"role": "user", "content": "cars"}
        ])
    );
}

#[tokio::test]
async fn search_memory_skips_mnemosyne_results_without_memory() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [
                {
                    "id": "mem_missing_text",
                    "metadata": {"source": "test"},
                    "updated_at": "2026-07-07T12:00:00.000Z",
                    "score": 0.99
                },
                {
                    "id": "mem_train",
                    "memory": "Kevin likes trains.",
                    "metadata": {"source": "test"},
                    "updated_at": "2026-07-07T12:00:00.000Z",
                    "score": 0.91
                }
            ],
            "timing": 2,
            "total": 2
        }),
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "tool_result" && title == "Tool result: search_memory" => {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("search_memory tool result, got {items:?}"));
    let memories = payload["memories"].as_array().expect("memories");
    assert!(!memories.is_empty());
    assert!(memories.iter().all(|memory| {
        memory["id"] == "mem_train" && memory["memory"] == "Kevin likes trains."
    }));
}

#[tokio::test]
async fn search_memory_omits_raw_mnemosyne_metadata() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_train",
                "memory": "Kevin likes trains.",
                "metadata": {"raw": "secret provider detail"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "tool_result" && title == "Tool result: search_memory" => {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .expect("search_memory tool result");
    assert!(payload["memories"][0].get("metadata").is_none());
}

#[tokio::test]
async fn search_memory_returns_sanitized_mnemosyne_failure() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": [{}]}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                title,
                metadata,
                ..
            } if activity_kind == "tool_result"
                && title == "Tool result: search_memory"
                && metadata["action"]["success"] == false =>
            {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("failed search_memory tool result, got {items:?}"));
    assert_eq!(
        payload,
        &json!({
            "error": {
                "code": "decode_failed",
                "message": "memory service returned an unreadable response"
            }
        })
    );
}

#[tokio::test]
async fn runtime_actor_executes_web_search_as_local_tool_result() {
    let search_provider = crate::search::types::SearchRuntimeProvider::Static {
        response: crate::search::types::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 1 web result".to_string(),
            results: vec![crate::search::types::SearchResult {
                rank: 1,
                title: "Rust Programming Language".to_string(),
                url: "https://www.rust-lang.org/".to_string(),
                snippet: "A language empowering everyone to build reliable software.".to_string(),
            }],
        },
    };
    let (handle, store) = test_runtime_handle_with_search_provider(
        Arc::new(fake_provider(FakeCodexScenario::NativeWebSearch)),
        search_provider,
    )
    .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search the web for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Audit)
        .await
        .expect("items");

    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolCall)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["display"]["target"] == "rust language"
    }));
    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolResult)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["action"]["success"] == true
            && item.payload_json["metadata"]["action"]["payload"]["provider"] == "duckduckgo_public"
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn native_capable_provider_continuation_uses_native_tool_result_input() {
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeSearchMemoryContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                tool_choice: true,
                schema_dialect: crate::provider::ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                prompt_cache_retention: true,
                prompt_cache_key: false,
                encrypted_reasoning: false,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
            }),
    );
    let (handle, _store, _server) = spawn_runtime_with_memory_provider(
        provider.clone(),
        json!({
            "results": [{
                "id": "mem_native_train",
                "memory": "Kevin likes trains.",
                "metadata": {},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.9
            }]
        }),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::AssistantText { text }
                    if text == "native tool result received"
            )
        }),
        "expected native continuation final answer: {items:?}"
    );
    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected initial request and continuation"
    );
    let GenerateInput::NativeToolResults(results) = &requests[1].input else {
        panic!(
            "expected native tool-result input, got {:?}",
            requests[1].input
        );
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id.as_deref(), Some("item_native_1"));
    assert_eq!(results[0].call_id, "call_native_1");
    assert_eq!(results[0].name, "search_memory");
    assert_eq!(results[0].provider_name.as_deref(), Some("search_memory"));
    assert_eq!(results[0].arguments["arguments"]["query"], "trains");
    assert!(results[0].success);
    assert!(
        !requests[1]
            .input
            .render_for_token_count()
            .contains("NOEMA_LOCAL_TOOL_RESULT")
    );
}

#[tokio::test]
async fn web_search_result_is_sent_as_native_tool_result_input() {
    let search_provider = crate::search::types::SearchRuntimeProvider::Static {
        response: crate::search::types::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 1 web result".to_string(),
            results: vec![crate::search::types::SearchResult {
                rank: 1,
                title: "Rust Programming Language".to_string(),
                url: "https://www.rust-lang.org/".to_string(),
                snippet: "A language empowering everyone to build reliable software.".to_string(),
            }],
        },
    };
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeWebSearchContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store) =
        test_runtime_handle_with_search_provider(provider.clone(), search_provider).await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let GenerateInput::NativeToolResults(results) = &request.input else {
            return false;
        };
        results.iter().any(|result| {
            result.name == "web.search"
                && result.success
                && result.payload["provider"] == "duckduckgo_public"
                && result
                    .payload
                    .get("results")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|results| !results.is_empty())
        })
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn native_provider_can_call_web_fetch_and_continue() {
    let fetch_response = crate::web_fetch::types::FetchResponse {
        provider: crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID.to_string(),
        url: String::new(),
        final_url: "https://example.com/page".to_string(),
        title: Some("Example Page".to_string()),
        format: "markdown".to_string(),
        extraction: crate::web_fetch::types::EXTRACTION_READABILITYRS.to_string(),
        content_kind: crate::web_fetch::types::FetchContentKind::RawMarkdown,
        content: "Example fetched page content.".to_string(),
        raw_excerpt: None,
        raw_chars: 29,
        returned_chars: 29,
        summary_model: None,
        summary_strategy: crate::web_fetch::types::FetchSummaryStrategy::NotSummarized,
        truncated: false,
    };
    let web_fetch_provider = crate::web_fetch::types::WebFetchRuntimeProvider::Static {
        response: Box::new(fetch_response),
    };
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeWebFetchContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store) =
        test_runtime_handle_with_search_and_fetch_providers(provider.clone(), web_fetch_provider)
            .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    let items = collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Fetch the example page.".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I read the fetched page."
    )));
    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let GenerateInput::NativeToolResults(results) = &request.input else {
            return false;
        };
        results.iter().any(|result| {
            result.name == "web.fetch"
                && result.success
                && result.payload["provider"] == crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID
                && result.payload["content"] == "Example fetched page content."
        })
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_continues_after_continuation_tool_call() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::ChainedSearchMemoryContinuation),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Check memory twice before answering.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let completed_search_results = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "tool_result" && title == "Tool result: search_memory"
            )
        })
        .count();
    assert_eq!(completed_search_results, 2, "{items:?}");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I checked both memory topics."
    )));
}

#[tokio::test]
async fn search_memory_uses_private_runtime_memory_endpoint() {
    let (handle, _store, server) = test_runtime_handle_with_private_memory(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_private",
                "memory": "Kevin prefers private sidecars",
                "updated_at": "2026-07-08T12:00:00.000Z",
                "score": 0.94
            }]
        }),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_result"
                    && title == "Tool result: search_memory"
                    && metadata["action"]["payload"]["memories"]
                        .as_array()
                        .is_some_and(|memories| memories.iter().any(|memory| {
                            memory["id"] == "mem_private"
                        }))
            )
        }),
        "expected private memory result in tool output"
    );
    assert!(!server.request_bodies().await.is_empty());
}

#[tokio::test]
async fn hard_ceiling_gets_one_no_tools_finalization_attempt() {
    let provider = Arc::new(RecordingFakeProvider::new(
        "codex",
        FakeCodexScenario::LongContinuationThenFinalization,
    ));
    let (handle, store) = test_runtime_handle_with_search_provider(
        provider.clone(),
        crate::search::types::SearchRuntimeProvider::Static {
            response: crate::search::types::SearchResponse {
                provider: "test".to_string(),
                provider_contract: "test".to_string(),
                query: "restaurants".to_string(),
                summary: "Found 1 test result".to_string(),
                results: vec![],
            },
        },
    )
    .await;
    store.ensure_default_actors().await.expect("actors");
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::High),
        })
        .await
        .expect("runtime preference");
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id,
        "Research healthy restaurants and keep going.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let requests = provider.requests();
    let finalization_requests: Vec<_> = requests
        .iter()
        .filter(|request| {
            request.tools.is_empty()
                && !request.parallel_tool_calls
                && request
                    .instructions
                    .as_deref()
                    .is_some_and(|instructions| instructions.contains("must stop now"))
        })
        .collect();
    assert_eq!(finalization_requests.len(), 1);
    assert_eq!(
        finalization_requests[0].options.reasoning_effort,
        Some(crate::provider::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn audit_execution_failure_gets_one_no_tools_finalization_attempt() {
    let provider = Arc::new(RecordingFakeProvider::new(
        "codex",
        FakeCodexScenario::ProgressAuditFailsThenFinalization,
    ));
    let (handle, store) = test_runtime_handle_with_search_provider(
        provider.clone(),
        crate::search::types::SearchRuntimeProvider::Static {
            response: crate::search::types::SearchResponse {
                provider: "test".to_string(),
                provider_contract: "test".to_string(),
                query: "restaurants".to_string(),
                summary: "Found 1 test result".to_string(),
                results: vec![],
            },
        },
    )
    .await;
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::store::TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex.provider_account_id,
            model_profile: "gpt-5.4-mini".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect("audit preference");
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id,
        "Research healthy restaurants and keep going.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let requests = provider.requests();
    let finalization_requests = requests
        .iter()
        .filter(|request| {
            request.tools.is_empty()
                && !request.parallel_tool_calls
                && request
                    .instructions
                    .as_deref()
                    .is_some_and(|instructions| instructions.contains("must stop now"))
        })
        .count();
    assert_eq!(finalization_requests, 1);
}

#[tokio::test]
async fn update_own_name_tool_updates_agent_and_continues_turn() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::UpdateOwnNameContinuation))
            .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "Hey! How about Fred?".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Fred"));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: update_own_name"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["display_name"] == "Fred"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "Fred it is. what would you like help with first?"
    )));
}

#[tokio::test]
async fn update_own_name_tool_does_not_start_repeated_continuation_tool_calls() {
    let (handle, store) = test_runtime_handle_with_store(fake_provider(
        FakeCodexScenario::RepeatedUpdateOwnNameContinuation,
    ))
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "Hey! How about Fred?".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Fred"));

    let update_name_tool_calls = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: update_own_name"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .count();
    assert_eq!(update_name_tool_calls, 1, "{items:?}");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Fred it is."
    )));
}

#[tokio::test]
async fn update_own_name_tool_history_is_visible_before_later_turns() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::UpdateOwnNameThenYay))
            .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Let's rename you to Momo".to_string(),
    )
    .await
    .expect("rename turn");
    let items = collect_turn(&handle, conversation_id, "Yay".to_string())
        .await
        .expect("yay turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Momo"));
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "yay acknowledged after saved name"
    )));
}

#[tokio::test]
async fn ambiguous_name_suggestion_asks_confirmation_without_tool_call() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::AmbiguousUpdateOwnName))
            .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "Maybe you could be Mira?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name, None);
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "Please confirm what you'd like to call me."
    )));
}

#[tokio::test]
async fn runtime_prompt_includes_stored_agent_name_after_update() {
    let handle = test_runtime_handle(fake_provider(
        FakeCodexScenario::UpdateOwnNameThenIdentityCheck,
    ))
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Your name is Mira.".to_string(),
    )
    .await
    .expect("name turn");
    let items = collect_turn(&handle, conversation_id, "What is your name?".to_string())
        .await
        .expect("identity turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw stored identity"
    )));
}

#[tokio::test]
async fn search_memory_profile_continuation_uses_scoped_empty_query() {
    let (handle, store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryProfileContinuation),
        json!({
            "results": [{
                "id": "mem_plane",
                "memory": "Kevin likes planes.",
                "metadata": {"source": "test"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.93
            }]
        }),
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like planes.".to_string(),
    )
    .await
    .expect("seed turn");

    collect_turn(
        &handle,
        conversation_id.clone(),
        "What memories do you have of me?".to_string(),
    )
    .await
    .expect("profile search turn");
    handle.shutdown().await;

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolResult
                && item.status == ConversationItemStatus::Completed
                && item.payload_json["metadata"]["action"]["success"] == true
                && item.payload_json["metadata"]["action"]["payload"]["scope_ids"]
                    == json!(["human:local"])
                && item.payload_json["metadata"]["action"]["payload"]["memories"]
                    .as_array()
                    .is_some_and(|memories| {
                        memories.iter().any(|memory| {
                            memory["kind"] == "mnemosyne"
                                && memory["memory"] == "Kevin likes planes."
                                && memory["scope_id"] == "human:local"
                        })
                    })
        }),
        "expected persisted successful scoped profile tool result, got {replay:?}"
    );
    let request_bodies = server.request_bodies().await;
    assert!(
        request_bodies.iter().any(|body| {
            body["query"] == "" && body["user_id"] == "human:local" && body["limit"] == 8
        }),
        "expected scoped empty-query memory search, got {request_bodies:?}"
    );
}

#[tokio::test]
async fn search_memory_uses_runtime_connection_until_restart() {
    let second_server = FakeMemoryServer::start(
        json!({"results": [{"id": "new", "memory": "new memory", "score": 0.9}]}),
        32,
    )
    .await;
    let (handle, store, first_server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": [{"id": "old", "memory": "old memory", "score": 0.1}]}),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("first turn");
    store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            mode: crate::MemoryServiceMode::External,
            base_url: Some(second_server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save second settings");
    let second_conversation_id = handle
        .start_conversation(None)
        .await
        .expect("second conversation")
        .conversation_id;

    collect_turn(
        &handle,
        second_conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    assert!(!first_server.request_bodies().await.is_empty());
    assert!(second_server.request_bodies().await.is_empty());
}

#[tokio::test]
async fn search_memory_tool_returns_empty_mnemosyne_result_without_unavailable() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("tool result payload");
    assert_eq!(payload["omissions"], json!([]));
    assert!(payload.get("unavailable").is_none());
    assert!(payload.get("context_packet_id").is_none());
    assert!(
        payload["memories"]
            .as_array()
            .is_some_and(|memories| memories.is_empty())
    );
}

#[tokio::test]
async fn search_memory_tool_invalid_arguments_are_failed_tool_result() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::InvalidSearchMemory)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("failed tool result payload");
    assert_eq!(payload["error"], "unsupported purpose: dump_everything");
}

#[tokio::test]
async fn uncalibrated_mcp_tool_call_returns_failed_tool_result() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::UncalibratedMcpToolCall))
            .await;
    seed_enabled_uncalibrated_mcp_tool(&store).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(&handle, conversation_id.clone(), "read my doc".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation items");
    assert!(
        items.iter().any(|item| {
            item.kind == ConversationItemKind::ToolResult
                && item.status == ConversationItemStatus::Failed
                && item.payload_json["metadata"]["action"]["name"] == "mcp.docs.read"
                && item.payload_json["metadata"]["action"]["payload"]["error"]
                    == "mcp_tool_not_calibrated"
        }),
        "expected failed uncalibrated MCP tool result, got {items:?}"
    );
}

#[tokio::test]
async fn failed_mcp_tool_result_continues_to_provider() {
    let handle = test_runtime_handle(fake_provider(
        FakeCodexScenario::FailedMcpToolResultContinuation,
    ))
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Create a Notion page".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Failed,
            title,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: mcp.mcp:notion.notion-create-pages"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "I saw the Notion tool failure and can explain it."
    )));
}

async fn seed_enabled_uncalibrated_mcp_tool(store: &crate::NoemaStore) {
    store
        .create_mcp_server(crate::NewMcpServer {
            mcp_server_id: "docs".to_string(),
            display_name: "Docs".to_string(),
            transport_kind: crate::McpTransportKind::Stdio,
            safe_config: json!({"command": "fake-docs-mcp"}),
        })
        .await
        .expect("create MCP server");
    store
        .update_mcp_server_setup_status(
            "docs",
            crate::McpServerHealthStatus::Healthy,
            crate::McpServerAuthStatus::None,
        )
        .await
        .expect("mark MCP healthy");
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE mcp_servers SET enabled = 1 WHERE mcp_server_id = 'docs'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("enable server");
    store
        .upsert_discovered_mcp_tool(crate::NewMcpTool {
            mcp_tool_id: "mcp_tool:docs:read".to_string(),
            mcp_server_id: "docs".to_string(),
            name: "read".to_string(),
            description: Some("Read a document".to_string()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({}),
            metadata_fingerprint: "fingerprint:docs:read:v1".to_string(),
        })
        .await
        .expect("upsert MCP tool");
}

async fn collect_turn(
    handle: &CodexRuntimeHandle,
    conversation_id: String,
    input: String,
) -> Result<Vec<TurnTranscriptItem>, DaemonError> {
    let (result, events) = collect_turn_events(handle, conversation_id, input).await;
    result?;
    Ok(transcript_items_from_events(events))
}

async fn collect_turn_events(
    handle: &CodexRuntimeHandle,
    conversation_id: String,
    input: String,
) -> (Result<(), DaemonError>, Vec<TurnStreamEvent>) {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let result = handle.turn(conversation_id, input, item_tx).await;
    let mut events = Vec::new();
    while let Ok(event) = item_rx.try_recv() {
        events.push(event);
    }
    (result, events)
}

fn transcript_items_from_events(events: Vec<TurnStreamEvent>) -> Vec<TurnTranscriptItem> {
    events
        .into_iter()
        .filter_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. }
                if !matches!(item.as_ref(), TurnTranscriptItem::UserText { .. }) =>
            {
                Some(*item)
            }
            TurnStreamEvent::ConversationItem { .. }
            | TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .collect()
}

fn assistant_text_item_event_index(
    events: &[TurnStreamEvent],
    expected_text: &str,
) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::AssistantText { text } if text == expected_text
                )
        )
    })
}
fn assistant_text(items: &[TurnTranscriptItem]) -> &str {
    let Some(text) = items.iter().find_map(|item| match item {
        TurnTranscriptItem::AssistantText { text } => Some(text.as_str()),
        TurnTranscriptItem::UserText { .. }
        | TurnTranscriptItem::MultipleChoicePrompt { .. }
        | TurnTranscriptItem::Activity { .. }
        | TurnTranscriptItem::A2uiCard { .. }
        | TurnTranscriptItem::ErrorNotice { .. } => None,
    }) else {
        panic!("expected assistant text item, got {items:?}");
    };
    text
}

async fn test_runtime_handle(provider: FakeCodexProvider) -> CodexRuntimeHandle {
    test_runtime_handle_with_store(provider).await.0
}

async fn test_runtime_handle_with_store(
    provider: FakeCodexProvider,
) -> (CodexRuntimeHandle, crate::NoemaStore) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    (handle, store)
}

async fn test_runtime_handle_with_mnemosyne(
    provider: FakeCodexProvider,
    response: serde_json::Value,
) -> (CodexRuntimeHandle, crate::NoemaStore, FakeMemoryServer) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let server = FakeMemoryServer::start(response, 32).await;
    store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            mode: crate::MemoryServiceMode::External,
            base_url: Some(server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save memory settings");
    std::mem::forget(home);
    let connection = crate::MnemosyneConnection::new(server.base_url(), None);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store.clone(),
        Some(connection),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn test_runtime_handle_with_private_memory(
    provider: FakeCodexProvider,
    response: serde_json::Value,
) -> (CodexRuntimeHandle, crate::NoemaStore, FakeMemoryServer) {
    let store = crate::store::tests::test_store().await;
    let server = FakeMemoryServer::start(response, 32).await;
    let connection = crate::MnemosyneConnection::new(server.base_url(), None);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store.clone(),
        Some(connection),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn spawn_runtime_with_memory_provider(
    provider: Arc<dyn super::runtime::RuntimeModelProvider>,
    response: serde_json::Value,
) -> (CodexRuntimeHandle, crate::NoemaStore, FakeMemoryServer) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    store.ensure_default_actors().await.expect("actors");
    let server = FakeMemoryServer::start(response, 32).await;
    store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            mode: crate::MemoryServiceMode::External,
            base_url: Some(server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save memory settings");
    std::mem::forget(home);
    let connection = crate::MnemosyneConnection::new(server.base_url(), None);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_memory(
        provider,
        store.clone(),
        Some(connection),
    )
    .await
    .expect("runtime");
    (handle, store, server)
}

async fn test_runtime_handle_with_search_provider(
    provider: Arc<dyn super::runtime::RuntimeModelProvider>,
    search_provider: crate::search::types::SearchRuntimeProvider,
) -> (CodexRuntimeHandle, crate::NoemaStore) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_search_provider(
        provider,
        store.clone(),
        search_provider,
    )
    .await
    .expect("runtime");
    (handle, store)
}

struct FakeMemoryServer {
    base_url: String,
    state: Arc<AsyncMutex<FakeMemoryState>>,
}

#[derive(Default)]
struct FakeMemoryState {
    bodies: Vec<serde_json::Value>,
    paths: Vec<String>,
}

impl FakeMemoryServer {
    async fn start(response: serde_json::Value, max_requests: usize) -> Self {
        Self::start_with_add_delay(response, max_requests, None).await
    }

    async fn start_with_add_delay(
        response: serde_json::Value,
        max_requests: usize,
        conversation_delay: Option<Duration>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let state = Arc::new(AsyncMutex::new(FakeMemoryState::default()));
        let server_state = Arc::clone(&state);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buffer = vec![0_u8; 8192];
                let read = stream.read(&mut buffer).await.expect("read");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let Some((head, body)) = request.split_once("\r\n\r\n") else {
                    continue;
                };
                let mut lines = head.lines();
                let request_line = lines.next().expect("request line");
                let path = request_line
                    .strip_prefix("POST ")
                    .and_then(|value| value.strip_suffix(" HTTP/1.1"))
                    .expect("POST request line")
                    .to_string();
                assert!(matches!(
                    path.as_str(),
                    "/v1/memories/search" | "/v1/memories/add"
                ));

                let mut headers = HashMap::new();
                for line in lines {
                    if let Some((name, value)) = line.split_once(':') {
                        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
                    }
                }
                let content_length = headers
                    .get("content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body_bytes = body.as_bytes().to_vec();
                while body_bytes.len() < content_length {
                    let read = stream.read(&mut buffer).await.expect("read body");
                    if read == 0 {
                        break;
                    }
                    body_bytes.extend_from_slice(&buffer[..read]);
                }
                let body_json = serde_json::from_slice(&body_bytes).expect("request body JSON");
                {
                    let mut state = server_state.lock().await;
                    state.paths.push(path.clone());
                    state.bodies.push(body_json);
                }

                let response_body = if path == "/v1/memories/add" {
                    if let Some(delay) = conversation_delay {
                        tokio::time::sleep(delay).await;
                    }
                    b"{}".to_vec()
                } else {
                    serde_json::to_vec(&response).expect("response JSON")
                };
                let response_head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    response_body.len()
                );
                stream
                    .write_all(response_head.as_bytes())
                    .await
                    .expect("write head");
                stream.write_all(&response_body).await.expect("write body");
            }
        });

        Self { base_url, state }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    async fn request_bodies(&self) -> Vec<serde_json::Value> {
        self.state.lock().await.bodies.clone()
    }
}

async fn wait_for_memory_observation_requests(server: &FakeMemoryServer, minimum_count: usize) {
    for _ in 0..50 {
        let bodies = server.request_bodies().await;
        let observations = bodies
            .iter()
            .filter(|body| body["metadata"]["sourceKind"] == "user_message")
            .count();
        if observations >= minimum_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {minimum_count} memory observation requests");
}

async fn test_runtime_handle_with_search_and_fetch_providers(
    provider: Arc<dyn super::runtime::RuntimeModelProvider>,
    web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
) -> (CodexRuntimeHandle, crate::NoemaStore) {
    let search_provider = crate::search::types::SearchRuntimeProvider::Static {
        response: crate::search::types::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 0 web results".to_string(),
            results: Vec::new(),
        },
    };
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider_and_search_fetch_providers(
        provider,
        store.clone(),
        search_provider,
        web_fetch_provider,
    )
    .await
    .expect("runtime");
    (handle, store)
}

async fn append_test_text_item(store: &crate::NoemaStore, conversation_id: &str, text: &str) {
    append_test_text_item_with_kind(store, conversation_id, ConversationItemKind::UserText, text)
        .await;
}

async fn append_test_text_item_with_kind(
    store: &crate::NoemaStore,
    conversation_id: &str,
    kind: ConversationItemKind,
    text: &str,
) {
    let author = match kind {
        ConversationItemKind::UserText => ActorRef::human("human:local"),
        ConversationItemKind::AssistantText => ActorRef::agent("agent:primary"),
        ConversationItemKind::Activity
        | ConversationItemKind::A2uiCard
        | ConversationItemKind::MultipleChoicePrompt
        | ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::Reasoning
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult
        | ConversationItemKind::ErrorNotice => ActorRef::agent("agent:primary"),
    };
    store
        .append_conversation_item(crate::NewConversationItem {
            conversation_id: conversation_id.to_string(),
            turn_id: None,
            parent_item_id: None,
            kind,
            status: ConversationItemStatus::Completed,
            author,
            content_text: Some(text.to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("append item");
}

async fn wait_for_context_summary_count(
    store: &crate::NoemaStore,
    conversation_id: &str,
    minimum_count: usize,
) {
    for _ in 0..50 {
        let summaries = store
            .list_context_summaries_for_conversation(conversation_id)
            .await
            .expect("summaries");
        if summaries.len() >= minimum_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {minimum_count} context summaries");
}

fn estimated_test_tokens(value: &str) -> u32 {
    value.chars().count().div_ceil(3) as u32
}

#[derive(Debug, Clone)]
struct FakeCodexProvider {
    scenario: FakeCodexScenario,
}

#[derive(Debug)]
struct RecordingFakeProvider {
    provider_kind: String,
    inner: FakeCodexProvider,
    requests: Mutex<Vec<GenerateRequest>>,
    tool_capabilities: ProviderToolCapabilities,
}

impl RecordingFakeProvider {
    fn new(provider_kind: &str, scenario: FakeCodexScenario) -> Self {
        Self {
            provider_kind: provider_kind.to_string(),
            inner: FakeCodexProvider::new(scenario),
            requests: Mutex::new(Vec::new()),
            tool_capabilities: ProviderToolCapabilities {
                fallback_mode: ProviderToolFallbackMode::BuiltinOnlyEnvelope,
                ..ProviderToolCapabilities::default()
            },
        }
    }

    fn with_tool_capabilities(mut self, tool_capabilities: ProviderToolCapabilities) -> Self {
        self.tool_capabilities = tool_capabilities;
        self
    }

    fn requests(&self) -> Vec<GenerateRequest> {
        self.requests.lock().expect("requests").clone()
    }
}

#[derive(Debug)]
struct CapturingProvider {
    capabilities: ProviderToolCapabilities,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl Default for CapturingProvider {
    fn default() -> Self {
        Self {
            capabilities: ProviderToolCapabilities::default(),
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct MetadataCapturingProvider {
    context_window_tokens: u32,
    fail_compaction: bool,
    fail_token_count: bool,
    enforce_context_window: bool,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl Default for MetadataCapturingProvider {
    fn default() -> Self {
        Self {
            context_window_tokens: 5_500,
            fail_compaction: false,
            fail_token_count: false,
            enforce_context_window: false,
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct BlockingOnceProvider {
    started: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
}

#[derive(Debug, Clone, Copy)]
enum FakeCodexScenario {
    Simple,
    MultipleChoice,
    ReasoningReplay,
    RestartContext,
    IdentityPromptCheck,
    PromptPhaseContract,
    PromptMarkdownContract,
    InitialNameOnboarding,
    InitialNameOnboardingNoAssistant,
    TurnError,
    ToolItem,
    ToolCallBeforeCommentary,
    ToolItemThenFailure,
    UncalibratedMcpToolCall,
    FailedMcpToolResultContinuation,
    InvalidSearchMemory,
    SearchMemoryContinuation,
    NativeSearchMemoryContinuation,
    NativeWebSearch,
    NativeWebSearchContinuation,
    NativeWebFetchContinuation,
    ChainedSearchMemoryContinuation,
    MemoryContextQuestion,
    LongContinuationThenFinalization,
    ProgressAuditFailsThenFinalization,
    SearchMemoryProfileContinuation,
    UpdateOwnNameContinuation,
    UpdateOwnNameThenYay,
    RepeatedUpdateOwnNameContinuation,
    AmbiguousUpdateOwnName,
    UpdateOwnNameThenIdentityCheck,
}

fn fake_provider(scenario: FakeCodexScenario) -> FakeCodexProvider {
    FakeCodexProvider::new(scenario)
}

impl FakeCodexProvider {
    fn new(scenario: FakeCodexScenario) -> Self {
        Self { scenario }
    }

    fn generate_response(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, ProviderError> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| "fake-model".to_string());
        let rendered_input = request.input.render_for_token_count();
        let input = current_user_input(&request.input);
        let instructions = request.instructions.unwrap_or_default();
        let output = match self.scenario {
            FakeCodexScenario::Simple => assistant_with_no_memories("fake answer"),
            FakeCodexScenario::MultipleChoice => vec![GenerateOutputItem::MultipleChoice {
                phase: Some(AssistantTextPhase::FinalAnswer),
                prompt: "Pick a direction".to_string(),
                selection_mode: MultipleChoiceSelectionMode::PickOne,
                options: vec![
                    MultipleChoiceOption {
                        id: "ship".to_string(),
                        label: "Ship it".to_string(),
                    },
                    MultipleChoiceOption {
                        id: "polish".to_string(),
                        label: "Polish first".to_string(),
                    },
                ],
            }],
            FakeCodexScenario::ReasoningReplay => {
                let saw_reasoning_replay = match &request.input {
                    GenerateInput::Items(items) => items.iter().any(|item| {
                        matches!(
                            item,
                            GenerateInputItem::Reasoning(reasoning)
                                if reasoning.encrypted_content == "opaque-turn-one"
                        )
                    }),
                    _ => false,
                };
                let mut response = fake_generate_response(
                    vec![GenerateOutputItem::AssistantText {
                        phase: None,
                        text: if saw_reasoning_replay {
                            "saw encrypted reasoning"
                        } else {
                            "first answer"
                        }
                        .to_string(),
                    }],
                    "codex",
                    model,
                );
                if !saw_reasoning_replay {
                    response.reasoning_items.push(GenerateReasoningItem {
                        id: Some("rs_fake_1".to_string()),
                        encrypted_content: Some("opaque-turn-one".to_string()),
                    });
                }
                return Ok(response);
            }
            FakeCodexScenario::RestartContext => {
                let saw_context = rendered_input.contains("first durable question")
                    && rendered_input.contains("fake answer")
                    && input.contains("second durable question")
                    && !instructions.contains("Recent durable transcript")
                    && !instructions.contains("first durable question");
                assistant_with_no_memories(if saw_context {
                    "saw durable context"
                } else {
                    "fake answer"
                })
            }
            FakeCodexScenario::IdentityPromptCheck => {
                let saw_identity = instructions.contains("Agent identity:")
                    && instructions.contains(r#"agent_id: "agent:primary""#)
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("update_own_name");
                assistant_with_no_memories(if saw_identity {
                    "saw unnamed identity"
                } else {
                    "missing unnamed identity"
                })
            }
            FakeCodexScenario::PromptPhaseContract => {
                let saw_phase_contract = instructions.contains(r#""phase":"commentary""#)
                    && instructions.contains(r#""phase":"final_answer""#)
                    && instructions.contains("Use phase \"commentary\" for text that explains what you are about to do before a tool result is available.")
                    && instructions.contains("Use phase \"final_answer\" only for the terminal answer after required tool results are available.");
                assistant_with_no_memories(if saw_phase_contract {
                    "saw phase contract"
                } else {
                    "missing phase contract"
                })
            }
            FakeCodexScenario::PromptMarkdownContract => {
                let saw_markdown_contract = instructions.contains(
                    "User-visible assistant text may use Markdown when it makes the answer clearer.",
                ) && instructions.contains(
                    "Keep Markdown inside responses[].text; the outer response must remain strict JSON.",
                );
                assistant_with_no_memories(if saw_markdown_contract {
                    "saw markdown contract"
                } else {
                    "missing markdown contract"
                })
            }
            FakeCodexScenario::InitialNameOnboarding => {
                let saw_onboarding = instructions.contains("Agent identity:")
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("Ask the user what they would like to name you.")
                    && instructions.contains("warm and welcoming")
                    && instructions.contains("energy")
                    && instructions.contains("Noema personal agent")
                    && instructions.contains("keep life moving with a little more ease")
                    && instructions.contains("what they would like to name you")
                    && instructions.contains("think, plan, make, untangle")
                    && instructions
                        .contains("Split the introduction into three separate text responses")
                    && instructions.contains("Always include exactly three text responses")
                    && !input.contains("Your name is");
                if saw_onboarding {
                    assistant_items_with_no_memories(&[
                        "hey, i’m glad to be here with you 👋",
                        "i can help you think, plan, make, untangle, and keep life moving with a little more ease",
                        "what would you like to name me?",
                    ])
                } else {
                    assistant_with_no_memories("missing warm onboarding prompt")
                }
            }
            FakeCodexScenario::InitialNameOnboardingNoAssistant => Vec::new(),
            FakeCodexScenario::TurnError => {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "turn failed".to_string(),
                    request_id: None,
                });
            }
            FakeCodexScenario::ToolItem => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("tool result received")
                } else {
                    vec![
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "fake answer".to_string(),
                        },
                    ]
                }
            }
            FakeCodexScenario::ToolCallBeforeCommentary => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("The memory check is complete.")
                } else {
                    vec![
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::AssistantText {
                            phase: Some(crate::provider::AssistantTextPhase::Commentary),
                            text: "Checking memory.".to_string(),
                        },
                    ]
                }
            }
            FakeCodexScenario::ToolItemThenFailure => {
                return Err(ProviderError::PartialResponse {
                    provider: "codex".to_string(),
                    model,
                    message: "tool failed later".to_string(),
                    output: vec![search_memory_action_item(
                        "call_1",
                        json!({"arguments": {"query": "trains"}}),
                    )],
                });
            }
            FakeCodexScenario::UncalibratedMcpToolCall => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("uncalibrated MCP tool failed")
                } else {
                    vec![mcp_tool_call(
                        "call_mcp_1",
                        "mcp.docs.read",
                        json!({"arguments": {"document_id": "doc_1"}}),
                    )]
                }
            }
            FakeCodexScenario::FailedMcpToolResultContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT")
                    && input.contains("mcp_server_not_found")
                {
                    assistant_with_no_memories("I saw the Notion tool failure and can explain it.")
                } else {
                    vec![mcp_tool_call(
                        "call_notion_create_1",
                        "mcp.mcp:notion.notion-create-pages",
                        json!({
                            "pages": [{
                                "properties": {"title": "Test page"},
                                "content": "Body"
                            }]
                        }),
                    )]
                }
            }
            FakeCodexScenario::InvalidSearchMemory => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("invalid tool result received")
                } else {
                    vec![search_memory_tool_call(
                        "call_bad",
                        json!({"arguments": {"query": "trains", "purpose": "dump_everything"}}),
                    )]
                }
            }
            FakeCodexScenario::SearchMemoryContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("I found your train memory.")
                } else if input.contains("Please remember I'm a big fan of trains") {
                    assistant_with_no_memories("fake answer")
                } else if input.contains("What do you remember about trains?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::NativeSearchMemoryContinuation => match &request.input {
                GenerateInput::NativeToolResults(results) => {
                    if results.iter().any(|result| {
                        result.call_id == "call_native_1" && result.name == "search_memory"
                    }) {
                        assistant_with_no_memories("native tool result received")
                    } else {
                        assistant_with_no_memories("wrong native tool result")
                    }
                }
                _ if input.contains("NOEMA_LOCAL_TOOL_RESULT") => {
                    assistant_with_no_memories("legacy tool result received")
                }
                _ if input.contains("What do you remember about trains?") => vec![
                    GenerateOutputItem::AssistantText {
                        phase: None,
                        text: "Searching memory.".to_string(),
                    },
                    GenerateOutputItem::ToolCall {
                        id: Some("item_native_1".to_string()),
                        provider_call_id: Some("call_native_1".to_string()),
                        provider_name: Some("search_memory".to_string()),
                        name: "search_memory".to_string(),
                        payload: json!({"arguments": {"query": "trains"}}),
                    },
                ],
                _ => assistant_with_no_memories("fake answer"),
            },
            FakeCodexScenario::NativeWebSearch => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("web search result received")
                } else {
                    vec![web_search_tool_call(
                        "call_web_1",
                        json!({
                            "query": "rust language",
                            "reason": "answer the user's request",
                            "max_results": 3
                        }),
                    )]
                }
            }
            FakeCodexScenario::NativeWebSearchContinuation => match &request.input {
                GenerateInput::NativeToolResults(results)
                    if results.iter().any(|result| result.name == "web.search") =>
                {
                    assistant_with_no_memories("I found a current web result.")
                }
                GenerateInput::NativeToolResults(_) => {
                    assistant_with_no_memories("wrong web search tool result")
                }
                _ => vec![web_search_tool_call(
                    "call_web_1",
                    json!({
                        "query": "rust language",
                        "reason": "answer the current question",
                        "max_results": 3
                    }),
                )],
            },
            FakeCodexScenario::NativeWebFetchContinuation => match &request.input {
                GenerateInput::NativeToolResults(results)
                    if results.iter().any(|result| result.name == "web.fetch") =>
                {
                    assistant_with_no_memories("I read the fetched page.")
                }
                GenerateInput::NativeToolResults(_) => {
                    assistant_with_no_memories("wrong web fetch tool result")
                }
                _ => vec![web_fetch_tool_call(
                    "call_fetch_1",
                    json!({
                        "url": "https://example.com/page",
                        "reason": "answer the current question",
                        "max_chars": 5000
                    }),
                )],
            },
            FakeCodexScenario::ChainedSearchMemoryContinuation => {
                if input.contains("call_2") {
                    assistant_with_no_memories("I checked both memory topics.")
                } else if input.contains("call_1") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "I need one more memory check.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_2",
                            json!({"arguments": {"query": "planes"}}),
                        ),
                    ]
                } else if input.contains("Check memory twice before answering.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Checking memory first.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MemoryContextQuestion => {
                if input.contains("start memory context test") {
                    assistant_items_with_no_memories(&[
                        "what are some topics you find interesting?",
                        "short answers are fine too",
                    ])
                } else {
                    assistant_with_no_memories("got it")
                }
            }
            FakeCodexScenario::LongContinuationThenFinalization => {
                let input_text = request.input.render_for_token_count();
                let loop_query = format!("restaurants {}", input_text.len());
                if request.tools.is_empty()
                    && !request.parallel_tool_calls
                    && instructions.contains("must stop now")
                {
                    assistant_with_no_memories(
                        "I gathered partial results and paused before the tool loop could run too long.",
                    )
                } else {
                    vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )]
                }
            }
            FakeCodexScenario::ProgressAuditFailsThenFinalization => {
                let input_text = request.input.render_for_token_count();
                let loop_query = format!("restaurants {}", input_text.len());
                if request.tools.is_empty()
                    && instructions.contains(
                        "You are auditing whether a Noema tool-continuation loop is making progress.",
                    )
                {
                    return Err(crate::provider::ProviderError::ProtocolError {
                        provider: "codex".to_string(),
                        message: "audit failed".to_string(),
                    });
                }
                if request.tools.is_empty() && instructions.contains("must stop now") {
                    assistant_with_no_memories(
                        "The progress check failed, so I am pausing with the useful work gathered so far.",
                    )
                } else {
                    vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )]
                }
            }
            FakeCodexScenario::SearchMemoryProfileContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![GenerateOutputItem::AssistantText {
                        phase: None,
                        text: "I remember that you like planes.".to_string(),
                    }]
                } else if input.contains("What memories do you have of me?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_profile",
                            json!({"arguments": {
                                "scope_ids": ["human:local"],
                                "query": "",
                                "purpose": "answer_human_question",
                                "limit": 8
                            }}),
                        ),
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::UpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    let expected_name = if instructions.contains(r#"display_name: "Fred""#) {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    let display_name_marker = format!(r#"display_name: "{expected_name}""#);
                    let saw_updated_identity = instructions.contains("Agent identity:")
                        && instructions.contains(&display_name_marker)
                        && !instructions.contains("You do not have a name yet.");
                    let saw_onboarding_tasks = instructions
                        .contains("Onboarding tasks, in priority order:")
                        && instructions.contains("what the user wants help with first");
                    if saw_updated_identity && saw_onboarding_tasks {
                        let reply =
                            format!("{expected_name} it is. what would you like help with first?");
                        assistant_with_no_memories(&reply)
                    } else if saw_updated_identity {
                        let reply = format!("{expected_name} it is.");
                        assistant_with_no_memories(&reply)
                    } else {
                        assistant_with_no_memories("same-turn identity was stale")
                    }
                } else {
                    let name = if input.contains("Fred") {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": name}),
                    )]
                }
            }
            FakeCodexScenario::UpdateOwnNameThenYay => {
                if input.contains("Let's rename you to Momo") {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Momo"}),
                    )]
                } else if input == "Yay" {
                    if rendered_input.contains("function_call_output")
                        && rendered_input.contains("update_own_name")
                        && rendered_input.contains("Momo")
                    {
                        assistant_with_no_memories("yay acknowledged after saved name")
                    } else {
                        vec![update_own_name_tool_call(
                            "call_name_2",
                            json!({"name": "Momo"}),
                        )]
                    }
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::RepeatedUpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![
                        update_own_name_tool_call("call_name_2", json!({"name": "Fred"})),
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Fred it is.".to_string(),
                        },
                    ]
                } else {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Fred"}),
                    )]
                }
            }
            FakeCodexScenario::AmbiguousUpdateOwnName => {
                assistant_with_no_memories("Please confirm what you'd like to call me.")
            }
            FakeCodexScenario::UpdateOwnNameThenIdentityCheck => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("Mira it is.")
                } else if input.contains("Your name is Mira.") {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Mira"}),
                    )]
                } else {
                    let saw_identity = instructions.contains("Agent identity:")
                        && instructions.contains(r#"display_name: "Mira""#)
                        && !instructions.contains("You do not have a name yet.");
                    assistant_with_no_memories(if saw_identity {
                        "saw stored identity"
                    } else {
                        "missing stored identity"
                    })
                }
            }
        };

        Ok(fake_generate_response(output, "codex", model))
    }
}

fn current_user_input(input: &GenerateInput) -> String {
    match input {
        GenerateInput::Text(text) => text.clone(),
        GenerateInput::Messages(messages) => messages
            .iter()
            .rev()
            .find(|message| message.role == crate::provider::GenerateMessageRole::User)
            .map_or_else(
                || input.render_for_token_count(),
                |message| message.content.clone(),
            ),
        GenerateInput::Items(items) => items
            .iter()
            .rev()
            .find_map(|item| match item {
                crate::provider::GenerateInputItem::Message(message)
                    if message.role == crate::provider::GenerateMessageRole::User =>
                {
                    Some(message.content.clone())
                }
                crate::provider::GenerateInputItem::Message(_)
                | crate::provider::GenerateInputItem::Reasoning(_)
                | crate::provider::GenerateInputItem::ToolCall(_)
                | crate::provider::GenerateInputItem::ToolResult(_) => None,
            })
            .unwrap_or_else(|| input.render_for_token_count()),
        GenerateInput::NativeToolResults(_) => input.render_for_token_count(),
    }
}

fn input_message_texts(input: &GenerateInput) -> Vec<String> {
    match input {
        GenerateInput::Text(text) => vec![text.clone()],
        GenerateInput::Messages(messages) => messages
            .iter()
            .map(|message| message.content.clone())
            .collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message) => Some(message.content.clone()),
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(_) => Vec::new(),
    }
}

async fn insert_authenticated_provider_account(
    store: &crate::NoemaStore,
    provider_account_id: &str,
    provider_kind: &str,
    account_key: &str,
) {
    crate::store::tests::insert_provider_account_for_tests(
        store,
        provider_account_id,
        provider_kind,
        account_key,
        &format!("{provider_kind} {account_key}"),
        crate::ProviderAuthMethod::SecretInput,
        false,
        crate::ProviderAccountStatus::Authenticated,
        json!({}),
    )
    .await;
}

impl super::runtime::RuntimeModelProvider for FakeCodexProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            fallback_mode: ProviderToolFallbackMode::BuiltinOnlyEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self.generate_response(request)?;
            for (response_index, response_item) in response.responses.iter().enumerate() {
                if let GenerateResponseItem::Text { text, .. } = response_item {
                    let mut chunk = String::new();
                    for character in text.chars() {
                        chunk.push(character);
                        if chunk.chars().count() == 4 {
                            on_event(GenerateStreamEvent::AssistantTextDelta {
                                response_index,
                                delta: chunk,
                            });
                            chunk = String::new();
                        }
                    }
                    if !chunk.is_empty() {
                        on_event(GenerateStreamEvent::AssistantTextDelta {
                            response_index,
                            delta: chunk,
                        });
                    }
                }
            }
            for (index, tool_call) in response.tool_calls.iter().enumerate() {
                on_event(GenerateStreamEvent::ToolCallStarted {
                    output_index: response.responses.len() + index,
                    name: tool_call.name.clone(),
                });
            }
            Ok(response)
        })
    }
}

impl super::runtime::RuntimeModelProvider for RecordingFakeProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        self.tool_capabilities
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.clone());
            let mut response = self.inner.generate_response(request)?;
            response.provider = self.provider_kind.clone();
            for (response_index, response_item) in response.responses.iter().enumerate() {
                if let GenerateResponseItem::Text { text, .. } = response_item {
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        response_index,
                        delta: text.clone(),
                    });
                }
            }
            for (index, tool_call) in response.tool_calls.iter().enumerate() {
                on_event(GenerateStreamEvent::ToolCallStarted {
                    output_index: response.responses.len() + index,
                    name: tool_call.name.clone(),
                });
            }
            Ok(response)
        })
    }
}

impl super::runtime::RuntimeModelProvider for CapturingProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        self.capabilities
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.clone());
            Ok(fake_generate_response(
                assistant_with_no_memories("fake answer"),
                "test",
                request.model.unwrap_or_else(|| "fake-model".to_string()),
            ))
        })
    }
}

impl super::runtime::RuntimeModelProvider for MetadataCapturingProvider {
    fn context_metadata(&self, _model: Option<&str>) -> crate::ProviderContextMetadata {
        crate::ProviderContextMetadata {
            context_window_tokens: Some(self.context_window_tokens),
            default_output_reserve_tokens: Some(512),
            compact_summary_target_tokens: Some(512),
        }
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        _model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            if self.fail_token_count {
                return Err(ProviderError::ProviderUnavailable {
                    provider: "test".to_string(),
                    message: "token count unavailable".to_string(),
                });
            }
            let instruction_tokens = instructions.map_or(0, estimated_test_tokens);
            Ok(Some(instruction_tokens + estimated_test_tokens(input)))
        })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.clone());
            if self.enforce_context_window {
                let input = request.input.render_for_token_count();
                let input_tokens = request
                    .instructions
                    .as_deref()
                    .map_or(0, estimated_test_tokens)
                    + estimated_test_tokens(&input);
                let available = self
                    .context_window_tokens
                    .saturating_sub(request.options.max_output_tokens.unwrap_or(512))
                    .saturating_sub(128);
                if input_tokens > available {
                    return Err(ProviderError::ApiError {
                        status: 400,
                        message: format!(
                            "exceeded context window size: Content contains {input_tokens} tokens"
                        ),
                        request_id: None,
                    });
                }
            }
            if self.fail_compaction && !request.options.require_noema_response {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "compaction failed".to_string(),
                    request_id: None,
                });
            }
            Ok(fake_generate_response(
                assistant_with_no_memories("fake answer"),
                "test",
                request.model.unwrap_or_else(|| "fake-model".to_string()),
            ))
        })
    }
}

impl super::runtime::RuntimeModelProvider for BlockingOnceProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(started) = self.started.lock().expect("started lock").take() {
                let _ = started.send(());
            }
            let release = self
                .release
                .lock()
                .expect("release lock")
                .take()
                .expect("release receiver");
            release
                .await
                .map_err(|_| ProviderError::ProviderUnavailable {
                    provider: "test".to_string(),
                    message: "release signal dropped".to_string(),
                })?;
            Ok(fake_generate_response(
                assistant_with_no_memories("slow answer"),
                "test",
                "blocking-once".to_string(),
            ))
        })
    }
}

fn fake_generate_response(
    output: Vec<GenerateOutputItem>,
    provider: &str,
    model: String,
) -> GenerateResponse {
    let mut responses = Vec::new();
    let mut tool_calls = Vec::new();

    for item in output {
        match item {
            GenerateOutputItem::AssistantText { phase, text } => {
                responses.push(GenerateResponseItem::Text { phase, text });
            }
            GenerateOutputItem::MultipleChoice {
                phase,
                prompt,
                selection_mode,
                options,
            } => {
                responses.push(GenerateResponseItem::MultipleChoice {
                    phase,
                    prompt,
                    selection_mode,
                    options,
                });
            }
            GenerateOutputItem::ToolCall {
                id,
                provider_call_id,
                provider_name,
                name,
                payload,
            } => {
                tool_calls.push(GenerateToolCall {
                    id,
                    provider_call_id,
                    provider_name,
                    name,
                    payload,
                });
            }
        }
    }

    let response_status = if tool_calls.is_empty() {
        GenerateResponseStatus::Final
    } else {
        GenerateResponseStatus::NeedsTools
    };

    GenerateResponse {
        responses,
        tool_calls,
        reasoning_items: Vec::new(),
        response_status,
        provider: provider.to_string(),
        model,
        response_id: Some("fake-response".to_string()),
        usage: None,
    }
}

fn assistant_with_no_memories(text: &str) -> Vec<GenerateOutputItem> {
    assistant_items_with_no_memories(&[text])
}

fn assistant_items_with_no_memories(texts: &[&str]) -> Vec<GenerateOutputItem> {
    texts
        .iter()
        .map(|text| GenerateOutputItem::AssistantText {
            phase: None,
            text: (*text).to_string(),
        })
        .collect()
}

fn search_memory_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "search_memory".to_string(),
        payload,
    }
}

fn web_search_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: Some(id.to_string()),
        provider_name: Some("web.search".to_string()),
        name: "web.search".to_string(),
        payload,
    }
}

fn web_fetch_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: Some(id.to_string()),
        provider_name: Some("web.fetch".to_string()),
        name: "web.fetch".to_string(),
        payload,
    }
}

fn search_memory_action_item(id: &str, payload: serde_json::Value) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "search_memory".to_string(),
        payload,
    }
}

fn update_own_name_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: "update_own_name".to_string(),
        payload,
    }
}

fn mcp_tool_call(id: &str, name: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        provider_call_id: None,
        provider_name: None,
        name: name.to_string(),
        payload,
    }
}
